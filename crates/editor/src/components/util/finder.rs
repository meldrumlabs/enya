//! Generic fuzzy finder component for modal search interfaces.
//!
//! This module provides a reusable `Finder<T>` component that implements the common
//! patterns shared across all finder-style modals (metrics finder, workspace finder, etc.).
//!
//! # Overview
//!
//! The `Finder<T>` component provides:
//! - **Fuzzy matching** via the `nucleo` crate for fast, typo-tolerant search
//! - **Keyboard navigation** with vim-style bindings (j/k, Ctrl+n/p) and arrows
//! - **Consistent styling** using the shared overlay style system
//! - **Highlighted matches** showing which characters matched the query
//! - **Optional preview pane** for showing additional item details
//!
//! # Usage
//!
//! To create a finder for your custom type, implement the [`FinderItem`] trait:
//!
//! ```ignore
//! use crate::components::util::finder::{Finder, FinderItem, FinderConfig};
//!
//! #[derive(Clone)]
//! struct MyItem {
//!     name: String,
//!     category: String,
//! }
//!
//! impl FinderItem for MyItem {
//!     fn search_text(&self) -> &str {
//!         &self.name
//!     }
//!
//!     fn icon(&self) -> &'static str {
//!         semantic_icons::file::FILE
//!     }
//!
//!     fn secondary_text(&self) -> Option<String> {
//!         Some(format!("[{}]", self.category))
//!     }
//! }
//!
//! // Create the finder with configuration
//! let config = FinderConfig {
//!     placeholder: "Search items...",
//!     icon: semantic_icons::action::SEARCH,
//!     show_preview: false,
//!     empty_message: "No items found",
//!     no_items_message: "No items available",
//! };
//!
//! let mut finder: Finder<MyItem> = Finder::new(config);
//! finder.set_items(vec![...]);
//! finder.open();
//!
//! // In your render loop:
//! if let Some(selected) = finder.show(ctx) {
//!     // Handle selection
//! }
//! ```
//!
//! # Architecture
//!
//! The finder is designed around the principle of composition over inheritance:
//!
//! - [`FinderItem`] - Trait defining how items are displayed and searched
//! - [`FinderConfig`] - Configuration for appearance and behavior
//! - [`Finder<T>`] - The generic finder component
//! - [`FinderResult<T>`] - A matched item with score and match positions
//!
//! For finders that need custom preview panes or additional UI, you can either:
//! 1. Wrap `Finder<T>` and add custom rendering around it
//! 2. Use the lower-level utilities (`FinderKeyboardInput`, `create_highlighted_text`, etc.)
//!
//! # Keyboard Shortcuts
//!
//! The finder supports the following keyboard shortcuts:
//!
//! | Key | Action |
//! |-----|--------|
//! | `↑` / `Ctrl+K` | Navigate up |
//! | `↓` / `Ctrl+J` / `Ctrl+N` | Navigate down |
//! | `Enter` | Confirm selection |
//! | `Escape` | Close finder |
//! | `Ctrl+P` | Toggle preview pane |

use egui::{Color32, RichText};
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};

use crate::ui::colors::text_color;
use crate::ui::palette;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

use super::finder_utils::{
    FinderColors, FinderKeyboardInput, OverlayStyle, create_highlighted_text, render_keyboard_hints,
};

// =============================================================================
// FinderItem Trait
// =============================================================================

/// Trait for items that can be searched and displayed in a [`Finder`].
///
/// Implement this trait for any type you want to use with the generic finder.
/// The trait provides the information needed for fuzzy matching and display.
///
/// # Example
///
/// ```ignore
/// #[derive(Clone)]
/// struct Metric {
///     name: String,
///     category: String,
/// }
///
/// impl FinderItem for Metric {
///     fn search_text(&self) -> &str {
///         &self.name
///     }
///
///     fn icon(&self) -> &'static str {
///         semantic_icons::chart::LINE
///     }
///
///     fn secondary_text(&self) -> Option<String> {
///         Some(format!("[{}]", self.category))
///     }
/// }
/// ```
pub trait FinderItem: Clone {
    /// Returns the primary text used for fuzzy matching.
    ///
    /// This is the text that will be searched when the user types a query.
    /// It's also displayed as the main label in the results list.
    fn search_text(&self) -> &str;

