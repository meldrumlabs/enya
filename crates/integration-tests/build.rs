fn main() {
    enya_build_tools::export_build_info_vars_for_crate(env!("CARGO_PKG_NAME"));

    if let Ok(hash) = enya_build_tools::git_commit_hash() {
        println!("cargo:rustc-env=ENYA_BUILD_GIT_HASH={hash}");
    }
    if let Ok(branch) = enya_build_tools::git_branch() {
        println!("cargo:rustc-env=ENYA_BUILD_GIT_BRANCH={branch}");
    }

    let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    println!("cargo:rustc-env=ENYA_BUILD_DATETIME={timestamp}");
}
