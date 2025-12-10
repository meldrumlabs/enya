use egui::{Color32, RichText};

use crate::theme::AppTheme;
use crate::ui::colors::text_color;
use crate::ui::semantic_icons;

/// A user-defined custom query
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CustomQuery {
    /// Unique identifier
    pub id: u64,
    /// User-defined name for the query
    pub name: String,
    /// The query string (e.g., "env:prod AND service:db")
    pub query: String,
    /// Hierarchical tags (e.g., ["production/api", "critical"])
    #[serde(default)]
    pub tags: Vec<String>,
}

impl CustomQuery {
    pub fn new(id: u64, name: impl Into<String>, query: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            query: query.into(),
            tags: Vec::new(),
        }
    }

    /// Create a new query with tags
    pub fn with_tags(
        id: u64,
        name: impl Into<String>,
        query: impl Into<String>,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            query: query.into(),
            tags,
        }
    }

    /// Add a tag to this query
    pub fn add_tag(&mut self, tag: impl Into<String>) {
        let tag = tag.into();
        if !self.tags.contains(&tag) {
            self.tags.push(tag);
        }
    }

    /// Remove a tag from this query
    pub fn remove_tag(&mut self, tag: &str) {
        self.tags.retain(|t| t != tag);
    }

    /// Check if this query has a specific tag (or is a descendant of it)
    pub fn has_tag(&self, tag_prefix: &str) -> bool {
        self.tags
            .iter()
            .any(|t| t == tag_prefix || t.starts_with(&format!("{tag_prefix}/")))
    }
}

/// Panel for managing custom queries
pub struct CustomQueriesPanel {
    /// List of saved custom queries
    queries: Vec<CustomQuery>,
    /// Current theme
    theme: AppTheme,
    /// Next ID for new queries
    next_id: u64,
    /// Currently editing query (id, name, query)
    editing: Option<(u64, String, String)>,
    /// Whether we're adding a new query
    adding_new: bool,
    /// New query name (when adding)
    new_name: String,
    /// New query string (when adding)
    new_query: String,
    /// Pending chart to add (query id)
    pending_chart: Option<u64>,
    /// Currently selected query id
    selected: Option<u64>,
    /// Filter text (set externally from Dashboard)
    filter: String,
    /// Active tag filter (set externally from Dashboard)
    tag_filter: Option<String>,
}

impl Default for CustomQueriesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl CustomQueriesPanel {
    pub fn new() -> Self {
        Self {
            queries: Vec::new(),
            theme: AppTheme::default(),
            next_id: 1,
            editing: None,
            adding_new: false,
            new_name: String::new(),
            new_query: String::new(),
            pending_chart: None,
            selected: None,
            filter: String::new(),
            tag_filter: None,
        }
    }

    /// Create with demo queries for testing
    pub fn with_demo_queries() -> Self {
        let mut panel = Self::new();
        panel.add_query_with_tags(
            "Prod DB Latency",
            "env:prod AND service:db",
            vec!["production/db".to_string(), "latency".to_string()],
        );
        panel.add_query_with_tags(
            "API Errors",
            "status:5xx AND service:api",
            vec!["production/api".to_string(), "critical".to_string()],
        );
        panel.add_query_with_tags(
            "High Memory",
            "memory_usage > 80%",
            vec!["production".to_string(), "critical".to_string()],
        );
        panel
    }

    /// Set the theme
    pub fn set_theme(&mut self, theme: AppTheme) {
        self.theme = theme;
    }

