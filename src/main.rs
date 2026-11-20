//! Kevro: keyboard-first window and app control for macOS.
//!
//! Startup, logging, the main-thread event loop and config hot-reload.

mod actions;
mod apps;
mod config;
mod focus;
mod hotkeys;
mod snap;

use std::time::{Duration, Instant, SystemTime};

use global_hotkey::{GlobalHotKeyEvent, HotKeyState};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt};

use crate::config::Config;
use crate::hotkeys::Hotkeys;

/// How often to check the config file for changes.
const CONFIG_POLL: Duration = Duration::from_secs(2);

#[derive(Debug)]
enum UserEvent {
    Hotkey(GlobalHotKeyEvent),
}

const USAGE: &str = "\
Usage: kevro [COMMAND]

Commands:
  (none)           Run Kevro (menu bar app)
  action <ACTION>  Run one action now and exit, e.g. `kevro action snap:left_half`,
                   `kevro action focus:focus_right`, `kevro action app:com.apple.Safari`
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
                actions::dispatch(&action, &options);
                // Give async work (app launches, logging) a moment to finish.
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        },
        ["help" | "--help" | "-h"] => println!("{USAGE}"),
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
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
    hotkeys: Hotkeys,
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
        actions::dispatch(&action, &self.config.options);
    }
}

fn run() -> ! {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    // Menu-bar-only: no Dock icon, even when run unbundled from a terminal.
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let proxy = event_loop.create_proxy();
    GlobalHotKeyEvent::set_event_handler(Some(move |e| {
        let _ = proxy.send_event(UserEvent::Hotkey(e));
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
    let mut app = App { hotkeys, config: Config::default(), config_mtime: None };

    event_loop.run(move |event, _target, control_flow| {
        match event {
            Event::NewEvents(StartCause::Init) => app.reload_config(),
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => app.poll_config(),
            Event::UserEvent(UserEvent::Hotkey(e)) => app.on_hotkey(e),
            _ => {}
        }
        if let ControlFlow::WaitUntil(t) = *control_flow
            && t > Instant::now()
        {
            return;
        }
        *control_flow = ControlFlow::WaitUntil(Instant::now() + CONFIG_POLL);
    })
}
