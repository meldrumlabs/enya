//! Build tools for enya crates.
//!
//! This crate provides utilities for generating build information environment variables
//! that can be consumed by the `build_info` crate.

use std::process::Command;

/// Export build info environment variables for use by the `build_info!` macro.
///
/// This function should be called from your `build.rs` file.
/// It exports the following environment variables with the `ENYA_BUILD_` prefix:
/// - `ENYA_BUILD_FEATURES` - Space-separated list of enabled features
/// - `ENYA_BUILD_RUSTC_VERSION` - Rust compiler version
/// - `ENYA_BUILD_LLVM_VERSION` - LLVM version
/// - `ENYA_BUILD_GIT_HASH` - Git commit hash (empty if not in a git repo)
/// - `ENYA_BUILD_GIT_BRANCH` - Git branch name (empty if not in a git repo)
/// - `ENYA_BUILD_IS_IN_ENYA_WORKSPACE` - "yes" if in enya workspace, "no" otherwise
/// - `ENYA_BUILD_TARGET_TRIPLE` - Target triple (e.g., "x86_64-unknown-linux-gnu")
/// - `ENYA_BUILD_DATETIME` - ISO 8601 build timestamp
pub fn export_build_info_vars_for_crate(crate_name: &str) {
    // Features
    let features = enabled_features(crate_name);
    println!("cargo:rustc-env=ENYA_BUILD_FEATURES={features}");

    // Rust compiler version
    let rustc_version = rustc_version().unwrap_or_default();
    println!("cargo:rustc-env=ENYA_BUILD_RUSTC_VERSION={rustc_version}");

    // LLVM version
    let llvm_version = llvm_version().unwrap_or_default();
    println!("cargo:rustc-env=ENYA_BUILD_LLVM_VERSION={llvm_version}");

    // Git info
    let git_hash = git_commit_hash().unwrap_or_default();
    println!("cargo:rustc-env=ENYA_BUILD_GIT_HASH={git_hash}");

    let git_branch = git_branch().unwrap_or_default();
    println!("cargo:rustc-env=ENYA_BUILD_GIT_BRANCH={git_branch}");

    // Workspace detection
    let is_in_workspace = is_in_enya_workspace();
    println!(
        "cargo:rustc-env=ENYA_BUILD_IS_IN_ENYA_WORKSPACE={}",
        if is_in_workspace { "yes" } else { "no" }
    );

    // Target triple
    let target = std::env::var("TARGET").unwrap_or_default();
    println!("cargo:rustc-env=ENYA_BUILD_TARGET_TRIPLE={target}");

    // Build datetime - empty by default, can be overridden by caller
    println!("cargo:rustc-env=ENYA_BUILD_DATETIME=");
}

/// Get the current git commit hash.
pub fn git_commit_hash() -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("git rev-parse failed".to_string())
    }
}

/// Get the current git branch name.
pub fn git_branch() -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("git rev-parse --abbrev-ref failed".to_string())
    }
}

/// Get the rustc version string.
fn rustc_version() -> Option<String> {
    let output = Command::new("rustc").arg("--version").output().ok()?;
    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout);
        // Extract just the version part, e.g., "rustc 1.76.0 (d5c2e9c34 2023-09-13)"
        // becomes "1.76.0 (d5c2e9c34 2023-09-13)"
        version.strip_prefix("rustc ").map(|s| s.trim().to_string())
    } else {
        None
    }
}

/// Get the LLVM version string from rustc.
fn llvm_version() -> Option<String> {
    let output = Command::new("rustc")
        .args(["--version", "--verbose"])
        .output()
        .ok()?;

    if output.status.success() {
        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines() {
            if let Some(version) = line.strip_prefix("LLVM version: ") {
                return Some(version.trim().to_string());
            }
        }
    }
    None
}

/// Check if we're building within the enya workspace.
fn is_in_enya_workspace() -> bool {
    // Check if we can find the workspace root with enya in the name
    let Ok(metadata) = cargo_metadata::MetadataCommand::new().exec() else {
        return false;
    };

    // Check if the workspace root contains "enya"
    metadata
        .workspace_root
        .to_string()
        .to_lowercase()
        .contains("enya")
}

/// Get the enabled features for a crate.
fn enabled_features(crate_name: &str) -> String {
    let Ok(metadata) = cargo_metadata::MetadataCommand::new().exec() else {
        return String::new();
    };

    // Find the package
    let Some(package) = metadata.packages.iter().find(|p| p.name == crate_name) else {
        return String::new();
    };

    // Get enabled features from environment variables
    // Cargo sets CARGO_FEATURE_<name> for each enabled feature
    package
        .features
        .keys()
        .filter(|feature| {
            let env_name = format!("CARGO_FEATURE_{}", feature.to_uppercase().replace('-', "_"));
            std::env::var(&env_name).is_ok()
        })
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}
