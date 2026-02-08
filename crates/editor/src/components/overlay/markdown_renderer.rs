//! Markdown renderer for agent panel messages.
//!
//! Parses markdown using `pulldown-cmark` and renders it with egui primitives,
//! reusing the existing theme system and typography constants. Supports:
//! - Paragraphs, headings (H1-H3)
//! - Bold, italic, strikethrough, inline code
//! - Fenced code blocks with language labels
//! - Ordered and unordered lists (nested)
//! - Blockquotes with accent left bar
//! - Horizontal rules
//! - Links (rendered as colored text)

use egui::{CornerRadius, RichText, Stroke, TextFormat, text::LayoutJob};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::components::util::SyntaxHighlightData;
use crate::ui::theme::AppTheme;
use crate::ui::typography;

/// Render markdown content into an egui UI.
pub fn render_markdown(ui: &mut egui::Ui, text: &str, theme: AppTheme) {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(text, options);

    let mut ctx = RenderContext {
        theme,
        bold: false,
        italic: false,
        strikethrough: false,
        inline_code: false,
        link: false,
        heading: None,
        list_stack: Vec::new(),
        in_code_block: false,
        code_block_lang: None,
        code_block_buf: String::new(),
        blockquote_depth: 0,
        paragraph_job: None,
        code_block_counter: 0,
    };

    for event in parser {
        ctx.handle_event(ui, event);
    }

    // Flush any remaining paragraph
    ctx.flush_paragraph(ui);
}

struct RenderContext {
    theme: AppTheme,
    bold: bool,
    italic: bool,
    strikethrough: bool,
    inline_code: bool,
    link: bool,
    heading: Option<HeadingLevel>,
    list_stack: Vec<Option<u64>>,
    in_code_block: bool,
    code_block_lang: Option<String>,
    code_block_buf: String,
    blockquote_depth: u32,
    paragraph_job: Option<LayoutJob>,
    code_block_counter: usize,
}

