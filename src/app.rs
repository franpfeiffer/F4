use iced::widget::{canvas, column, stack, text_editor};
use iced::widget::text::Wrapping;
use iced::{Color, Element, Fill, Point, Rectangle, Renderer, Size, Task, Theme};
use std::path::PathBuf;

use crate::highlight::{FindHighlightSettings, FindHighlighter, format_highlight};
use crate::message::{Message, PendingAction, VimMode, VimPending};

pub struct App {
    pub content: text_editor::Content,
    pub current_file: Option<PathBuf>,
    pub is_modified: bool,
    pub show_panel: bool,
    pub find_query: String,
    pub replace_text: String,
    pub case_sensitive: bool,
    pub goto_line: String,
    pub find_matches: Vec<(usize, usize)>,
    pub current_match: Option<usize>,
    pub word_wrap: bool,
    pub scale: f32,
    pub ctrl_held: bool,
    pub show_about: bool,
    pub pending_action: Option<PendingAction>,
    pub vim_enabled: bool,
    pub vim_mode: VimMode,
    pub vim_pending: Option<VimPending>,
    pub vim_count: String,
    pub vim_operator: Option<char>,
    pub vim_register: String,
    pub vim_find_last: Option<(char, bool, bool)>,
}

impl App {
    pub fn new() -> (Self, Task<Message>) {
        (
            Self {
                content: text_editor::Content::new(),
                current_file: None,
                is_modified: false,
                show_panel: false,
                find_query: String::new(),
                replace_text: String::new(),
                case_sensitive: false,
                goto_line: String::new(),
                find_matches: Vec::new(),
                current_match: None,
                word_wrap: true,
                scale: 1.0,
                ctrl_held: false,
                show_about: false,
                pending_action: None,
                vim_enabled: false,
                vim_mode: VimMode::Insert,
                vim_pending: None::<VimPending>,
                vim_count: String::new(),
                vim_operator: None,
                vim_register: String::new(),
                vim_find_last: None,
            },
            Task::none(),
        )
    }

    pub fn title(&self) -> String {
        let name = match &self.current_file {
            Some(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| String::from("Untitled")),
            None => String::from("Untitled"),
        };
        if self.is_modified {
            format!("*{} - f4", name)
        } else {
            format!("{} - f4", name)
        }
    }

    pub fn theme(&self) -> Theme {
        Theme::Dark
    }

    pub fn view(&self) -> Element<'_, Message> {
        let mut col = column![self.menu_bar()];

        if self.show_panel {
            col = col.push(self.search_panel());
        }

        let wrapping = if self.word_wrap {
            Wrapping::Word
        } else {
            Wrapping::None
        };

        let editor = text_editor(&self.content)
            .height(Fill)
            .wrapping(wrapping)
            .on_action(Message::Edit)
            .style(|theme: &Theme, status| {
                let mut style = text_editor::default(theme, status);
                style.border.width = 0.0;
                style
            });

        let active = self.show_panel && !self.find_query.is_empty();
        let editor: Element<'_, Message> = editor
            .highlight_with::<FindHighlighter>(
                FindHighlightSettings {
                    matches: if active { self.find_matches.clone() } else { vec![] },
                    query_len: if active { self.find_query.len() } else { 0 },
                    current_match: if active { self.current_match } else { None },
                },
                format_highlight,
            )
            .into();

        col = col.push(editor);
        col = col.push(self.status_bar());

        let show_block_cursor = self.vim_enabled && self.vim_mode == VimMode::Normal;

        let has_overlay = self.show_about || self.pending_action.is_some() || show_block_cursor;
        if has_overlay {
            let mut layers = stack![col];
            if show_block_cursor {
                let cursor = self.content.cursor();
                let line = cursor.position.line as f32;
                let col_pos = cursor.position.column as f32;
                layers = layers.push(
                    canvas(BlockCursor { line, col_pos }).width(Fill).height(Fill)
                );
            }
            if self.show_about {
                layers = layers.push(self.about_dialog());
            }
            if self.pending_action.is_some() {
                layers = layers.push(self.save_changes_dialog());
            }
            layers.into()
        } else {
            col.into()
        }
    }
}

struct BlockCursor {
    line: f32,
    col_pos: f32,
}

impl<Message> canvas::Program<Message> for BlockCursor {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let line_height = 20.0_f32;
        let char_width = 8.6_f32;
        let editor_padding = 5.0_f32;
        let menu_bar_height = 29.0_f32;

        let x = editor_padding + self.col_pos * char_width;
        let y = menu_bar_height + editor_padding + self.line * line_height;

        let block = Rectangle {
            x,
            y,
            width: char_width,
            height: line_height,
        };

        frame.fill_rectangle(
            Point::new(block.x, block.y),
            Size::new(block.width, block.height),
            Color::from_rgba(1.0, 1.0, 1.0, 0.35),
        );

        vec![frame.into_geometry()]
    }
}

