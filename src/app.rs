use core::sync;
use egui::{
    Color32, FontId, TextEdit, TextFormat,
    text::{LayoutJob, LayoutSection},
};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TextMergeWithOffset};
use std::ops::Range;
use std::rc::Rc;
use std::{cell::RefCell, sync::Arc};

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
}

impl Default for App {
    fn default() -> Self {
        Self {
            text: "Hello World!".to_owned(),
            file_name: "untitled.md".to_owned(),
            file_id: None,
            async_state: Rc::new(RefCell::new(AsyncState::default())),
            open_url_input: String::new(),
        }
    }
}

fn layouter(ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32) -> Arc<egui::Galley> {
    let text = String::from(buf.as_str());

    let iterator = TextMergeWithOffset::new(Parser::new(buf.as_str()).into_offset_iter());

    let mut sections = Vec::new();

    let font_regular = FontId::monospace(14.);
    let font_h1 = FontId::monospace(28.);
    let font_h2 = FontId::monospace(26.);
    let font_h3 = FontId::monospace(24.);
    let font_h4 = FontId::monospace(22.);
    let font_h5 = FontId::monospace(20.);
    let font_h6 = FontId::monospace(18.);

    let mut tag_stack = Vec::new();

    let mut last_end: usize = 0;
    for event in iterator {
        match event {
            (Event::Start(tag), _) => {
                tag_stack.push(tag);
            }
            (Event::End(_), _) => {
                tag_stack.pop();
                ()
            }
            (Event::Text(_), range) => {
                let font = match tag_stack.last() {
                    Some(Tag::Heading {
                        level: HeadingLevel::H1,
                        ..
                    }) => &font_h1,
                    Some(Tag::Heading {
                        level: HeadingLevel::H2,
                        ..
                    }) => &font_h2,
                    Some(Tag::Heading {
                        level: HeadingLevel::H3,
                        ..
                    }) => &font_h3,
                    Some(Tag::Heading {
                        level: HeadingLevel::H4,
                        ..
                    }) => &font_h4,
                    Some(Tag::Heading {
                        level: HeadingLevel::H5,
                        ..
                    }) => &font_h5,
                    Some(Tag::Heading {
                        level: HeadingLevel::H6,
                        ..
                    }) => &font_h6,
                    _ => &font_regular,
                };

                let color = match tag_stack.last() {
                    Some(Tag::Heading { .. }) => Color32::GRAY,
                    _ => Color32::WHITE,
                };

                if range.start > last_end {
                    sections.push(LayoutSection {
                        leading_space: 0.0,
                        byte_range: Range {
                            start: last_end,
                            end: range.start,
                        },
                        format: TextFormat::simple(font.clone(), color),
                    })
                }
                last_end = range.end;

                sections.push(LayoutSection {
                    leading_space: 0.0,
                    byte_range: range,
                    format: TextFormat::simple(font.clone(), color),
                })
            }
            _ => (),
        }
    }

    if last_end < text.len() {
        sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: Range {
                start: last_end,
                end: text.len(),
            },
            format: TextFormat::simple(font_regular, Color32::WHITE),
        })
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

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    let layouter = &mut layouter;
                    let text_edit = TextEdit::multiline(&mut self.text)
                        .code_editor()
                        .layouter(layouter);

                    ui.add(text_edit);
                },
            );
        });
    }
}
