use egui::{
    Color32, FontId, Frame, Stroke, TextEdit, TextFormat, Vec2,
    text::{LayoutJob, LayoutSection},
};
use pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd, TextMergeWithOffset,
};
use std::ops::Range;
use std::rc::Rc;
use std::{cell::RefCell, sync::Arc};
use syntect::{
    highlighting::{HighlightState, Highlighter, ThemeSet},
    parsing::{ParseState, SyntaxSet},
};

use crate::google;

struct AsyncState {
    access_token: Option<String>,
    status: Option<String>,
    busy: bool,
    pending_file_id: Option<String>,
    pending_content: Option<(String, String)>, // (content, file_id)
}

impl Default for AsyncState {
    fn default() -> Self {
        Self {
            access_token: None,
            status: None,
            busy: false,
            pending_file_id: None,
            pending_content: None,
        }
    }
}

impl AsyncState {
    async fn get_access_token(state: Rc<RefCell<AsyncState>>) -> Result<String, String> {
        if let Some(res) = state.borrow().access_token.clone() {
            return Ok(res);
        }

        google::get_access_token().await.map(move |res| {
            state.borrow_mut().access_token = Some(res.clone());
            res
        })
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct App {
    text: String,
    file_name: String,
    file_id: Option<String>,
    #[serde(skip)]
    async_state: Rc<RefCell<AsyncState>>,
    #[serde(skip)]
    open_url_input: String,

    #[serde(skip)]
    syntax_set: SyntaxSet,
    #[serde(skip)]
    theme_set: ThemeSet,
}

impl Default for App {
    fn default() -> Self {
        Self {
            text: "Hello World!".to_owned(),
            file_name: "untitled.md".to_owned(),
            file_id: None,
            async_state: Rc::new(RefCell::new(AsyncState::default())),
            open_url_input: String::new(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }
}

fn text_format(heading_level: Option<HeadingLevel>, emphasis: bool, strong: bool) -> TextFormat {
    let font_regular = FontId::monospace(14.);
    let font_h1 = FontId::monospace(28.);
    let font_h2 = FontId::monospace(26.);
    let font_h3 = FontId::monospace(24.);
    let font_h4 = FontId::monospace(22.);
    let font_h5 = FontId::monospace(20.);
    let font_h6 = FontId::monospace(18.);

    let font_id = heading_level.map_or(font_regular, |h| match h {
        HeadingLevel::H1 => font_h1,
        HeadingLevel::H2 => font_h2,
        HeadingLevel::H3 => font_h3,
        HeadingLevel::H4 => font_h4,
        HeadingLevel::H5 => font_h5,
        HeadingLevel::H6 => font_h6,
    });

    let color = if heading_level.is_some() {
        Color32::LIGHT_BLUE
    } else {
        Color32::WHITE
    };

    let underline = if strong {
        Stroke { color, width: 1. }
    } else {
        Stroke::NONE
    };

    return TextFormat {
        font_id,
        color,
        underline,
        italics: emphasis,
        expand_bg: 0.,
        ..Default::default()
    };
}

fn code_layout(
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    sections: &mut Vec<LayoutSection>,
    language: &str,
    code: &str,
    code_range: Range<usize>,
) {
    let font_id = FontId::monospace(14.);
    let color = Color32::WHITE;

    let text_format = TextFormat {
        font_id,
        color,
        expand_bg: 0.,
        ..Default::default()
    };

    let Some(syntax) = syntax_set.find_syntax_by_token(language) else {
        sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: code_range.clone(),
            format: text_format.clone(),
        });
        return;
    };

    let theme = &theme_set.themes["base16-ocean.dark"];
    let highlighter = Highlighter::new(theme);
    let mut highlight_state =
        HighlightState::new(&highlighter, syntect::parsing::ScopeStack::new());
    let mut parse_state = ParseState::new(syntax);

    let mut rest = code;
    let mut last_end = code_range.start;
    loop {
        let (line, line_range) = {
            if let Some(idx) = rest.find('\n') {
                let (a, b) = rest.split_at(idx + 1); // '\n' included
                let range = Range {
                    start: last_end,
                    end: last_end + a.len(),
                };
                last_end = range.end;
                rest = b;
                (a, range)
            } else {
                let range = Range {
                    start: last_end,
                    end: last_end + rest.len(),
                };
                (rest, range)
            }
        };

        if let Ok(ops) = parse_state.parse_line(line, &syntax_set) {
            let iter = syntect::highlighting::RangedHighlightIterator::new(
                &mut highlight_state,
                &ops[..],
                line,
                &highlighter,
            );

            for (style, s, token_range_in_line) in iter {
                let syntect::highlighting::Color { r, g, b, a } = style.foreground;
                let color = Color32::from_rgba_unmultiplied(r, g, b, a);
                let token_range = Range {
                    start: line_range.start + token_range_in_line.start,
                    end: line_range.start + token_range_in_line.end,
                };

                sections.push(LayoutSection {
                    leading_space: 0.0,
                    byte_range: token_range.clone(),
                    format: TextFormat {
                        color,
                        ..text_format.clone()
                    },
                });

                log::info!("{:#?}", (style, s, token_range))
            }
        } else {
            sections.push(LayoutSection {
                leading_space: 0.0,
                byte_range: line_range.clone(),
                format: text_format.clone(),
            });
        }

        assert!(line_range.end <= code_range.end);
        if line_range.end >= code_range.end {
            break;
        }
    }
}

fn layouter(
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    ui: &egui::Ui,
    buf: &dyn egui::TextBuffer,
    wrap_width: f32,
) -> Arc<egui::Galley> {
    let text = String::from(buf.as_str());

    let iterator = TextMergeWithOffset::new(Parser::new(buf.as_str()).into_offset_iter());

    let mut sections = Vec::new();

    let mut debug_tags = Vec::new();

    // Dunno if they could really nest. Using the top one
    let mut heading_stack = Vec::new();
    let mut code_stack = Vec::new();

    let mut strong_depth: u32 = 0;
    let mut emphasis_depth: u32 = 0;
    let mut _quote_depth: u32 = 0; // TODO: Make a background around quotes

    let mut last_end: usize = 0;
    for event in iterator {
        debug_tags.push(event.clone());

        let text_format = text_format(
            heading_stack.last().copied(),
            emphasis_depth > 0,
            strong_depth > 0,
        );

        let range = event.1;

        // Close previous range
        {
            let current_end = match event.0 {
                Event::End(_) => range.end,
                _ => range.start,
            };

            if current_end > last_end {
                sections.push(LayoutSection {
                    leading_space: 0.0,
                    byte_range: Range {
                        start: last_end,
                        end: current_end,
                    },
                    format: text_format.clone(),
                });
                last_end = current_end;
            }
        }

        match event.0 {
            Event::Start(Tag::Heading { level, .. }) => {
                heading_stack.push(level);
            }
            Event::End(TagEnd::Heading(_)) => {
                heading_stack.pop();
                ()
            }

            Event::Start(Tag::CodeBlock(code_block)) => {
                code_stack.push(code_block.clone());
            }
            Event::End(TagEnd::CodeBlock) => {
                code_stack.pop();
                ()
            }

            Event::Start(Tag::BlockQuote { .. }) => {
                _quote_depth += 1;
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                _quote_depth -= 1;
            }

            Event::Start(Tag::Strong { .. }) => {
                strong_depth += 1;
            }
            Event::End(TagEnd::Strong) => {
                strong_depth -= 1;
            }

            Event::Start(Tag::Emphasis { .. }) => {
                emphasis_depth += 1;
            }
            Event::End(TagEnd::Emphasis) => {
                emphasis_depth -= 1;
            }

            Event::Text(str) => {
                if let Some(CodeBlockKind::Fenced(language)) = code_stack.last() {
                    code_layout(
                        syntax_set,
                        theme_set,
                        &mut sections,
                        language.as_ref(),
                        str.as_ref(),
                        range.clone(),
                    );
                } else {
                    sections.push(LayoutSection {
                        leading_space: 0.0,
                        byte_range: range.clone(),
                        format: text_format,
                    });
                }

                last_end = range.end;
            }
            _ => (),
        }
    }

    log::info!("{:#?}", debug_tags);

    {
        let text_format = text_format(
            heading_stack.last().copied(),
            emphasis_depth > 0,
            strong_depth > 0,
        );

        if last_end < text.len() {
            sections.push(LayoutSection {
                leading_space: 0.0,
                byte_range: Range {
                    start: last_end,
                    end: text.len(),
                },
                format: text_format,
            })
        }
    }

    let mut layout_job = LayoutJob {
        sections,
        text,
        wrap: egui::text::TextWrapping {
            max_width: wrap_width,
            ..Default::default()
        },
        break_on_newline: true,
        ..Default::default()
    };

    layout_job.wrap.max_width = wrap_width;
    return ui.fonts_mut(|f| f.layout_job(layout_job));
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }

    fn start_save(&self, ctx: &egui::Context, force_new: bool) {
        let state = self.async_state.clone();
        let ctx = ctx.clone();
        let content = self.text.clone();
        let filename = self.file_name.clone();
        let file_id = if force_new {
            None
        } else {
            self.file_id.clone()
        };

        {
            let mut s = state.borrow_mut();
            s.busy = true;
            s.status = Some("Saving...".to_owned());
        }

        wasm_bindgen_futures::spawn_local(async move {
            let token = match AsyncState::get_access_token(state.clone()).await {
                Ok(t) => t,
                Err(e) => {
                    let mut s = state.borrow_mut();
                    s.busy = false;
                    s.status = Some(format!("Auth failed: {e}"));
                    ctx.request_repaint();
                    return;
                }
            };

            match google::save_file(&token, &content, &filename, file_id.as_deref()).await {
                Ok(result) => {
                    let mut s = state.borrow_mut();
                    s.busy = false;
                    s.status = Some(format!("Saved as \"{}\"", result.name));
                    s.pending_file_id = Some(result.file_id);
                }
                Err(e) => {
                    let mut s = state.borrow_mut();
                    s.busy = false;
                    s.status = Some(format!("Save failed: {e}"));
                }
            }
            ctx.request_repaint();
        });
    }

    fn start_open(&self, ctx: &egui::Context, drive_file_id: String) {
        let state = self.async_state.clone();
        let ctx = ctx.clone();

        {
            let mut s = state.borrow_mut();
            s.busy = true;
            s.status = Some("Opening...".to_owned());
        }

        wasm_bindgen_futures::spawn_local(async move {
            let token = match AsyncState::get_access_token(state.clone()).await {
                Ok(t) => t,
                Err(e) => {
                    let mut s = state.borrow_mut();
                    s.busy = false;
                    s.status = Some(format!("Auth failed: {e}"));
                    ctx.request_repaint();
                    return;
                }
            };

            match google::open_file(&token, &drive_file_id).await {
                Ok(content) => {
                    let mut s = state.borrow_mut();
                    s.busy = false;
                    s.status = Some("File opened".to_owned());
                    s.pending_content = Some((content, drive_file_id));
                }
                Err(e) => {
                    let mut s = state.borrow_mut();
                    s.busy = false;
                    s.status = Some(format!("Open failed: {e}"));
                }
            }
            ctx.request_repaint();
        });
    }
}

impl eframe::App for App {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Drain pending results from async tasks
        {
            let mut s = self.async_state.borrow_mut();
            if let Some(id) = s.pending_file_id.take() {
                self.file_id = Some(id);
            }
            if let Some((content, id)) = s.pending_content.take() {
                self.text = content;
                self.file_id = Some(id);
            }
        }