    /// Returns the icon to display next to the item.
    ///
    /// Should return a Phosphor icon string (e.g., from `semantic_icons`).
    fn icon(&self) -> &'static str;

    /// Returns optional secondary text to display after the main text.
    ///
    /// This is typically used for categories, descriptions, or metadata.
    /// The text is displayed in a muted color after the main search text.
    ///
    /// Returns `None` by default, which displays no secondary text.
    fn secondary_text(&self) -> Option<String> {
        None
    }
}

// =============================================================================
// FinderConfig
// =============================================================================

/// Configuration for a [`Finder`] instance.
///
/// This struct controls the appearance and behavior of the finder modal.
///
/// # Example
///
/// ```ignore
/// let config = FinderConfig {
///     placeholder: "Search workspaces...",
///     icon: semantic_icons::file::FOLDER_OPEN,
///     show_preview: false,
///     empty_message: "No matching workspaces",
///     no_items_message: "No saved workspaces",
/// };
/// ```
#[derive(Clone)]
pub struct FinderConfig {
    /// Placeholder text shown in the search input when empty.
    ///
    /// Example: "Search metrics...", "Find workspace..."
    pub placeholder: &'static str,

    /// Icon displayed in the search input header.
    ///
    /// Should be a Phosphor icon string (e.g., `semantic_icons::action::SEARCH`).
    pub icon: &'static str,

    /// Whether to show the preview pane on the right side.
    ///
    /// When `true`, the finder displays a two-column layout with results
    /// on the left and a preview pane on the right. The preview pane
    /// content is controlled by the `render_preview` callback.
    ///
    /// Default: `false`
    pub show_preview: bool,

    /// Message shown when no results match the current query.
    ///
    /// Example: "No results found", "No matching metrics"
    pub empty_message: &'static str,

    /// Message shown when there are no items at all.
    ///
    /// This is different from `empty_message` - it's shown when the
    /// finder has no items to search through, not just no matches.
    ///
    /// Example: "No saved workspaces", "Connect to load metrics"
    pub no_items_message: &'static str,
}

impl Default for FinderConfig {
    fn default() -> Self {
        Self {
            placeholder: "Search...",
            icon: "", // Will use search icon
            show_preview: false,
            empty_message: "No results found",
            no_items_message: "No items available",
        }
    }
}

// =============================================================================
// FinderResult
// =============================================================================

/// A fuzzy match result containing the matched item, score, and match positions.
///
/// This struct is returned by the finder's internal matching logic and contains
/// all the information needed to render a highlighted result row.
#[derive(Debug, Clone)]
pub struct FinderResult<T: FinderItem> {
    /// The matched item.
    pub item: T,

    /// The fuzzy match score (higher is better).
    ///
    /// Scores are computed by the `nucleo` matcher and consider factors like:
    /// - How many characters matched
    /// - Whether matches are consecutive
    /// - Whether matches are at word boundaries
    pub score: i64,

    /// Character positions in `search_text()` that matched the query.
    ///
    /// These positions are used to highlight the matching characters
    /// in the results list.
    pub match_positions: Vec<usize>,
}

// =============================================================================
// Finder<T>
// =============================================================================

/// A generic fuzzy finder modal for searching and selecting items.
///
/// `Finder<T>` provides a complete, reusable implementation of the telescope/fzf-style
/// finder pattern used throughout the editor. It handles:
///
/// - Fuzzy matching with the `nucleo` crate
/// - Keyboard navigation (arrows, vim keys, enter/escape)
/// - Consistent visual styling with the overlay system
/// - Match highlighting in results
/// - Optional preview pane support
///
/// # Type Parameters
///
/// - `T`: The item type, which must implement [`FinderItem`] and `Clone`
///
/// # Example
///
/// ```ignore
/// // Create and configure the finder
/// let mut finder: Finder<MetricItem> = Finder::new(FinderConfig {
///     placeholder: "Search metrics...",
///     icon: semantic_icons::action::SEARCH,
///     show_preview: true,
///     empty_message: "No matching metrics",
///     no_items_message: "Connect to load metrics",
/// });
///
/// // Populate with items
/// finder.set_items(metrics);
///
/// // Open the finder
/// finder.open();
///
/// // In your render loop:
/// if let Some(selected_metric) = finder.show(ctx) {
///     // User selected a metric
///     handle_selection(selected_metric);
/// }
/// ```
pub struct Finder<T: FinderItem> {
    /// Current search query entered by the user.
    query: String,

