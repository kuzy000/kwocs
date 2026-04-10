use egui::{
    Color32, FontId, TextEdit, TextFormat,
    text::{LayoutJob, LayoutSection},
};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TextMergeWithOffset};
use std::ops::Range;

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct App {
    text: String,
}

impl Default for App {
    fn default() -> Self {
        Self {
            // Example stuff:
            text: "Hello World!".to_owned(),
        }
    }
}

impl App {
    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Default::default()
        }
    }
}

impl eframe::App for App {
    /// Called by the framework to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Put your widgets into a `SidePanel`, `TopBottomPanel`, `CentralPanel`, `Window` or `Area`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        egui::Panel::top("top_panel").show_inside(ui, |ui| {
            // The top panel is often a good place for a menu bar:

            egui::MenuBar::new().ui(ui, |ui| {
                // NOTE: no File->Quit on web pages!
                let is_web = cfg!(target_arch = "wasm32");
                if !is_web {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(16.0);
                }

                egui::widgets::global_theme_preference_buttons(ui);
            });
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    let mut layouter = |ui: &egui::Ui,
                                        buf: &dyn egui::TextBuffer,
                                        wrap_width: f32| {
                        let text = String::from(buf.as_str());

                        let iterator =
                            TextMergeWithOffset::new(Parser::new(buf.as_str()).into_offset_iter());

                        let mut sections = Vec::new();

                        let font_regular = FontId::monospace(14.);
                        let font_h1 = FontId::monospace(28.);
                        let font_h2 = FontId::monospace(26.);
                        let font_h3 = FontId::monospace(24.);
                        let font_h4 = FontId::monospace(22.);
                        let font_h5 = FontId::monospace(20.);
                        let font_h6 = FontId::monospace(18.);

                        let mut debug_events = Vec::new();

                        let mut tag_stack = Vec::new();

                        let mut last_end: usize = 0;
                        for event in iterator {
                            debug_events.push(event.clone());
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

                        log::info!("{:#?}", debug_events);

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
                        ui.fonts_mut(|f| f.layout_job(layout_job))
                    };

                    let text_edit = TextEdit::multiline(&mut self.text)
                        .code_editor()
                        .layouter(&mut layouter);

                    ui.add(text_edit);
                },
            );
        });
    }
}
