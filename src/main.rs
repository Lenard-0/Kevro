//! Kevro: keyboard-first window and app control for macOS.
//!
//! Startup, CLI subcommands, logging, the main-thread event loop, the menu
//! bar icon and config hot-reload.

mod actions;
mod apps;
mod ax;
mod config;
mod focus;
mod geom;
mod hotkeys;
mod login;
mod registry;
mod screens;
mod snap;
mod tray;

use std::time::{Duration, Instant, SystemTime};

use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use tray_icon::menu::MenuEvent;

use crate::config::Config;
use crate::hotkeys::Hotkeys;
use crate::tray::{MenuAction, Tray};

/// How often to check the config file and Accessibility status for changes.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug)]
enum UserEvent {
    Hotkey(GlobalHotKeyEvent),
    Menu(MenuEvent),
}

const USAGE: &str = "\
Usage: kevro [COMMAND]

Commands:
  (none)           Run Kevro (menu bar app)
  action <ACTION>  Run one action now and exit, e.g. `kevro action snap:left_half`,
                   `kevro action focus:focus_right`, `kevro action app:com.apple.Safari`
  install          Start Kevro at login (writes a LaunchAgent and starts it now)
  uninstall        Stop starting at login (removes the LaunchAgent and stops it)
  windows          List on-screen windows as Kevro sees them (for debugging)
  status           Show Accessibility permission, screens and config status
  help             Show this message";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _log_guard = init_logging();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [] => {
            tracing::info!("Kevro {} starting", env!("CARGO_PKG_VERSION"));
            run();
        }
        ["action", spec] => match actions::Action::parse(spec) {
            Ok(action) => {
                let options = config::load().map(|c| c.options).unwrap_or_default();
                actions::Dispatcher::default().dispatch(&action, &options);
                // Give async work (app launches, logging) a moment to finish.
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        },
        ["status"] => print_status(),
        ["install"] => exit_on_err(login::install()),
        ["uninstall"] => exit_on_err(login::uninstall()),
        ["windows"] => {
            for w in ax::onscreen_windows() {
                let name = objc2_app_kit::NSRunningApplication::runningApplicationWithProcessIdentifier(w.pid)
                    .and_then(|a| a.localizedName())
                    .map(|n| n.to_string())
                    .unwrap_or_default();
                println!("{:>6}  {:<24} {:?}", w.pid, name, w.frame);
            }
        }
        ["help" | "--help" | "-h"] => println!("{USAGE}"),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

fn exit_on_err(r: Result<(), String>) {
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn print_status() {
    let trusted = ax::is_trusted();
    println!("Kevro {}", env!("CARGO_PKG_VERSION"));
    println!("Executable:    {}", std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_default());
    println!(
        "Accessibility: {}",
        if trusted { "granted" } else { "NOT granted (snapping and focus disabled)" }
    );
    println!("Config:        {}", config::config_path().display());
    match config::load() {
        Ok(c) => println!("               valid, {} bindings", c.bindings.len()),
        Err(e) => println!("               ERROR: {e}"),
    }
    println!("Log:           {}", config::log_dir().join("kevro.log").display());
    let agent = login::plist_path();
    println!(
        "Login item:    {}",
        if agent.exists() { format!("installed ({})", agent.display()) } else { "not installed".into() }
    );
    for (i, s) in screens::all().iter().enumerate() {
        println!("Screen {i}:      frame {:?}\n               visible {:?}", s.frame, s.visible);
    }
}

fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let filter = || EnvFilter::try_from_env("KEVRO_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let stderr = fmt::layer().with_writer(std::io::stderr).with_filter(filter());

    let dir = config::log_dir();
    match std::fs::create_dir_all(&dir) {
        Ok(()) => {
            let appender = tracing_appender::rolling::never(&dir, "kevro.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let file = fmt::layer().with_ansi(false).with_writer(writer).with_filter(filter());
            tracing_subscriber::registry().with(stderr).with(file).init();
            Some(guard)
        }
        Err(e) => {
            tracing_subscriber::registry().with(stderr).init();
            tracing::warn!("cannot create log dir {}: {e}", dir.display());
            None
        }
    }
}

struct App {
    tray: Option<Tray>,
    hotkeys: Hotkeys,
    dispatcher: actions::Dispatcher,
    config: Config,
    config_mtime: Option<SystemTime>,
}

impl App {
    /// Load the config file and re-register hotkeys. On error, keep the
    /// previous config and bindings.
    fn reload_config(&mut self) {
        self.config_mtime = config::modified_time();
        match config::load() {
            Ok(cfg) => {
                let n = self.hotkeys.apply(&cfg.bindings);
                tracing::info!(
                    "config loaded from {}: {n}/{} hotkeys registered",
                    config::config_path().display(),
                    cfg.bindings.len()
                );
                self.config = cfg;
            }
            Err(e) => tracing::error!("config error, keeping previous bindings: {e}"),
        }
    }

    fn poll(&mut self) {
        self.poll_config();
        if let Some(tray) = &mut self.tray {
            tray.set_trusted(ax::is_trusted());
        }
    }

    fn on_menu(&mut self, event: MenuEvent) {
        let Some(action) = self.tray.as_ref().and_then(|t| t.action_for(&event)) else { return };
        match action {
            MenuAction::Accessibility => open(&[tray::SETTINGS_URL]),
            MenuAction::Reload => {
                tracing::info!("reload requested from menu");
                self.reload_config();
            }
            MenuAction::OpenConfig => open(&["-t", &config::config_path().to_string_lossy()]),
            MenuAction::OpenLog => open(&[&config::log_dir().join("kevro.log").to_string_lossy()]),
            MenuAction::Quit => {
                tracing::info!("quit requested from menu");
                std::process::exit(0);
            }
        }
    }

    fn poll_config(&mut self) {
        let mtime = config::modified_time();
        if mtime.is_some() && mtime != self.config_mtime {
            tracing::info!("config file changed, reloading");
            self.reload_config();
        }
    }

    fn on_hotkey(&mut self, event: GlobalHotKeyEvent) {
        if event.state != HotKeyState::Pressed {
            return;
        }
        let Some((key, action)) = self.hotkeys.lookup(event.id) else { return };
        tracing::info!("hotkey {key} -> {action}");
        let action = action.clone();
        self.dispatcher.dispatch(&action, &self.config.options);
    }
}

fn open(args: &[&str]) {
    if let Err(e) = std::process::Command::new("/usr/bin/open").args(args).spawn() {
        tracing::warn!("open {args:?} failed: {e}");
    }
}

/// Exit if another bundled Kevro is already running (e.g. the login item
/// plus a manual launch). Exits with 0 so launchd doesn't restart us.
fn exit_if_already_running() {
    let own = std::process::id() as i32;
    let others = objc2_app_kit::NSRunningApplication::runningApplicationsWithBundleIdentifier(
        &objc2_foundation::NSString::from_str(login::LABEL),
    );
    if let Some(other) = others.iter().find(|a| a.processIdentifier() != own && !a.isTerminated()) {
        tracing::info!("Kevro is already running (pid {}), exiting", other.processIdentifier());
        std::process::exit(0);
    }
}

fn run() -> ! {
    exit_if_already_running();

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    // Menu-bar-only: no Dock icon, even when run unbundled from a terminal.
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let proxy = event_loop.create_proxy();
    GlobalHotKeyEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Hotkey(e));
    }));
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Menu(e));
    }));

    match config::ensure_config_file() {
        Ok(true) => tracing::info!("wrote default config to {}", config::config_path().display()),
        Ok(false) => {}
        Err(e) => tracing::error!("cannot write default config: {e}"),
    }

    let hotkeys = match Hotkeys::new() {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("cannot start the hotkey manager: {e}");
            std::process::exit(1);
        }
    };
    let mut app = App {
        tray: None,
        hotkeys,
        dispatcher: actions::Dispatcher::default(),
        config: Config::default(),
        config_mtime: None,
    };

    event_loop.run(move |event, _target, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) => {
                // The tray must be created once the event loop is running.
                match Tray::new() {
                    Ok(t) => app.tray = Some(t),
                    Err(e) => tracing::error!("cannot create menu bar icon: {e}"),
                }
                app.reload_config();
                if ax::is_trusted() {
                    tracing::info!("Accessibility permission granted");
                } else {
                    tracing::warn!(
                        "Accessibility permission not granted: app hotkeys work, snapping and \
                         focus are disabled until you allow Kevro in System Settings → \
                         Privacy & Security → Accessibility"
                    );
                    ax::is_trusted_with_prompt();
                }
                app.poll();
            }
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => app.poll(),
            Event::UserEvent(UserEvent::Hotkey(e)) => app.on_hotkey(e),
            Event::UserEvent(UserEvent::Menu(e)) => app.on_menu(e),
            _ => {}
        }
        if let ControlFlow::WaitUntil(t) = *control_flow
            && t > Instant::now()
        {
            return;
        }
        *control_flow = ControlFlow::WaitUntil(Instant::now() + POLL_INTERVAL);
    })
}
