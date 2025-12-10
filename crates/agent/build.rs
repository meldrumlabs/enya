fn main() {
    // Export base metadata (features, rustc/llvm version, etc.).
    enya_build_tools::export_build_info_vars_for_crate(env!("CARGO_PKG_NAME"));

    // Override the git-related fields so we get useful info even when building locally.
    if let Ok(hash) = enya_build_tools::git_commit_hash() {
        println!("cargo:rustc-env=ENYA_BUILD_GIT_HASH={hash}");
    }
    if let Ok(branch) = enya_build_tools::git_branch() {
        println!("cargo:rustc-env=ENYA_BUILD_GIT_BRANCH={branch}");
    }

    // Always include a build timestamp.
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    println!("cargo:rustc-env=ENYA_BUILD_DATETIME={timestamp}");
}