        egui::Panel::top("top_panel").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let busy = self.async_state.borrow().busy;

                ui.menu_button("File", |ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        if ui.button("Save to Drive").clicked() {
                            ui.close();
                            self.start_save(ui.ctx(), false);
                        }
                        if ui.button("Save as new").clicked() {
                            ui.close();
                            self.start_save(ui.ctx(), true);
                        }
                    });

                    ui.separator();

                    ui.label("Open from Drive URL:");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.open_url_input);
                        if ui.add_enabled(!busy, egui::Button::new("Open")).clicked() {
                            if let Some(id) = google::extract_file_id(&self.open_url_input) {
                                ui.close();
                                self.start_open(ui.ctx(), id);
                            } else {
                                self.async_state.borrow_mut().status =
                                    Some("Invalid Drive URL".to_owned());
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                ui.add(
                    egui::TextEdit::singleline(&mut self.file_name)
                        .desired_width(200.0)
                        .hint_text("filename.md"),
                );

                ui.add_space(16.0);
                egui::widgets::global_theme_preference_buttons(ui);
            });

            // Status message
            if let Some(status) = self.async_state.borrow().status.clone() {
                ui.label(status);
            }
        });

        egui::Panel::bottom("botton_panel").show_inside(ui, |ui| {
            ui.label("Some status");
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    egui::ScrollArea::vertical()
                        .wheel_scroll_multiplier(Vec2::splat(2.))
                        .show(ui, |ui| {
                            let mut layouter_closure =
                                |ui: &egui::Ui,
                                 buf: &dyn egui::TextBuffer,
                                 wrap_width: f32|
                                 -> Arc<egui::Galley> {
                                    layouter(&self.syntax_set, &self.theme_set, ui, buf, wrap_width)
                                };

                            TextEdit::multiline(&mut self.text)
                                .code_editor()
                                .layouter(&mut layouter_closure)
                                .frame(Frame::new())
                                .show(ui);
                        })
                },
            );
        });
    }
}
