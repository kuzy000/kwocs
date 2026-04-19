use eframe::wgpu::Color;
use egui::{
    Color32, FontFamily, FontId, Frame, Stroke, TextEdit, TextFormat, Vec2,
    epaint::{color, text},
    load::BytesLoadResult,
    style::Selection,
    text::{LayoutJob, LayoutSection},
};
use pulldown_cmark::{
    CodeBlockKind, Event, HeadingLevel, Parser, Tag, TagEnd, TextMergeWithOffset,
};
use std::rc::Rc;
use std::{cell::RefCell, sync::Arc};
use std::{default, ops::Range};
use syntect::{
    highlighting::{HighlightState, Highlighter, ThemeSet},
    parsing::{ParseState, SyntaxSet},
};

use crate::google;

const FONT_HEADING: &str = "Heading-Regular";
const FONT_HEADING_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Serif/static/NotoSerif-Medium.ttf");

const FONT_HEADING_BOLD: &str = "Heading-Bold";
const FONT_HEADING_BOLD_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Serif/static/NotoSerif-MediumItalic.ttf");

const FONT_HEADING_ITALIC: &str = "Heading-Italic";
const FONT_HEADING_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Serif/static/NotoSerif-Italic.ttf");

const FONT_HEADING_BOLD_ITALIC: &str = "Heading-BoldItalic";
const FONT_HEADING_BOLD_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Serif/static/NotoSerif-BoldItalic.ttf");

const FONT_TEXT: &str = "Text-Regular";
const FONT_TEXT_BYTES: &[u8] = include_bytes!("../assets/Noto_Sans/static/NotoSans-Regular.ttf");

const FONT_TEXT_BOLD: &str = "Text-Bold";
const FONT_TEXT_BOLD_BYTES: &[u8] = include_bytes!("../assets/Noto_Sans/static/NotoSans-Bold.ttf");

const FONT_TEXT_ITALIC: &str = "Text-Italic";
const FONT_TEXT_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Sans/static/NotoSans-Italic.ttf");

const FONT_TEXT_BOLD_ITALIC: &str = "Text-BoldItalic";
const FONT_TEXT_BOLD_ITALIC_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Sans/static/NotoSans-BoldItalic.ttf");

const FONT_CODE: &str = "Code-Regular";
const FONT_CODE_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Sans_Mono/static/NotoSansMono-Regular.ttf");

const FONT_CODE_BOLD: &str = "Code-Bold";
const FONT_CODE_BOLD_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Sans_Mono/static/NotoSansMono-Bold.ttf");

const FONT_CODE_ITALIC: &str = FONT_CODE;

const FONT_CODE_BOLD_ITALIC: &str = FONT_CODE_BOLD;

const FONT_CODE_HEADING: &str = "CodeHeading-Regular";
const FONT_CODE_HEADING_BYTES: &[u8] =
    include_bytes!("../assets/Noto_Sans_Mono/static/NotoSansMono-Medium.ttf");

const FONT_CODE_HEADING_BOLD: &str = FONT_CODE_BOLD;

const FONT_CODE_HEADING_BOLD_ITALIC: &str = FONT_CODE_BOLD_ITALIC;

const FONT_CODE_HEADING_ITALIC: &str = FONT_CODE_HEADING;

const SYNTECT_DARK_BYTES: &[u8] = include_bytes!("../assets/sublime-vscode-plus/Dark+.tmTheme");
const SYNTECT_LIGHT_BYTES: &[u8] = include_bytes!("../assets/sublime-vscode-plus/Light+.tmTheme");

struct CustomColors {
    fg_regular: Color32,
    fg_markup: Color32,
    bg: Color32,
    bg_extreme: Color32,
}

const CUSTOM_COLORS_DARK: CustomColors = CustomColors {
    fg_regular: Color32::from_gray(227),
    fg_markup: Color32::from_gray(96),
    bg: Color32::from_gray(19),
    bg_extreme: Color32::from_gray(36),
};

const CUSTOM_COLORS_LIGHT: CustomColors = CustomColors {
    fg_regular: Color32::from_gray(31),
    fg_markup: Color32::from_gray(160),
    bg: Color32::from_gray(255),
    bg_extreme: Color32::from_gray(240),
};

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
    #[serde(skip)]
    layouter_duration: web_time::Duration,
}

