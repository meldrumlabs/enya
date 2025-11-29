//! Information about the build of a Rust crate.
//!
//! To use this you also need to call `enya_build_tools::export_env_vars()` from your build.rs.

mod build_info;
mod crate_version;

pub use build_info::BuildInfo;
pub use crate_version::CrateVersion;

/// Create a [`BuildInfo`] at compile-time using environment variables exported by
/// calling `enya_build_tools::export_env_vars()` from your build.rs.
#[macro_export]
macro_rules! build_info {
    () => {
        $crate::BuildInfo {
            crate_name: env!("CARGO_PKG_NAME"),
            features: env!("ENYA_BUILD_FEATURES"),
            version: $crate::CrateVersion::parse(env!("CARGO_PKG_VERSION")),
            rustc_version: env!("ENYA_BUILD_RUSTC_VERSION"),
            llvm_version: env!("ENYA_BUILD_LLVM_VERSION"),
            git_hash: env!("ENYA_BUILD_GIT_HASH"),
            git_branch: env!("ENYA_BUILD_GIT_BRANCH"),
            // TODO: `PartialEq` is not available in const contexts, so this won't actually
            // build if you try to instantiate a BuildInfo in a constant.
            is_in_enya_workspace: env!("ENYA_BUILD_IS_IN_ENYA_WORKSPACE") == "yes",
            target_triple: env!("ENYA_BUILD_TARGET_TRIPLE"),
            datetime: env!("ENYA_BUILD_DATETIME"),
        }
    };
}