impl RenderContext {
    fn handle_event(&mut self, ui: &mut egui::Ui, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.handle_start(ui, tag),
            Event::End(tag_end) => self.handle_end(ui, tag_end),
            Event::Text(text) => self.handle_text(ui, &text),
            Event::Code(code) => self.handle_inline_code(&code),
            Event::SoftBreak => self.append_to_job(" "),
            Event::HardBreak => self.append_to_job("\n"),
            Event::Rule => {
                self.flush_paragraph(ui);
                ui.add_space(4.0);
                let rect = ui.available_rect_before_wrap();
                let y = rect.top();
                ui.painter().line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                    Stroke::new(1.0, self.theme.border_subtle()),
                );
                ui.add_space(8.0);
            }
            _ => {}
        }
    }

    fn handle_start(&mut self, ui: &mut egui::Ui, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                self.paragraph_job = Some(LayoutJob::default());
            }
            Tag::Heading { level, .. } => {
                self.heading = Some(level);
                self.paragraph_job = Some(LayoutJob::default());
            }
            Tag::BlockQuote(_) => {
                self.blockquote_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.flush_paragraph(ui);
                let lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        let l = lang.to_string();
                        if l.is_empty() { None } else { Some(l) }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                self.in_code_block = true;
                self.code_block_lang = lang;
                self.code_block_buf.clear();
            }
            Tag::List(first_item) => {
                self.flush_paragraph(ui);
                self.list_stack.push(first_item);
            }
            Tag::Item => {
                self.paragraph_job = Some(LayoutJob::default());
            }
            Tag::Emphasis => self.italic = true,
            Tag::Strong => self.bold = true,
            Tag::Strikethrough => self.strikethrough = true,
            Tag::Link { .. } => self.link = true,
            _ => {}
        }
    }

    fn handle_end(&mut self, ui: &mut egui::Ui, tag_end: TagEnd) {
        match tag_end {
            TagEnd::Paragraph => {
                self.flush_paragraph(ui);
                ui.add_space(4.0);
            }
            TagEnd::Heading(_) => {
                self.flush_heading(ui);
                self.heading = None;
                ui.add_space(2.0);
            }
            TagEnd::BlockQuote(_) => {
                self.blockquote_depth = self.blockquote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                self.render_code_block(ui);
                self.in_code_block = false;
                self.code_block_lang = None;
                self.code_block_buf.clear();
                ui.add_space(4.0);
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    ui.add_space(4.0);
                }
            }
            TagEnd::Item => self.flush_list_item(ui),
            TagEnd::Emphasis => self.italic = false,
            TagEnd::Strong => self.bold = false,
            TagEnd::Strikethrough => self.strikethrough = false,
            TagEnd::Link => self.link = false,
            _ => {}
        }
    }

    fn handle_text(&mut self, _ui: &mut egui::Ui, text: &str) {
        if self.in_code_block {
            self.code_block_buf.push_str(text);
        } else {
            // Compute format before borrowing paragraph_job
            let format = self.current_text_format();
            if let Some(job) = &mut self.paragraph_job {
                job.append(text, 0.0, format);
            }
        }
    }

    fn handle_inline_code(&mut self, code: &str) {
        let size = self.text_size();
        let format = TextFormat {
            font_id: typography::monospace(size),
            color: self.theme.accent_primary(),
            background: self.theme.bg_elevated(),
            ..Default::default()
        };
        if let Some(job) = &mut self.paragraph_job {
            job.append(&format!(" {code} "), 0.0, format);
        }
    }

    fn append_to_job(&mut self, s: &str) {
        let format = self.current_text_format();
        if let Some(job) = &mut self.paragraph_job {
            job.append(s, 0.0, format);
        }
    }

    fn text_size(&self) -> f32 {
        match self.heading {
            Some(HeadingLevel::H1) => typography::HEADING,
            Some(HeadingLevel::H2) => typography::XL,
            Some(HeadingLevel::H3) => typography::LG,
            _ => typography::MD,
        }
    }

    fn current_text_format(&self) -> TextFormat {
        let size = self.text_size();
        let font_id = if self.inline_code {
            typography::monospace(size)
        } else {
            typography::proportional(size)
        };

        let color = if self.link {
            self.theme.accent_primary()
        } else {
            self.theme.text_primary()
        };

        let mut format = TextFormat {
            font_id,
            color,
            ..Default::default()
        };

        if self.italic {
            format.italics = true;
        }
        if self.strikethrough {
            format.strikethrough = Stroke::new(1.0, color);
        }
        if self.inline_code {
            format.background = self.theme.bg_elevated();
        }

        format
    }

    fn flush_paragraph(&mut self, ui: &mut egui::Ui) {
        if let Some(job) = self.paragraph_job.take() {
            if job.is_empty() {
                return;
            }

            if self.blockquote_depth > 0 {
                let accent = self.theme.accent_primary();
                let indent = self.blockquote_depth as f32 * 12.0;
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    let rect = ui.available_rect_before_wrap();
                    let bar_top = egui::pos2(rect.left() - 6.0, rect.top());
                    let response = ui.label(job);
                    let bar =
                        egui::Rect::from_min_size(bar_top, egui::vec2(2.0, response.rect.height()));
                    ui.painter()
                        .rect_filled(bar, CornerRadius::ZERO, accent.linear_multiply(0.5));
                });
            } else {
                ui.label(job);
            }
        }
    }

    fn flush_heading(&mut self, ui: &mut egui::Ui) {
        if let Some(mut job) = self.paragraph_job.take() {
            if job.is_empty() {
                return;
            }
            for section in &mut job.sections {
                section.format.color = self.theme.text_primary();
            }
            ui.add_space(4.0);
            ui.label(job);
        }
    }

    fn flush_list_item(&mut self, ui: &mut egui::Ui) {
        if let Some(job) = self.paragraph_job.take() {
            let depth = self.list_stack.len().saturating_sub(1);
            let indent = 12.0 + depth as f32 * 16.0;

            let bullet = if let Some(entry) = self.list_stack.last_mut() {
                match entry {
                    Some(counter) => {
                        let s = format!("{counter}.");
                        *counter += 1;
                        s
                    }
                    None => {
                        if depth % 2 == 0 {
                            "•".to_string()
                        } else {
                            "◦".to_string()
                        }
                    }
                }
            } else {
                "•".to_string()
            };

            ui.horizontal(|ui| {
                ui.add_space(indent);
                ui.label(
                    RichText::new(bullet)
                        .color(self.theme.text_tertiary())
                        .size(typography::MD),
                );
                ui.add_space(4.0);
                ui.label(job);
            });
        }
    }

    fn render_code_block(&mut self, ui: &mut egui::Ui) {
        let text = self.code_block_buf.trim_end_matches('\n');
        if text.is_empty() {
            return;
        }

        let bg = self.theme.bg_elevated();
        let border = self.theme.border_subtle();
        let text_secondary = self.theme.text_secondary();
        let lang = self.code_block_lang.as_deref().unwrap_or("");

        // Compute syntax highlighting for the code block content
        let highlight_data = SyntaxHighlightData::new(text, lang);

        // Unique ID for this code block's copy state
        let block_id = ui.id().with("md_code_copy").with(self.code_block_counter);
        self.code_block_counter += 1;
        let code_text = text.to_string();

        egui::Frame::new()
            .fill(bg)
            .corner_radius(CornerRadius::same(6))
            .stroke(Stroke::new(1.0, border))
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                // Header row: language label + copy button
                ui.horizontal(|ui| {
                    if !lang.is_empty() {
                        egui::Frame::new()
                            .fill(self.theme.bg_surface())
                            .corner_radius(CornerRadius::same(3))
                            .inner_margin(egui::Margin::symmetric(5, 1))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(lang)
                                        .color(text_secondary)
                                        .size(typography::XS)
                                        .font(typography::monospace(typography::XS)),
                                );
                            });
                    }

                    // Push copy button to the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Check if recently copied (within 1.5s)
                        let copied_time: Option<f64> =
                            ui.ctx().data(|d| d.get_temp::<f64>(block_id));
                        let now = ui.input(|i| i.time);
                        let just_copied = copied_time.is_some_and(|t| now - t < 1.5);

                        let (label, color) = if just_copied {
                            (
                                format!("{} Copied!", egui_nerdfonts::regular::CHECK),
                                self.theme.accent_primary(),
                            )
                        } else {
                            (
                                egui_nerdfonts::regular::CONTENT_COPY.to_string(),
                                text_secondary,
                            )
                        };

                        let btn = ui.add(
                            egui::Button::new(
                                RichText::new(label).size(typography::XS).color(color),
                            )
                            .frame(false),
                        );

                        if btn.clicked() {
                            ui.ctx().copy_text(code_text.clone());
                            ui.ctx().data_mut(|d| d.insert_temp::<f64>(block_id, now));
                        }

                        // Request repaint while "Copied!" is showing
                        if just_copied {
                            ui.ctx().request_repaint();
                        }
                    });
                });

                if !lang.is_empty() {
                    ui.add_space(4.0);
                }

                // Render each line with syntax highlighting
                for (i, line) in text.lines().enumerate() {
                    let job = highlight_data.highlight_line(i + 1, line, self.theme);
                    ui.label(job);
                }
            });
    }
}