impl Default for App {
    fn default() -> Self {
        let dark_theme = {
            let mut cursor = std::io::Cursor::new(SYNTECT_DARK_BYTES);
            ThemeSet::load_from_reader(&mut cursor).expect("Failed to load default dark theme!")
        };

        let light_theme = {
            let mut cursor = std::io::Cursor::new(SYNTECT_LIGHT_BYTES);
            ThemeSet::load_from_reader(&mut cursor).expect("Failed to load default light theme!")
        };

        let theme_set = ThemeSet {
            themes: [
                ("dark".to_owned(), dark_theme),
                ("light".to_owned(), light_theme),
            ]
            .into(),
        };

        Self {
            text: "Hello World!".to_owned(),
            file_name: "untitled.md".to_owned(),
            file_id: None,
            async_state: Rc::new(RefCell::new(AsyncState::default())),
            open_url_input: String::new(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set,
            layouter_duration: Default::default(),
        }
    }
}

fn text_size(heading_level: Option<HeadingLevel>) -> f32 {
    match heading_level {
        Some(HeadingLevel::H1) => 24.,
        Some(HeadingLevel::H2) => 22.,
        Some(HeadingLevel::H3) => 20.,
        Some(HeadingLevel::H4) => 18.,
        Some(HeadingLevel::H5) => 18.,
        Some(HeadingLevel::H6) => 18.,
        None => 16.,
    }
}

fn text_line_height(
    ui: &egui::Ui,
    heading_level: Option<HeadingLevel>,
    font_size: f32,
    font_family: &FontFamily,
) -> f32 {
    let font_metrics = ui.fonts_mut(|f| {
        let font = f.fonts.font(font_family);
        font.styled_metrics(ui.pixels_per_point(), font_size, &Default::default())
    });

    return font_metrics.row_height
        + match heading_level {
            Some(HeadingLevel::H1) => 10.,
            Some(HeadingLevel::H2) => 8.,
            Some(HeadingLevel::H3) => 6.,
            Some(HeadingLevel::H4) => 4.,
            Some(HeadingLevel::H5) => 4.,
            Some(HeadingLevel::H6) => 4.,
            None => 0.,
        };
}

fn text_font_family(heading: bool, code: bool, emphasis: bool, strong: bool) -> FontFamily {
    let name = match (heading, code, emphasis, strong) {
        (false, false, false, false) => FONT_TEXT,
        (false, false, false, true) => FONT_TEXT_BOLD,
        (false, false, true, false) => FONT_TEXT_ITALIC,
        (false, false, true, true) => FONT_TEXT_BOLD_ITALIC,

        (false, true, false, false) => FONT_CODE,
        (false, true, false, true) => FONT_CODE_BOLD,
        (false, true, true, false) => FONT_CODE_ITALIC,
        (false, true, true, true) => FONT_CODE_BOLD_ITALIC,

        (true, false, false, false) => FONT_HEADING,
        (true, false, false, true) => FONT_HEADING_BOLD,
        (true, false, true, false) => FONT_HEADING_ITALIC,
        (true, false, true, true) => FONT_HEADING_BOLD_ITALIC,

        (true, true, false, false) => FONT_CODE_HEADING,
        (true, true, false, true) => FONT_CODE_HEADING_BOLD,
        (true, true, true, false) => FONT_CODE_HEADING_ITALIC,
        (true, true, true, true) => FONT_CODE_HEADING_BOLD_ITALIC,
    };

    return FontFamily::Name(name.into());
}

fn text_format_markup(ui: &egui::Ui, heading_level: Option<HeadingLevel>) -> TextFormat {
    let font_size = text_size(heading_level);
    let font_family = FontFamily::Name(FONT_CODE.into());
    let line_height = text_line_height(ui, heading_level, font_size, &font_family);
    let font_id = FontId::new(font_size, font_family.clone());

    let colors = if ui.style().visuals.dark_mode {
        CUSTOM_COLORS_DARK
    } else {
        CUSTOM_COLORS_LIGHT
    };

    let color = colors.fg_markup;

    return TextFormat {
        font_id,
        color,
        line_height: Some(line_height),
        expand_bg: 0.,
        ..Default::default()
    };
}

fn text_format(
    ui: &egui::Ui,
    heading_level: Option<HeadingLevel>,
    emphasis: bool,
    strong: bool,
    code: bool,
) -> TextFormat {
    let font_size = text_size(heading_level);
    let font_family = text_font_family(heading_level.is_some(), code, emphasis, strong);
    let line_height = text_line_height(ui, heading_level, font_size, &font_family);
    let font_id = FontId::new(font_size, font_family);

    let italics = code && emphasis; // NotoSansMono doesn't have italics variant

    let colors = if ui.style().visuals.dark_mode {
        CUSTOM_COLORS_DARK
    } else {
        CUSTOM_COLORS_LIGHT
    };

    let color = colors.fg_regular;

    return TextFormat {
        font_id,
        color,
        italics,
        line_height: Some(line_height),
        expand_bg: 0.,
        ..Default::default()
    };
}

fn code_layout(
    ui: &egui::Ui,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    sections: &mut Vec<LayoutSection>,
    language: &str,
    code: &str,
    code_range: Range<usize>,
) {
    let font_id = FontId::new(text_size(None), text_font_family(false, true, false, false));

    let colors = if ui.style().visuals.dark_mode {
        CUSTOM_COLORS_DARK
    } else {
        CUSTOM_COLORS_LIGHT
    };
    let color = colors.fg_regular;

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

    let theme = if ui.style().visuals.dark_mode {
        &theme_set.themes["dark"]
    } else {
        &theme_set.themes["light"]
    };

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

            for (style, _str, token_range_in_line) in iter {
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

    let options = pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_TASKLISTS
        | pulldown_cmark::Options::ENABLE_GFM;

    let iterator =
        TextMergeWithOffset::new(Parser::new_ext(buf.as_str(), options).into_offset_iter());

    let mut sections = Vec::new();

    let mut debug_tags = Vec::new();

    // Dunno if they could really nest. Using the top one
    let mut heading_stack = Vec::new();
    heading_stack.push(None); // We have skipping of some headings in which case `None` is pushed

    let mut code_stack = Vec::new();

    let mut strong_depth: u32 = 0;
    let mut emphasis_depth: u32 = 0;
    let mut table_depth: u32 = 0;
    let mut _quote_depth: u32 = 0; // TODO: Make a background around quotes

    let mut last_end: usize = 0;
    for event in iterator {
        debug_tags.push(event.clone());

        let range = event.1;

        // Close previous range
        {
            let current_end = match event.0 {
                Event::End(_) => range.end,
                _ => range.start,
            };

            if current_end > last_end {
                let format = text_format_markup(ui, heading_stack.last().copied().unwrap_or(None));

                sections.push(LayoutSection {
                    leading_space: 0.0,
                    byte_range: Range {
                        start: last_end,
                        end: current_end,
                    },
                    format,
                });
                last_end = current_end;
            }
        }

        match event.0 {
            Event::Start(Tag::Heading { level, .. }) => {
                // I don't like that this is a header. It is just some text before list
                // HEADER
                //  -
                //
                // Here it should be at least --- to be a header
                // HEADER
                //  ---
                let str = &text[range.start..range.end];
                let str = str.trim();
                let skip = !str.starts_with('#') && !str.ends_with("---") && !str.ends_with("===");

                heading_stack.push(if skip { None } else { Some(level) });
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

            Event::Start(Tag::Table { .. }) => {
                table_depth += 1;
            }
            Event::End(TagEnd::Table) => {
                table_depth -= 1;
            }

            Event::Text(str) => {
                if let Some(CodeBlockKind::Fenced(language)) = code_stack.last() {
                    code_layout(
                        ui,
                        syntax_set,
                        theme_set,
                        &mut sections,
                        language.as_ref(),
                        str.as_ref(),
                        range.clone(),
                    );
                } else {
                    let format = text_format(
                        ui,
                        heading_stack.last().copied().unwrap_or(None),
                        emphasis_depth > 0,
                        strong_depth > 0,
                        table_depth > 0,
                    );

                    sections.push(LayoutSection {
                        leading_space: 0.0,
                        byte_range: range.clone(),
                        format,
                    });
                }

                last_end = range.end;
            }
            Event::Code(_) => {
                // Must always be true because "`c`" is a minimum Code sentence
                if range.len() >= 3 {
                    let markup =
                        text_format_markup(ui, heading_stack.last().copied().unwrap_or(None));
                    let format = text_format(
                        ui,
                        heading_stack.last().copied().unwrap_or(None),
                        emphasis_depth > 0,
                        strong_depth > 0,
                        true,
                    );

                    sections.push(LayoutSection {
                        leading_space: 0.0,
                        byte_range: Range {
                            start: range.start,
                            end: range.start + 1,
                        },
                        format: markup.clone(),
                    });
                    sections.push(LayoutSection {
                        leading_space: 0.0,
                        byte_range: Range {
                            start: range.start + 1,
                            end: range.end - 1,
                        },
                        format,
                    });
                    sections.push(LayoutSection {
                        leading_space: 0.0,
                        byte_range: Range {
                            start: range.end - 1,
                            end: range.end,
                        },
                        format: markup,
                    });
                } else {
                    // Fallback
                    let format = text_format(
                        ui,
                        heading_stack.last().copied().unwrap_or(None),
                        emphasis_depth > 0,
                        strong_depth > 0,
                        false,
                    );

                    sections.push(LayoutSection {
                        leading_space: 0.0,
                        byte_range: range.clone(),
                        format,
                    });
                }

                last_end = range.end;
            }
            _ => (),
        }
    }

    log::info!("{:#?}", debug_tags); // TODO: remove spam

    {
        let format = text_format_markup(ui, heading_stack.last().copied().unwrap_or(None));

        if last_end < text.len() {
            sections.push(LayoutSection {
                leading_space: 0.0,
                byte_range: Range {
                    start: last_end,
                    end: text.len(),
                },
                format: format,
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
        {
            let mut fonts = egui::FontDefinitions::default();

            let mut add_font = |name: &str, bytes: &'static [u8]| {
                fonts.font_data.insert(
                    name.to_owned(),
                    Arc::new(egui::FontData::from_static(bytes)),
                );

                fonts
                    .families
                    .insert(FontFamily::Name(name.into()), vec![name.to_owned()]);
            };

            add_font(FONT_HEADING, FONT_HEADING_BYTES);
            add_font(FONT_HEADING_ITALIC, FONT_HEADING_ITALIC_BYTES);
            add_font(FONT_HEADING_BOLD, FONT_HEADING_BOLD_BYTES);
            add_font(FONT_HEADING_BOLD_ITALIC, FONT_HEADING_BOLD_ITALIC_BYTES);

            add_font(FONT_TEXT, FONT_TEXT_BYTES);
            add_font(FONT_TEXT_ITALIC, FONT_TEXT_ITALIC_BYTES);
            add_font(FONT_TEXT_BOLD, FONT_TEXT_BOLD_BYTES);
            add_font(FONT_TEXT_BOLD_ITALIC, FONT_TEXT_BOLD_ITALIC_BYTES);

            add_font(FONT_CODE, FONT_CODE_BYTES);
            add_font(FONT_CODE_BOLD, FONT_CODE_BOLD_BYTES);
            add_font(FONT_CODE_HEADING, FONT_CODE_HEADING_BYTES);

            cc.egui_ctx.set_fonts(fonts);
        }

        {
            let modify_style = |colors: CustomColors, style: &mut egui::Style| {
                style.visuals.widgets.inactive.fg_stroke.color = colors.fg_regular;
                style.visuals.widgets.noninteractive.fg_stroke.color = colors.fg_regular;

                style.visuals.extreme_bg_color = colors.bg_extreme;
                style.visuals.panel_fill = colors.bg;
                style.visuals.selection = Selection {
                    bg_fill: colors.fg_regular,
                    stroke: Stroke::new(1., colors.bg),
                }
            };

            cc.egui_ctx
                .style_mut_of(egui::Theme::Dark, |style: &mut egui::Style| {
                    modify_style(CUSTOM_COLORS_DARK, style)
                });
            cc.egui_ctx
                .style_mut_of(egui::Theme::Light, |style: &mut egui::Style| {
                    modify_style(CUSTOM_COLORS_LIGHT, style)
                });
        }

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
            ui.label(format!(
                "Layouter: {}ms",
                self.layouter_duration.as_secs_f64() * 1000.,
            ));
        });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    egui::ScrollArea::vertical()
                        .wheel_scroll_multiplier(Vec2::splat(1.5))
                        .show(ui, |ui| {
                            let mut layouter_closure =
                                |ui: &egui::Ui,
                                 buf: &dyn egui::TextBuffer,
                                 wrap_width: f32|
                                 -> Arc<egui::Galley> {
                                    let start = web_time::Instant::now();
                                    let res = layouter(
                                        &self.syntax_set,
                                        &self.theme_set,
                                        ui,
                                        buf,
                                        wrap_width,
                                    );
                                    self.layouter_duration = start.elapsed();

                                    return res;
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