    /// Set the filter text (for external filtering from Dashboard)
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
    }

    /// Set the tag filter (for filtering by hierarchical tags)
    pub fn set_tag_filter(&mut self, tag: Option<&str>) {
        self.tag_filter = tag.map(|s| s.to_string());
    }

    /// Get the current tag filter
    pub fn tag_filter(&self) -> Option<&str> {
        self.tag_filter.as_deref()
    }

    /// Check if a query matches the current filter (text and tag)
    fn matches_filter(&self, query: &CustomQuery) -> bool {
        // Check text filter
        if !self.filter.is_empty() {
            let filter_lower = self.filter.to_lowercase();
            let matches_text = query.name.to_lowercase().contains(&filter_lower)
                || query.query.to_lowercase().contains(&filter_lower);
            if !matches_text {
                return false;
            }
        }

        // Check tag filter
        if let Some(ref tag_filter) = self.tag_filter {
            if !query.has_tag(tag_filter) {
                return false;
            }
        }

        true
    }

    /// Check if there are any queries matching the current filter
    pub fn has_matching_queries(&self) -> bool {
        if self.filter.is_empty() {
            return !self.queries.is_empty();
        }
        self.queries.iter().any(|q| self.matches_filter(q))
    }

    /// Add a new query
    pub fn add_query(&mut self, name: impl Into<String>, query: impl Into<String>) {
        self.queries
            .push(CustomQuery::new(self.next_id, name, query));
        self.next_id += 1;
    }

    /// Add a new query with tags
    pub fn add_query_with_tags(
        &mut self,
        name: impl Into<String>,
        query: impl Into<String>,
        tags: Vec<String>,
    ) {
        self.queries
            .push(CustomQuery::with_tags(self.next_id, name, query, tags));
        self.next_id += 1;
    }

    /// Add a tag to a query by id
    pub fn add_tag_to_query(&mut self, query_id: u64, tag: &str) {
        if let Some(query) = self.queries.iter_mut().find(|q| q.id == query_id) {
            query.add_tag(tag);
        }
    }

    /// Remove a tag from a query by id
    pub fn remove_tag_from_query(&mut self, query_id: u64, tag: &str) {
        if let Some(query) = self.queries.iter_mut().find(|q| q.id == query_id) {
            query.remove_tag(tag);
        }
    }

    /// Get all unique tags across all queries
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .queries
            .iter()
            .flat_map(|q| q.tags.iter().cloned())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Remove a query by id
    pub fn remove_query(&mut self, id: u64) {
        self.queries.retain(|q| q.id != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
    }

    /// Get a query by id
    pub fn get_query(&self, id: u64) -> Option<&CustomQuery> {
        self.queries.iter().find(|q| q.id == id)
    }

    /// Get all queries
    pub fn queries(&self) -> &[CustomQuery] {
        &self.queries
    }

    /// Take the pending chart request (returns query id if any)
    pub fn take_pending_chart(&mut self) -> Option<u64> {
        self.pending_chart.take()
    }

    /// Get the currently selected query
    pub fn selected(&self) -> Option<u64> {
        self.selected
    }

    /// Render the panel (content only, no header wrapper)
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let text_color = text_color(self.theme);
        self.show_content(ui, text_color);
    }

    fn show_content(&mut self, ui: &mut egui::Ui, text_color: Color32) {
        // Add new query button
        ui.horizontal(|ui| {
            ui.add_space(16.0);
            if ui
                .button(RichText::new(format!(
                    "{} New Query",
                    semantic_icons::action::ADD
                )))
                .clicked()
            {
                self.adding_new = true;
                self.new_name = "New Query".to_string();
                self.new_query = String::new();
            }
        });

        // New query form
        if self.adding_new {
            self.show_new_query_form(ui, text_color);
        }

        ui.add_space(4.0);

        // List of saved queries (filtered)
        let queries_snapshot: Vec<_> = self
            .queries
            .iter()
            .filter(|q| self.matches_filter(q))
            .cloned()
            .collect();
        for query in &queries_snapshot {
            self.show_query_item(ui, query, text_color);
        }

        if queries_snapshot.is_empty() && !self.adding_new {
            ui.horizontal(|ui| {
                ui.add_space(32.0);
                let message = if self.filter.is_empty() {
                    "No custom queries yet"
                } else {
                    "No matching queries"
                };
                ui.label(
                    RichText::new(message)
                        .color(text_color.gamma_multiply(0.5))
                        .italics()
                        .small(),
                );
            });
        }
    }

    fn show_new_query_form(&mut self, ui: &mut egui::Ui, text_color: Color32) {
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            ui.vertical(|ui| {
                // Name input
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Name:")
                            .color(text_color.gamma_multiply(0.7))
                            .small(),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_name)
                            .desired_width(150.0)
                            .hint_text("Query name"),
                    );
                });

                // Query input
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Query:")
                            .color(text_color.gamma_multiply(0.7))
                            .small(),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_query)
                            .desired_width(150.0)
                            .hint_text("env:prod AND service:db")
                            .font(egui::TextStyle::Monospace),
                    );
                });

                // Buttons
                ui.horizontal(|ui| {
                    if ui
                        .button(
                            RichText::new(format!("{} Save", semantic_icons::action::CONFIRM))
                                .color(egui::Color32::from_rgb(34, 197, 94)),
                        )
                        .clicked()
                    {
                        let name = self.new_name.trim().to_string();
                        let query = self.new_query.trim().to_string();
                        if !name.is_empty() {
                            self.add_query(name, query);
                            self.adding_new = false;
                            self.new_name.clear();
                            self.new_query.clear();
                        }
                    }

                    if ui
                        .button(
                            RichText::new(format!("{} Cancel", semantic_icons::action::CANCEL))
                                .color(text_color.gamma_multiply(0.7)),
                        )
                        .clicked()
                    {
                        self.adding_new = false;
                        self.new_name.clear();
                        self.new_query.clear();
                    }
                });
            });
        });
        ui.add_space(8.0);
        ui.separator();
    }

    fn show_query_item(&mut self, ui: &mut egui::Ui, query: &CustomQuery, text_color: Color32) {
        let is_selected = self.selected == Some(query.id);
        let is_editing = self
            .editing
            .as_ref()
            .is_some_and(|(id, _, _)| *id == query.id);

        if is_editing {
            self.show_edit_form(ui, query.id, text_color);
        } else {
            let mut open_chart = false;
            let mut start_edit = false;
            let mut delete = false;

            ui.horizontal(|ui| {
                ui.add_space(24.0);

                // Query icon
                ui.label(
                    RichText::new(semantic_icons::mode::COMMAND)
                        .color(text_color.gamma_multiply(0.6)),
                );

                // Selectable query name
                let response =
                    ui.selectable_label(is_selected, RichText::new(&query.name).color(text_color));

                if response.clicked() {
                    self.selected = Some(query.id);
                }

                if response.double_clicked() {
                    open_chart = true;
                }

                let is_hovered = response.hovered();

                // Show query string on hover
                response.on_hover_text(&query.query);

                // Action buttons (visible on hover or when selected)
                if is_selected || is_hovered {
                    // Open as chart
                    if ui
                        .small_button(semantic_icons::action::CHART)
                        .on_hover_text("Open as chart")
                        .clicked()
                    {
                        open_chart = true;
                    }

                    // Edit
                    if ui
                        .small_button(semantic_icons::action::EDIT)
                        .on_hover_text("Edit query")
                        .clicked()
                    {
                        start_edit = true;
                    }

                    // Delete
                    if ui
                        .small_button(
                            RichText::new(semantic_icons::action::DELETE)
                                .color(egui::Color32::from_rgb(239, 68, 68)),
                        )
                        .on_hover_text("Delete query")
                        .clicked()
                    {
                        delete = true;
                    }
                }
            });

            // Show tag chips below the query name (if any tags exist)
            if !query.tags.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(48.0); // Indent to align with query name
                    for tag in &query.tags {
                        self.show_tag_chip(ui, tag, text_color);
                    }
                });
            }

            // Handle actions outside the closure
            if open_chart {
                self.pending_chart = Some(query.id);
            }
            if start_edit {
                self.editing = Some((query.id, query.name.clone(), query.query.clone()));
            }
            if delete {
                self.remove_query(query.id);
            }
        }
    }

    /// Render a tag chip
    fn show_tag_chip(&self, ui: &mut egui::Ui, tag: &str, text_color: Color32) {
        let chip_bg = match self.theme {
            AppTheme::Light => Color32::from_rgb(230, 235, 245),
            AppTheme::Dark => Color32::from_rgb(50, 55, 70),
        };
        let chip_text = text_color.gamma_multiply(0.7);

        // Get short display name (last segment of path)
        let display_name = tag.rsplit('/').next().unwrap_or(tag);

        egui::Frame::new()
            .fill(chip_bg)
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(6, 2))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("#{display_name}"))
                        .color(chip_text)
                        .size(10.0),
                );
            })
            .response
            .on_hover_text(format!("Tag: {tag}"));
    }

    fn show_edit_form(&mut self, ui: &mut egui::Ui, query_id: u64, text_color: Color32) {
        let Some((_, ref mut name, ref mut query_str)) = self.editing else {
            return;
        };

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            ui.vertical(|ui| {
                // Name input
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Name:")
                            .color(text_color.gamma_multiply(0.7))
                            .small(),
                    );
                    ui.add(
                        egui::TextEdit::singleline(name)
                            .desired_width(150.0)
                            .hint_text("Query name"),
                    );
                });

                // Query input
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Query:")
                            .color(text_color.gamma_multiply(0.7))
                            .small(),
                    );
                    ui.add(
                        egui::TextEdit::singleline(query_str)
                            .desired_width(150.0)
                            .hint_text("env:prod AND service:db")
                            .font(egui::TextStyle::Monospace),
                    );
                });
            });
        });

        // Buttons (separate to avoid borrow issues)
        let mut save = false;
        let mut cancel = false;

        ui.horizontal(|ui| {
            ui.add_space(24.0);
            if ui
                .button(
                    RichText::new(format!("{} Save", semantic_icons::action::CONFIRM))
                        .color(egui::Color32::from_rgb(34, 197, 94)),
                )
                .clicked()
            {
                save = true;
            }

            if ui
                .button(
                    RichText::new(format!("{} Cancel", semantic_icons::action::CANCEL))
                        .color(text_color.gamma_multiply(0.7)),
                )
                .clicked()
            {
                cancel = true;
            }
        });
        ui.add_space(4.0);

        if save {
            if let Some((id, name, query_str)) = self.editing.take() {
                if let Some(q) = self.queries.iter_mut().find(|q| q.id == id) {
                    q.name = name.trim().to_string();
                    q.query = query_str.trim().to_string();
                }
            }
        }
        if cancel {
            self.editing = None;
        }

        // Keep the edit state for the id check
        if !save && !cancel {
            // Workaround: re-check if we're still editing this query
            if self
                .editing
                .as_ref()
                .is_some_and(|(id, _, _)| *id != query_id)
            {
                self.editing = None;
            }
        }
    }
}
