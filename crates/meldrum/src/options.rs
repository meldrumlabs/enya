/// Default dircetory for storing meldrum data
pub const DEFAULT_DATA_DIR: &str = "/opt/meldrum";

pub struct Options {
    /// Directory where metrics and logs are stored
    data_dir: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            data_dir: DEFAULT_DATA_DIR.to_string(),
        }
    }
}
