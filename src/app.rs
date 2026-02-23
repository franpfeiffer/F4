use iced::widget::{column, container, row, stack, text, text_editor};
use iced::widget::text::Wrapping;
use iced::{Element, Fill, Task, Theme};
use iced::widget;
use std::path::PathBuf;

use crate::cursor_editor::CursorEditor;
use crate::highlight::{FindHighlightSettings, FindHighlighter, format_highlight};
use crate::message::{LineNumbers, Message, PendingAction, VimMode, VimPending};

pub const EDITOR_ID: widget::Id = widget::Id::new("editor");

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
    pub vim_command: String,
    pub vim_visual_anchor: Option<(usize, usize)>,
    pub vim_visual_head: (usize, usize),
    pub vim_col: usize,
    pub line_numbers: LineNumbers,
    pub vim_search_query: String,
    pub vim_search_forward: bool,
    pub undo_tree: crate::undo_tree::UndoTree,
    pub show_undo_panel: bool,
    pub selected_undo_node: Option<usize>,
    pub undo_panel_focused: bool,
    pub undo_preview_text: String,
    pub changedtick: u64,
    pub last_snapshot_tick: u64,
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
                vim_command: String::new(),
                vim_visual_anchor: None,
                vim_visual_head: (0, 0),
                vim_col: 0,
                line_numbers: LineNumbers::None,
                vim_search_query: String::new(),
                vim_search_forward: true,
                undo_tree: crate::undo_tree::UndoTree::new(crate::undo_tree::Snapshot {
                    text: String::new(),
                    cursor_line: 0,
                    cursor_col: 0,
                }),
                show_undo_panel: false,
                selected_undo_node: None,
                undo_panel_focused: false,
                undo_preview_text: String::new(),
                changedtick: 0,
                last_snapshot_tick: 0,
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
            format!("*{} - F4", name)
        } else {
            format!("{} - F4", name)
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

        let vim_normal_or_visual = self.vim_enabled && matches!(
            self.vim_mode,
            VimMode::Normal | VimMode::Visual | VimMode::VisualLine
        );

        let editor = text_editor(&self.content)
            .id(EDITOR_ID)
            .height(Fill)
            .wrapping(wrapping)
            .on_action(Message::Edit)
            .key_binding(move |key_press| {
                if vim_normal_or_visual {
                    if matches!(
                        key_press.key,
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
                    ) {
                        return None;
                    }
                    return None;
                }
                text_editor::Binding::from_key_press(key_press)
            })
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

        let show_block = self.vim_enabled && matches!(
            self.vim_mode,
            VimMode::Normal | VimMode::Visual | VimMode::VisualLine
        );
        let cursor = self.content.cursor();
        let current_line = cursor.position.line;
        let editor_widget: Element<'_, Message> = CursorEditor::new(
            editor,
            current_line,
            self.vim_col,
            show_block,
        ).into();

        let editor_area: Element<'_, Message> = if self.line_numbers != LineNumbers::None {
            let total = self.content.line_count();
            let last_line_empty = self.content.line(total.saturating_sub(1))
                .map(|l| l.text.is_empty())
                .unwrap_or(true);
            let line_count = if last_line_empty && total > 1 { total - 1 } else { total };
            let font_size = 16.0_f32;
            let line_height = font_size * 1.3;
            let gutter_col = (0..line_count).fold(
                column![].spacing(0),
                |col, i| {
                    let n = match self.line_numbers {
                        LineNumbers::Absolute => i + 1,
                        LineNumbers::Relative => {
                            if i == current_line { i + 1 } else { (i as isize - current_line as isize).unsigned_abs() }
                        }
                        LineNumbers::None => unreachable!(),
                    };
                    let color = if i == current_line {
                        iced::Color::from_rgb(0.7, 0.7, 0.7)
                    } else {
                        iced::Color::from_rgb(0.4, 0.4, 0.4)
                    };
                    col.push(
                        container(
                            text(format!("{:>4}", n))
                                .size(font_size)
                                .font(iced::Font::MONOSPACE)
                                .style(move |_: &Theme| text::Style { color: Some(color) })
                        )
                        .height(line_height)
                        .align_y(iced::Alignment::Center)
                    )
                }
            );
            let gutter: Element<'_, Message> = container(gutter_col)
                .padding([5, 4])
                .style(|theme: &Theme| container::Style {
                    background: Some(theme.extended_palette().background.weak.color.into()),
                    ..Default::default()
                })
                .into();
            row![gutter, editor_widget].into()
        } else {
            editor_widget
        };

        let main_row: Element<'_, Message> = if self.show_undo_panel {
            row![editor_area, self.undo_tree_panel()].into()
        } else {
            editor_area
        };
        col = col.push(main_row);
        if self.vim_enabled && self.vim_mode == VimMode::Command {
            col = col.push(self.command_bar());
        } else if self.vim_enabled && self.vim_mode == VimMode::Search {
            col = col.push(self.search_bar());
        } else {
            col = col.push(self.status_bar());
        }

        let has_overlay = self.show_about || self.pending_action.is_some();
        if has_overlay {
            let mut layers = stack![col];
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


