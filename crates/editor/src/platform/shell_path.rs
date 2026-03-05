//! Resolve the user's login shell PATH for macOS GUI apps.
//!
//! When Enya.app is launched from Finder, Dock, or Spotlight, macOS gives the
//! process a minimal PATH (`/usr/bin:/bin:/usr/sbin:/sbin`). Developer tools
//! like `npx`, `git`, and `node` live in directories added by Homebrew, nvm,
//! volta, etc. — none of which are in the default GUI PATH.
//!
//! This module runs the user's login shell once at startup to discover their
//! real PATH and sets it on the process environment.

use std::process::Command;
use std::time::Duration;

/// Marker prefix used to extract PATH from shell output.
const PATH_MARKER: &str = "ENYA_PATH=";

/// Directories commonly added by developer toolchains on macOS.
/// Used as a fallback when shell resolution fails.
const FALLBACK_DIRS: &[&str] = &[
    "/opt/homebrew/bin",
    "/opt/homebrew/sbin",
    "/usr/local/bin",
    "/usr/local/sbin",
];

/// Resolve the user's login shell PATH and update the process environment.
///
/// macOS GUI apps inherit a minimal PATH that excludes Homebrew, nvm, volta,
/// and other developer tool directories. This function runs the user's login
/// shell to discover the real PATH and sets it on the process.
///
/// # Safety contract
///
/// Must be called from `main()` / `run_native_app()` **before** any threads
/// are spawned, so the `std::env::set_var` call has no concurrent readers.
pub fn resolve_shell_environment() {
    let current_path = std::env::var("PATH").unwrap_or_default();

    // If PATH already contains typical developer directories, we were likely
    // launched from a terminal and don't need to resolve anything.
    if path_looks_rich(&current_path) {
        return;
    }

    // Try to resolve the full PATH from the user's login shell.
    if let Some(resolved) = resolve_path_from_shell() {
        if !resolved.is_empty() && resolved != current_path {
            // SAFETY: Called before any threads are spawned (before tokio
            // runtime creation), so there are no concurrent env readers.
            unsafe {
                std::env::set_var("PATH", &resolved);
            }
            return;
        }
    }

    // Fallback: prepend well-known directories to the existing PATH.
    let home = std::env::var("HOME").unwrap_or_default();
    let mut dirs: Vec<String> = FALLBACK_DIRS.iter().map(|d| (*d).to_string()).collect();

    // Add user-specific tool directories if HOME is set.
    if !home.is_empty() {
        dirs.push(format!("{home}/.volta/bin"));
        dirs.push(format!("{home}/.cargo/bin"));
        dirs.push(format!("{home}/.nodenv/shims"));
    }

    // Only prepend directories that actually exist on disk.
    let existing: Vec<String> = dirs
        .into_iter()
        .filter(|d| std::path::Path::new(d).is_dir())
        .collect();

    if !existing.is_empty() {
        let mut merged = existing.join(":");
        if !current_path.is_empty() {
            merged.push(':');
            merged.push_str(&current_path);
        }
        // SAFETY: Called before any threads are spawned.
        unsafe {
            std::env::set_var("PATH", &merged);
        }
    }
}

/// Check if the PATH already contains directories that indicate a rich
/// developer environment (i.e., launched from a terminal).
fn path_looks_rich(path: &str) -> bool {
    path.contains("/opt/homebrew/bin")
        || path.contains("/usr/local/bin")
        || path.contains("/.volta/bin")
        || path.contains("/.cargo/bin")
        || path.contains("/.nvm/")
}

/// Run the user's login shell to resolve their real PATH.
///
/// Returns `Some(path_string)` on success, `None` on failure or timeout.
fn resolve_path_from_shell() -> Option<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    // Verify the shell binary exists before trying to run it.
    if !std::path::Path::new(&shell).exists() {
        return None;
    }

    // Use a unique marker to reliably extract PATH from shell output,
    // since login shells may print banners or other text to stdout.
    let print_cmd = format!("printf '{PATH_MARKER}%s' \"$PATH\"");

    let child = Command::new(&shell)
        .args(["-l", "-c", &print_cmd])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // Wait with a timeout to avoid blocking on slow shell configs.
    let output = wait_with_timeout(child, Duration::from_secs(5))?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;

    // Find the marker in the output and extract the PATH after it.
    let marker_pos = stdout.rfind(PATH_MARKER)?;
    let path = &stdout[marker_pos + PATH_MARKER.len()..];

    if path.is_empty() {
        return None;
    }

    Some(path.to_string())
}

/// Wait for a child process with a timeout.
///
/// If the process doesn't finish in time, it is killed.
fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Option<std::process::Output> {
    let start = std::time::Instant::now();
    let poll_interval = Duration::from_millis(50);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                // Process exited — collect output.
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        let mut buf = Vec::new();
                        std::io::Read::read_to_end(&mut s, &mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();

                return Some(std::process::Output {
                    status,
                    stdout,
                    stderr: Vec::new(),
                });
            }
            Ok(None) => {
                // Still running.
                if start.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(poll_interval);
            }
            Err(_) => {
                let _ = child.kill();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_looks_rich_detects_homebrew() {
        assert!(path_looks_rich(
            "/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        ));
    }

    #[test]
    fn path_looks_rich_detects_usr_local() {
        assert!(path_looks_rich("/usr/local/bin:/usr/bin:/bin"));
    }

    #[test]
    fn path_looks_rich_detects_volta() {
        assert!(path_looks_rich(
            "/Users/me/.volta/bin:/usr/bin:/bin:/usr/sbin:/sbin"
        ));
    }

    #[test]
    fn path_looks_rich_returns_false_for_minimal() {
        assert!(!path_looks_rich("/usr/bin:/bin:/usr/sbin:/sbin"));
    }

    #[test]
    fn path_looks_rich_returns_false_for_empty() {
        assert!(!path_looks_rich(""));
    }

    #[test]
    fn resolve_path_from_shell_returns_something() {
        // This test runs the actual login shell, so it may be slow.
        // It should work on any macOS dev machine.
        if let Some(path) = resolve_path_from_shell() {
            assert!(!path.is_empty());
            // Should contain at least the basic system paths
            assert!(path.contains("/usr/bin"));
        }
        // If resolve_path_from_shell returns None, that's OK in CI
        // environments where $SHELL may not be set.
    }
}
