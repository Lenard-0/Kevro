//! Launch or activate apps via NSWorkspace. Needs no Accessibility permission.

use std::path::PathBuf;

use objc2::rc::Retained;
use objc2_app_kit::{
    NSApplicationActivationOptions, NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration,
};
use objc2_foundation::{NSString, NSURL};

/// Launch `target` (a bundle ID or app name) if it isn't running; otherwise
/// bring it to the front. If `hide_if_frontmost` and it is already frontmost,
/// hide it instead.
pub fn launch_or_focus(target: &str, hide_if_frontmost: bool) {
    let workspace = NSWorkspace::sharedWorkspace();

    if let Some(app) = find_running(&workspace, target) {
        let pid = app.processIdentifier();
        let frontmost = workspace
            .frontmostApplication()
            .map(|a| a.processIdentifier());
        if hide_if_frontmost && frontmost == Some(pid) {
            tracing::info!("{target} is frontmost, hiding it");
            app.hide();
            return;
        }
        if app.isHidden() {
            app.unhide();
        }
        // Since macOS 14, activation requests from a background app are only
        // a hint and are often silently ignored even though they return true.
        // So we request activation of all windows, then also open the app's
        // bundle through LaunchServices, which behaves like a Dock click and
        // reliably brings it forward (creating a window if it has none).
        app.activateWithOptions(NSApplicationActivationOptions::ActivateAllWindows);
        if let Some(url) = app.bundleURL() {
            open_url(&workspace, &url, target);
        }
        tracing::info!("focused {target} (pid {pid})");
        return;
    }

    match find_app_url(&workspace, target) {
        Some(url) => open_url(&workspace, &url, target),
        None => open_with_command(target),
    }
}

fn find_running(workspace: &NSWorkspace, target: &str) -> Option<Retained<NSRunningApplication>> {
    let by_id =
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(target));
    if let Some(app) = by_id.iter().find(|a| !a.isTerminated()) {
        return Some(app);
    }
    let wanted = target.trim_end_matches(".app").to_lowercase();
    workspace.runningApplications().iter().find(|a| {
        !a.isTerminated()
            && a.localizedName()
                .is_some_and(|n| n.to_string().to_lowercase() == wanted)
    })
}

fn find_app_url(workspace: &NSWorkspace, target: &str) -> Option<Retained<NSURL>> {
    if let Some(url) = workspace.URLForApplicationWithBundleIdentifier(&NSString::from_str(target))
    {
        return Some(url);
    }
    let file = if target.ends_with(".app") {
        target.to_string()
    } else {
        format!("{target}.app")
    };
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    [
        PathBuf::from("/Applications"),
        PathBuf::from("/Applications/Utilities"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
        home.join("Applications"),
    ]
    .into_iter()
    .map(|dir| dir.join(&file))
    .find(|p| p.exists())
    .map(|p| NSURL::fileURLWithPath(&NSString::from_str(&p.to_string_lossy())))
}

fn open_url(workspace: &NSWorkspace, url: &NSURL, target: &str) {
    let config = NSWorkspaceOpenConfiguration::configuration();
    config.setActivates(true);
    workspace.openApplicationAtURL_configuration_completionHandler(url, &config, None);
    tracing::info!("opening {target}");
}

/// Last resort: `open -a`, which also searches Spotlight for the app.
fn open_with_command(target: &str) {
    let flag = if target.contains('.') && !target.contains(' ') {
        "-b"
    } else {
        "-a"
    };
    match std::process::Command::new("/usr/bin/open")
        .arg(flag)
        .arg(target)
        .spawn()
    {
        Ok(mut child) => {
            // Reap in the background so we don't leave a zombie process.
            let target = target.to_string();
            std::thread::spawn(move || match child.wait() {
                Ok(s) if s.success() => tracing::info!("opened {target} via open {flag}"),
                Ok(s) => tracing::warn!("app \"{target}\" not found (open {flag} exited with {s})"),
                Err(e) => tracing::warn!("open {flag} {target} failed: {e}"),
            });
        }
        Err(e) => tracing::warn!("cannot run open for \"{target}\": {e}"),
    }
}

/// A running app with a UI (Dock-style "regular" app).
#[derive(Debug, Clone)]
pub struct RunningApp {
    pub pid: i32,
    pub bundle_id: Option<String>,
    pub name: Option<String>,
}

impl RunningApp {
    /// Does a config target (bundle ID or app name) refer to this app?
    pub fn matches(&self, target: &str) -> bool {
        let t = target.trim_end_matches(".app");
        self.bundle_id
            .as_deref()
            .is_some_and(|b| b.eq_ignore_ascii_case(t))
            || self
                .name
                .as_deref()
                .is_some_and(|n| n.eq_ignore_ascii_case(t))
    }
}

pub fn running_apps() -> Vec<RunningApp> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter(|a| {
            !a.isTerminated()
                && a.activationPolicy() == objc2_app_kit::NSApplicationActivationPolicy::Regular
        })
        .map(|a| RunningApp {
            pid: a.processIdentifier(),
            bundle_id: a.bundleIdentifier().map(|s| s.to_string()),
            name: a.localizedName().map(|s| s.to_string()),
        })
        .collect()
}

/// Call `f` on the main thread whenever an app launches or quits.
pub fn on_apps_changed(f: impl Fn() + 'static) {
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    let f = std::rc::Rc::new(f);
    // SAFETY: these are valid static notification names.
    let names = unsafe {
        [
            objc2_app_kit::NSWorkspaceDidLaunchApplicationNotification,
            objc2_app_kit::NSWorkspaceDidTerminateApplicationNotification,
        ]
    };
    for name in names {
        let f = f.clone();
        let block = block2::RcBlock::new(
            move |_n: std::ptr::NonNull<objc2_foundation::NSNotification>| f(),
        );
        // SAFETY: a nil queue delivers on the posting (main) thread. The
        // returned token must outlive the observation, which is the whole
        // life of the app, so it is deliberately leaked.
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
        };
        std::mem::forget(token);
    }
}
