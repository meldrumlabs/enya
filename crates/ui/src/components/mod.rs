use crate::theme::AppTheme;

pub mod metrics_tree;

pub use metrics_tree::{MetricCategory, MetricInfo, MetricSelection, MetricsTree};

/// Trait that defines an Enya Component
pub trait Component {
    /// The core function that is responsible for drawing the component
    fn show(&mut self, ui: &mut egui::Ui);
    /// Returns the identifier for the component
    fn id(&self) -> usize;
    /// Returns the name for the component (e.g., SQL)
    fn name(&self) -> String;
    /// Saves the current theme for the component
    fn set_theme(&mut self, theme: AppTheme);
    fn set_api_key(&mut self, key: &str);
    fn set_staging_api_key(&mut self, key: &str);
    /// Returns a RichText label for the given component
    fn label(&self) -> egui::RichText;
}
