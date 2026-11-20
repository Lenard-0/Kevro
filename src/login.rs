//! Launch at login via a per-user LaunchAgent.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const LABEL: &str = "io.github.lenard-0.kevro";

pub fn plist_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join("Library").join("LaunchAgents").join(format!("{LABEL}.plist"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The LaunchAgent plist. `KeepAlive.SuccessfulExit = false` restarts Kevro
/// if it crashes, but not after "Quit Kevro" (which exits with status 0).
pub fn plist_contents(exe: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
</dict>
</plist>
"#,
        label = LABEL,
        exe = xml_escape(&exe.to_string_lossy()),
    )
}

fn gui_domain() -> Result<String, String> {
    let out = Command::new("/usr/bin/id").arg("-u").output().map_err(|e| e.to_string())?;
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if uid.is_empty() {
        return Err("could not determine user id".into());
    }
    Ok(format!("gui/{uid}"))
}

fn launchctl(args: &[&str]) -> Result<(), String> {
    let out = Command::new("/bin/launchctl").args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Write the LaunchAgent pointing at the running executable and load it.
pub fn install() -> Result<(), String> {
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| format!("cannot find own executable: {e}"))?;
    if !exe.to_string_lossy().contains(".app/Contents/MacOS/") {
        eprintln!(
            "warning: {} is not inside an .app bundle. Accessibility permission works best \
             for the bundled app; run `Kevro.app/Contents/MacOS/kevro install` instead.",
            exe.display()
        );
    }
    let path = plist_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, plist_contents(&exe)).map_err(|e| e.to_string())?;
    let domain = gui_domain()?;
    // Unload any previous version first; ignore "not loaded" errors.
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    launchctl(&["bootstrap", &domain, &path.to_string_lossy()])
        .map_err(|e| format!("launchctl bootstrap failed: {e}"))?;
    println!("Installed {}", path.display());
    println!("Kevro will start at login (and has been started now) from {}", exe.display());
    Ok(())
}

/// Unload and remove the LaunchAgent. This also stops a Kevro started by it.
pub fn uninstall() -> Result<(), String> {
    let path = plist_path();
    let domain = gui_domain()?;
    let _ = launchctl(&["bootout", &format!("{domain}/{LABEL}")]);
    match std::fs::remove_file(&path) {
        Ok(()) => println!("Removed {}", path.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => println!("Not installed"),
        Err(e) => return Err(e.to_string()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_escapes_and_contains_label() {
        let p = plist_contents(Path::new("/Apps/A&B <x>.app/Contents/MacOS/kevro"));
        assert!(p.contains("<string>io.github.lenard-0.kevro</string>"));
        assert!(p.contains("/Apps/A&amp;B &lt;x&gt;.app/Contents/MacOS/kevro"));
        assert!(p.contains("<key>RunAtLoad</key>\n    <true/>"));
    }
}
