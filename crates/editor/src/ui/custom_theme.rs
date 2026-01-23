//! Custom theme support for plugin-defined themes.
//!
//! This module provides storage and resolution for custom color themes
//! defined by Lua plugins.

use egui::Color32;
use enya_plugin::{ThemeBase, ThemeColors, ThemeDefinition};
use rustc_hash::FxHashMap;

use super::theme::AppTheme;

/// Registry of custom themes from plugins.
#[derive(Debug, Default)]
pub struct CustomThemeStore {
    /// Map of theme name to definition
    themes: FxHashMap<String, ThemeDefinition>,
}

impl CustomThemeStore {
    /// Create a new empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a custom theme.
    pub fn register(&mut self, theme: ThemeDefinition) {
        log::info!(
            "[theme] Registered custom theme: {} ({})",
            theme.display_name,
            theme.name
        );
        self.themes.insert(theme.name.clone(), theme);
    }

    /// Get a theme by name.
    pub fn get(&self, name: &str) -> Option<&ThemeDefinition> {
        self.themes.get(name)
    }

    /// List all registered custom themes.
    pub fn list(&self) -> Vec<&ThemeDefinition> {
        self.themes.values().collect()
    }

    /// Get the number of registered themes.
    pub fn len(&self) -> usize {
        self.themes.len()
    }

    /// Check if there are no themes.
    pub fn is_empty(&self) -> bool {
        self.themes.is_empty()
    }

    /// Get custom theme names for the theme picker.
    pub fn theme_names(&self) -> Vec<(&str, &str)> {
        self.themes
            .values()
            .map(|t| (t.name.as_str(), t.display_name.as_str()))
            .collect()
    }
}

/// A resolved custom theme with all colors filled in.
///
/// Missing colors are inherited from the base theme (Dark or Light).
#[derive(Debug, Clone)]
pub struct ResolvedCustomTheme {
    /// Theme name
    pub name: String,
    /// Display name
    pub display_name: String,
    /// Whether this is a dark theme
    pub is_dark: bool,

    // Backgrounds
    pub bg_base: Color32,
    pub bg_surface: Color32,
    pub bg_elevated: Color32,

    // Text
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,

    // Accents
    pub accent_primary: Color32,
    pub accent_hover: Color32,
    pub accent_muted: Color32,

    // Borders
    pub border_subtle: Color32,
    pub border_strong: Color32,

    // Semantic colors
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub info: Color32,

    // Chart palette
    pub chart_palette: [Color32; 8],
}

impl ResolvedCustomTheme {
    /// Resolve a custom theme definition, falling back to base theme for missing colors.
    pub fn from_definition(def: &ThemeDefinition) -> Self {
        let base = match def.base {
            ThemeBase::Dark => AppTheme::Dark,
            ThemeBase::Light => AppTheme::Light,
        };

        let colors = &def.colors;

        // Helper to resolve a color, falling back to base theme
        let resolve = |custom: Option<u32>, fallback: Color32| -> Color32 {
            custom
                .map(|c| {
                    let (r, g, b) = ThemeColors::to_rgb(c);
                    Color32::from_rgb(r, g, b)
                })
                .unwrap_or(fallback)
        };

        // Resolve chart palette
        let mut chart_palette = base.chart_palette();
        for (i, &color) in colors.chart_palette.iter().enumerate().take(8) {
            let (r, g, b) = ThemeColors::to_rgb(color);
            chart_palette[i] = Color32::from_rgb(r, g, b);
        }

        Self {
            name: def.name.clone(),
            display_name: def.display_name.clone(),
            is_dark: matches!(def.base, ThemeBase::Dark),

            bg_base: resolve(colors.bg_base, base.bg_base()),
            bg_surface: resolve(colors.bg_surface, base.bg_surface()),
            bg_elevated: resolve(colors.bg_elevated, base.bg_elevated()),

            text_primary: resolve(colors.text_primary, base.text_primary()),
            text_secondary: resolve(colors.text_secondary, base.text_secondary()),
            text_muted: resolve(colors.text_muted, base.text_tertiary()),

            accent_primary: resolve(colors.accent_primary, base.accent_primary()),
            accent_hover: resolve(colors.accent_hover, base.accent_hover()),
            accent_muted: resolve(colors.accent_muted, base.accent_muted()),

            border_subtle: resolve(colors.border_subtle, base.border_subtle()),
            border_strong: resolve(colors.border_strong, base.border_default()),

            success: resolve(colors.success, base.semantic_success()),
            warning: resolve(colors.warning, base.semantic_warning()),
            error: resolve(colors.error, base.semantic_error()),
            info: resolve(colors.info, base.semantic_info()),

            chart_palette,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_register_and_get() {
        let mut store = CustomThemeStore::new();

        let theme = ThemeDefinition::new("test-theme", "Test Theme", ThemeBase::Dark);
        store.register(theme);

        assert_eq!(store.len(), 1);
        assert!(store.get("test-theme").is_some());
        assert!(store.get("nonexistent").is_none());
    }

    #[test]
    fn test_store_list() {
        let mut store = CustomThemeStore::new();

        store.register(ThemeDefinition::new("theme-1", "Theme 1", ThemeBase::Dark));
        store.register(ThemeDefinition::new("theme-2", "Theme 2", ThemeBase::Light));

        let themes = store.list();
        assert_eq!(themes.len(), 2);
    }

    #[test]
    fn test_resolved_theme_inherits_from_base() {
        let def = ThemeDefinition::new("minimal", "Minimal", ThemeBase::Dark);
        let resolved = ResolvedCustomTheme::from_definition(&def);

        // Should inherit from Dark theme
        assert!(resolved.is_dark);
        assert_eq!(resolved.bg_base, AppTheme::Dark.bg_base());
        assert_eq!(resolved.accent_primary, AppTheme::Dark.accent_primary());
    }

    #[test]
    fn test_resolved_theme_uses_custom_colors() {
        let mut def = ThemeDefinition::new("custom", "Custom", ThemeBase::Dark);
        def.colors.bg_base = Some(0x1a1b26);
        def.colors.accent_primary = Some(0x7aa2f7);

        let resolved = ResolvedCustomTheme::from_definition(&def);

        assert_eq!(resolved.bg_base, Color32::from_rgb(0x1a, 0x1b, 0x26));
        assert_eq!(resolved.accent_primary, Color32::from_rgb(0x7a, 0xa2, 0xf7));
        // Unset colors fall back to base
        assert_eq!(resolved.bg_surface, AppTheme::Dark.bg_surface());
    }

    #[test]
    fn test_theme_names() {
        let mut store = CustomThemeStore::new();

        store.register(ThemeDefinition::new(
            "tokyo",
            "Tokyo Night",
            ThemeBase::Dark,
        ));

        let names = store.theme_names();
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], ("tokyo", "Tokyo Night"));
    }
}
