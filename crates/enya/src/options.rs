/// Default directory for storing enya data
pub const DEFAULT_DATA_DIR: &str = "/tmp/enya";

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

impl Options {
    /// Returns the directory used to store metrics and logs.
    #[must_use]
    pub fn data_dir(&self) -> &str {
        &self.data_dir
    }
}