    /// All searchable items.
    items: Vec<T>,

    /// Filtered and scored results based on current query.
    results: Vec<FinderResult<T>>,

    /// Index of the currently selected result (0-based).
    selected_index: usize,

    /// Whether the finder modal is currently visible.
    is_open: bool,

    /// Current UI theme (affects colors and styling).
    theme: AppTheme,

    /// The nucleo fuzzy matcher instance.
    matcher: Matcher,

    /// Flag indicating results need to be refreshed.
    needs_refresh: bool,

    /// Configuration for this finder instance.
    config: FinderConfig,
}

impl<T: FinderItem> Default for Finder<T> {
    fn default() -> Self {
        Self::new(FinderConfig::default())
    }
}

impl<T: FinderItem> Finder<T> {
    /// Creates a new finder with the given configuration.
    ///
    /// The finder starts in a closed state with no items. Use [`set_items`](Self::set_items)
    /// to populate it and [`open`](Self::open) to show it.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let finder = Finder::new(FinderConfig {
    ///     placeholder: "Search...",
    ///     ..Default::default()
    /// });
    /// ```
    pub fn new(config: FinderConfig) -> Self {
        Self {
            query: String::new(),
            items: Vec::new(),
            results: Vec::new(),
            selected_index: 0,
            is_open: false,
            theme: AppTheme::default(),
            matcher: Matcher::new(Config::DEFAULT),
            needs_refresh: true,
            config,
        }
    }

    /// Sets the UI theme for styling.
    ///
    /// Call this when the application theme changes to update colors.
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Returns `true` if the finder is currently visible.
    pub fn is_open(&self) -> bool {
        self.is_open
    }

    /// Opens the finder modal.
    ///
    /// This clears any previous query, resets the selection to the first item,
    /// and marks results for refresh.
    pub fn open(&mut self) {
        self.is_open = true;
        self.query.clear();
        self.selected_index = 0;
        self.needs_refresh = true;
    }

    /// Closes the finder modal.
    ///
    /// This also clears the query and resets the selection.
    pub fn close(&mut self) {
        self.is_open = false;
        self.query.clear();
        self.selected_index = 0;
    }

