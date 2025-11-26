fn main() {
    // Export base metadata (features, rustc/llvm version, etc.).
    re_build_tools::export_build_info_vars_for_crate(env!("CARGO_PKG_NAME"));

    // Override the git-related fields so we get useful info even when building locally.
    if let Ok(hash) = re_build_tools::git_commit_hash() {
        println!("cargo:rustc-env=RE_BUILD_GIT_HASH={hash}");
    }
    if let Ok(branch) = re_build_tools::git_branch() {
        println!("cargo:rustc-env=RE_BUILD_GIT_BRANCH={branch}");
    }

    // Always include a build timestamp and mark that we're outside the rerun workspace.
    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    println!("cargo:rustc-env=RE_BUILD_DATETIME={timestamp}");
    println!("cargo:rustc-env=RE_BUILD_IS_IN_RERUN_WORKSPACE=no");
}
