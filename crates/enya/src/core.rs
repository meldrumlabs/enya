use build_info::BuildInfo;

#[derive(Clone)]
pub struct Core {
    build_info: BuildInfo,
}

impl Core {
    pub fn new(build_info: BuildInfo) -> Self {
        Self { build_info }
    }

    pub fn build_info(&self) -> BuildInfo {
        self.build_info
    }
}