    /// Sets the items to search through.
    ///
    /// This replaces any existing items and marks results for refresh.
    /// The finder will re-filter on the next frame.
    ///
    /// # Example
    ///
    /// ```ignore
    /// finder.set_items(vec![
    ///     MyItem { name: "foo".into(), ... },
    ///     MyItem { name: "bar".into(), ... },
    /// ]);
    /// ```
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.needs_refresh = true;
    }

    /// Sets the query text programmatically.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.needs_refresh = true;
    }

    /// Returns a reference to the current items.
    pub fn items(&self) -> &[T] {
        &self.items
    }

    /// Returns a mutable reference to the current items.
    ///
    /// After modifying items, call `mark_needs_refresh()` to update results.
    pub fn items_mut(&mut self) -> &mut Vec<T> {
        &mut self.items
    }

    /// Marks that results need to be refreshed.
    ///
    /// Call this after modifying items via `items_mut()`.
    pub fn mark_needs_refresh(&mut self) {
        self.needs_refresh = true;
    }

    /// Returns a reference to the current filtered results.
    pub fn results(&self) -> &[FinderResult<T>] {
        &self.results
    }

    /// Returns the currently selected result, if any.
    pub fn selected(&self) -> Option<&FinderResult<T>> {
        self.results.get(self.selected_index)
    }

    /// Returns the currently selected item, if any.
    pub fn selected_item(&self) -> Option<&T> {
        self.selected().map(|r| &r.item)
    }

    /// Returns the current selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Returns whether the preview pane is enabled.
    pub fn show_preview(&self) -> bool {
        self.config.show_preview
    }

    /// Toggles the preview pane visibility.
    pub fn toggle_preview(&mut self) {
        self.config.show_preview = !self.config.show_preview;
    }

    /// Returns the current theme.
    pub fn theme(&self) -> AppTheme {
        self.theme
    }

    /// Refreshes the filtered results based on the current query.
    ///
    /// This is called automatically by `show()` when `needs_refresh` is true.
    #[profiling::function]
    fn refresh_results(&mut self) {
        self.results.clear();

        if self.query.is_empty() {
            // Show all items when query is empty, sorted alphabetically
            for item in &self.items {
                self.results.push(FinderResult {
                    item: item.clone(),
                    score: 0,
                    match_positions: Vec::new(),
                });
            }
            // Sort alphabetically by search text
            self.results
                .sort_by(|a, b| a.item.search_text().cmp(b.item.search_text()));
        } else {
            // Parse the query into a pattern for fuzzy matching
            let pattern = Pattern::new(
                &self.query,
                CaseMatching::Ignore,
                Normalization::Smart,
                AtomKind::Fuzzy,
            );

            // Fuzzy match and score items
            let mut indices: Vec<u32> = Vec::new();
            let mut buf = Vec::new();
            for item in &self.items {
                indices.clear();
                let haystack = Utf32Str::new(item.search_text(), &mut buf);

                if let Some(score) = pattern.indices(haystack, &mut self.matcher, &mut indices) {
                    self.results.push(FinderResult {
                        item: item.clone(),
                        score: i64::from(score),
                        match_positions: indices.iter().map(|&i| i as usize).collect(),
                    });
                }
            }
            // Sort by score descending (best matches first)
            self.results.sort_by(|a, b| b.score.cmp(&a.score));
        }

        // Reset selection if it's out of bounds
        if self.selected_index >= self.results.len() {
            self.selected_index = 0;
        }

        self.needs_refresh = false;
    }

    /// Shows the finder modal and handles user interaction.
    ///
    /// Returns `Some(item)` if the user selected an item this frame,
    /// or `None` if the finder is closed or no selection was made.
    ///
    /// # Example
    ///
    /// ```ignore
    /// if let Some(selected) = finder.show(ctx) {
    ///     // User selected an item
    ///     println!("Selected: {}", selected.search_text());
    /// }
    /// ```
    #[profiling::function]
    pub fn show(&mut self, ctx: &egui::Context) -> Option<T> {
        self.show_with_preview(ctx, |_, _, _| {})
    }

    /// Shows the finder modal with a custom preview pane renderer.
    ///
    /// The `render_preview` callback is called to render the preview pane content
    /// when `config.show_preview` is true and an item is selected.
    ///
    /// # Arguments
    ///
    /// - `ctx`: The egui context
    /// - `render_preview`: Callback to render preview content. Receives:
    ///   - `ui`: The egui UI for the preview pane
    ///   - `result`: The currently selected result
    ///   - `colors`: Theme-aware colors for styling
    ///
    /// # Example
    ///
    /// ```ignore
    /// let selected = finder.show_with_preview(ctx, |ui, result, colors| {
    ///     ui.label(&result.item.name);
    ///     ui.label(&result.item.description);
    /// });
    /// ```
    #[profiling::function]
    pub fn show_with_preview<F>(&mut self, ctx: &egui::Context, render_preview: F) -> Option<T>
    where
        F: FnOnce(&mut egui::Ui, &FinderResult<T>, &FinderColors),
    {
        if !self.is_open {
            return None;
        }

        // Refresh results if needed
        if self.needs_refresh {
            self.refresh_results();
        }

        let mut selected_item: Option<T> = None;
        let mut should_close = false;
        let mut toggle_preview = false;

        // Handle keyboard input
        let input = FinderKeyboardInput::read(ctx);

        if input.escape {
            should_close = true;
        }

        if input.toggle_preview {
            toggle_preview = true;
        }

        if input.navigate_up && self.selected_index > 0 {
            self.selected_index -= 1;
        }

        if input.navigate_down && self.selected_index + 1 < self.results.len() {
            self.selected_index += 1;
        }

        if input.confirm && !self.results.is_empty() {
            selected_item = Some(self.results[self.selected_index].item.clone());
            should_close = true;
        }

        // Calculate popup dimensions
        let screen_rect = ctx.available_rect();
        let list_width = if self.config.show_preview {
            (screen_rect.width() * 0.35).clamp(300.0, 425.0)
        } else {
            (screen_rect.width() * 0.70).clamp(600.0, 850.0)
        };
        let preview_width = if self.config.show_preview {
            (screen_rect.width() * 0.35).clamp(300.0, 425.0)
        } else {
            0.0
        };
        let total_width = list_width + preview_width;
        let popup_max_height = (screen_rect.height() * 0.65).min(550.0);

        // Get the currently selected result for preview
        let selected_result_for_preview = if !self.results.is_empty() {
            Some(self.results[self.selected_index].clone())
        } else {
            None
        };

        let colors = FinderColors::new(self.theme);
        let overlay_style = OverlayStyle::frosted_glass(self.theme);

        egui::Area::new(egui::Id::new("finder_popup"))
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                overlay_style.frame().show(ui, |ui| {
                    ui.set_width(total_width);
                    ui.set_max_height(popup_max_height);

                    // Search input section
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(self.config.icon)
                                .color(text_color(self.theme).gamma_multiply(0.6))
                                .size(typography::HEADING),
                        );
                        ui.add_space(8.0);

                        let text_edit = egui::TextEdit::singleline(&mut self.query)
                            .font(typography::heading())
                            .hint_text(
                                RichText::new(self.config.placeholder)
                                    .color(text_color(self.theme).gamma_multiply(0.4)),
                            )
                            .frame(false)
                            .desired_width(total_width - 60.0);

                        let response = ui.add(text_edit);

                        // Request focus on the text input
                        response.request_focus();

                        // Check if query changed
                        if response.changed() {
                            self.needs_refresh = true;
                            self.selected_index = 0;
                        }
                    });

                    ui.add_space(8.0);

                    // Separator below search
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );
                    ui.add_space(4.0);

                    // Main content area
                    let content_height = popup_max_height - 90.0;

                    if self.config.show_preview {
                        // Two-column layout: results + preview
                        ui.horizontal(|ui| {
                            // Results list (left side)
                            ui.vertical(|ui| {
                                ui.set_width(list_width);
                                ui.set_height(content_height);
                                self.render_results_list(
                                    ui,
                                    content_height,
                                    list_width,
                                    &colors,
                                    &mut selected_item,
                                    &mut should_close,
                                );
                            });

                            // Vertical separator
                            let line_rect = ui.available_rect_before_wrap();
                            ui.painter().vline(
                                line_rect.left(),
                                line_rect.y_range(),
                                egui::Stroke::new(1.0, colors.separator),
                            );

                            // Preview pane (right side)
                            ui.vertical(|ui| {
                                ui.set_width(preview_width);
                                ui.set_height(content_height);

                                egui::Frame::new()
                                    .fill(colors.preview_bg)
                                    .inner_margin(12.0)
                                    .show(ui, |ui| {
                                        ui.set_min_height(content_height - 24.0);

                                        if let Some(ref result) = selected_result_for_preview {
                                            render_preview(ui, result, &colors);
                                        } else {
                                            // No selection placeholder
                                            ui.centered_and_justified(|ui| {
                                                ui.label(
                                                    RichText::new("Select an item to preview")
                                                        .color(
                                                            text_color(self.theme)
                                                                .gamma_multiply(0.4),
                                                        )
                                                        .italics(),
                                                );
                                            });
                                        }
                                    });
                            });
                        });
                    } else {
                        // Single column: results only
                        self.render_results_list(
                            ui,
                            content_height,
                            list_width,
                            &colors,
                            &mut selected_item,
                            &mut should_close,
                        );
                    }

                    ui.add_space(4.0);

                    // Footer with keyboard hints
                    ui.painter().hline(
                        ui.available_rect_before_wrap().x_range(),
                        ui.cursor().top(),
                        egui::Stroke::new(1.0, colors.separator),
                    );
                    ui.add_space(6.0);
                    render_keyboard_hints(ui, text_color(self.theme).gamma_multiply(0.4));
                    ui.add_space(8.0);
                });
            });

        if toggle_preview {
            self.toggle_preview();
        }

        if should_close {
            self.close();
        }

        selected_item
    }

    /// Renders the scrollable results list.
    fn render_results_list(
        &self,
        ui: &mut egui::Ui,
        content_height: f32,
        list_width: f32,
        colors: &FinderColors,
        selected_item: &mut Option<T>,
        should_close: &mut bool,
    ) {
        egui::ScrollArea::vertical()
            .max_height(content_height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(list_width - 8.0);

                if self.results.is_empty() {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        let message = if self.items.is_empty() {
                            self.config.no_items_message
                        } else {
                            self.config.empty_message
                        };
                        ui.label(
                            RichText::new(message)
                                .color(text_color(self.theme).gamma_multiply(0.5))
                                .size(typography::XL),
                        );
                    });
                    ui.add_space(20.0);
                } else {
                    for (i, result) in self.results.iter().enumerate() {
                        let is_selected = i == self.selected_index;
                        let clicked = self.render_result_row(ui, result, is_selected, colors);
                        if clicked {
                            *selected_item = Some(result.item.clone());
                            *should_close = true;
                        }
                    }
                }
            });
    }

    /// Renders a single result row.
    fn render_result_row(
        &self,
        ui: &mut egui::Ui,
        result: &FinderResult<T>,
        is_selected: bool,
        colors: &FinderColors,
    ) -> bool {
        let text_col = text_color(self.theme);
        let accent_col = match self.theme {
            AppTheme::Light => palette::accent::LIGHT,
            AppTheme::Dark => palette::accent::PRIMARY,
        };
        let secondary_color = text_col.gamma_multiply(0.5);

        let row_height = 36.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        let is_hovered = response.hovered();

        // Background - use subtle hover style like landing page
        let bg_color = if is_selected {
            accent_col.gamma_multiply(0.12)
        } else if is_hovered {
            text_col.gamma_multiply(0.05)
        } else {
            Color32::TRANSPARENT
        };

        if bg_color != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, 0.0, bg_color);
        }

        // Selection indicator bar
        if is_selected {
            let indicator_rect = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, row_height));
            ui.painter().rect_filled(indicator_rect, 0.0, accent_col);
        }

        // Content layout
        let content_rect = rect.shrink2(egui::vec2(16.0, 0.0));
        let mut cursor_x = content_rect.left();

        // Icon - use accent color on hover/select like landing page
        let icon_color = if is_selected || is_hovered {
            accent_col
        } else {
            text_col.gamma_multiply(0.6)
        };
        let icon_galley = ui.painter().layout_no_wrap(
            result.item.icon().to_string(),
            typography::proportional(typography::XL),
            icon_color,
        );
        ui.painter().galley(
            egui::pos2(
                cursor_x,
                content_rect.center().y - icon_galley.size().y / 2.0,
            ),
            icon_galley.clone(),
            icon_color,
        );
        cursor_x += icon_galley.size().x + 10.0;

        // Main text with highlighted matches
        let search_text = result.item.search_text();
        let text_galley = create_highlighted_text(
            ui,
            search_text,
            &result.match_positions,
            text_col,
            colors.highlight,
        );
        ui.painter().galley(
            egui::pos2(
                cursor_x,
                content_rect.center().y - text_galley.size().y / 2.0,
            ),
            text_galley.clone(),
            text_col,
        );
        cursor_x += text_galley.size().x + 12.0;

        // Secondary text (if any)
        if let Some(secondary) = result.item.secondary_text() {
            let remaining_width = content_rect.right() - cursor_x;
            let secondary_galley = ui.painter().layout_no_wrap(
                secondary,
                typography::proportional(typography::SM),
                secondary_color,
            );

            if secondary_galley.size().x <= remaining_width {
                ui.painter().galley(
                    egui::pos2(
                        cursor_x,
                        content_rect.center().y - secondary_galley.size().y / 2.0,
                    ),
                    secondary_galley,
                    secondary_color,
                );
            }
        }

        // Scroll selected item into view
        if is_selected {
            response.scroll_to_me(Some(egui::Align::Center));
        }

        response.clicked()
    }
}
