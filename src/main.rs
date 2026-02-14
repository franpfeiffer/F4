use iced::widget::{button, center, checkbox, column, container, row, stack, text, text_editor, text_input};
use iced::widget::text::Wrapping;
use iced::{event, keyboard, window, Element, Event, Fill, Font, Length, Subscription, Task, Theme};
use iced_aw::menu::{Item, Menu, MenuBar};
use std::path::PathBuf;
use std::sync::Arc;

const ICON: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> iced::Result {
    let icon = window::icon::from_file_data(ICON, None).ok();

    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .scale_factor(|app| app.scale)
        .default_font(Font::MONOSPACE)
        .window(window::Settings {
            size: iced::Size::new(800.0, 600.0),
            icon,
            ..window::Settings::default()
        })
        .run()
}

struct App {
    content: text_editor::Content,
    current_file: Option<PathBuf>,
    is_modified: bool,
    show_panel: bool,
    find_query: String,
    replace_text: String,
    case_sensitive: bool,
    goto_line: String,
    find_matches: Vec<(usize, usize)>,
    current_match: Option<usize>,
    word_wrap: bool,
    scale: f32,
    ctrl_held: bool,
    show_about: bool,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
    New,
    Open,
    FileOpened(Option<(PathBuf, String)>),
    Save,
    SaveAs,
    FileSaved(Option<PathBuf>),
    Exit,
    Undo,
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
    FormatDocument,
    TogglePanel,
    ClosePanel,
    FindQueryChanged(String),
    ReplaceTextChanged(String),
    GoToLineChanged(String),
    ToggleCaseSensitive(bool),
    FindNext,
    FindPrevious,
    ReplaceOne,
    ReplaceAll,
    GoToLineSubmit,
    ToggleWordWrap,
    ZoomIn,
    ZoomOut,
    CtrlPressed,
    CtrlReleased,
    ShowAbout,
    CloseAbout,
}

impl App {
    fn new() -> (Self, Task<Message>) {
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
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
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

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, status, _window| {
            match &event {
                Event::Keyboard(keyboard::Event::KeyPressed { key: keyboard::Key::Named(keyboard::key::Named::Control), .. }) => {
                    return Some(Message::CtrlPressed);
                }
                Event::Keyboard(keyboard::Event::KeyReleased { key: keyboard::Key::Named(keyboard::key::Named::Control), .. }) => {
                    return Some(Message::CtrlReleased);
                }
                _ => {}
            }

            if let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, physical_key, .. }) = event {
                if modifiers.control() {
                    match physical_key {
                        keyboard::key::Physical::Code(keyboard::key::Code::Equal) => return Some(Message::ZoomIn),
                        keyboard::key::Physical::Code(keyboard::key::Code::Minus) => return Some(Message::ZoomOut),
                        _ => {}
                    }
                }

                if matches!(status, event::Status::Captured) {
                    return None;
                }

                if modifiers.control() && modifiers.shift() {
                    match key.as_ref() {
                        keyboard::Key::Character("S") => return Some(Message::SaveAs),
                        _ => {}
                    }
                }

                if modifiers.control() && !modifiers.shift() {
                    match key.as_ref() {
                        keyboard::Key::Character("n") => return Some(Message::New),
                        keyboard::Key::Character("o") => return Some(Message::Open),
                        keyboard::Key::Character("s") => return Some(Message::Save),
                        keyboard::Key::Character("f") => return Some(Message::TogglePanel),
                        keyboard::Key::Character("h") => return Some(Message::TogglePanel),
                        keyboard::Key::Character("g") => return Some(Message::TogglePanel),
                        _ => {}
                    }
                }

                if modifiers.is_empty() {
                    match key.as_ref() {
                        keyboard::Key::Named(keyboard::key::Named::F5) => {
                            return Some(Message::FormatDocument);
                        }
                        keyboard::Key::Named(keyboard::key::Named::F3) => {
                            return Some(Message::FindNext);
                        }
                        keyboard::Key::Named(keyboard::key::Named::Escape) => {
                            return Some(Message::ClosePanel);
                        }
                        _ => {}
                    }
                }

                if modifiers.shift() && !modifiers.control() {
                    if let keyboard::Key::Named(keyboard::key::Named::F3) = key.as_ref() {
                        return Some(Message::FindPrevious);
                    }
                }
            }

            None
        })
    }

    fn find_all_matches(&mut self) {
        self.find_matches.clear();
        self.current_match = None;

        if self.find_query.is_empty() {
            return;
        }

        let content_text = self.content.text();
        let (haystack, needle) = if self.case_sensitive {
            (content_text.clone(), self.find_query.clone())
        } else {
            (content_text.to_lowercase(), self.find_query.to_lowercase())
        };

        let mut start = 0;
        while let Some(pos) = haystack[start..].find(&needle) {
            let abs_pos = start + pos;
            let line = content_text[..abs_pos].matches('\n').count();
            let line_start = if line == 0 {
                0
            } else {
                content_text[..abs_pos].rfind('\n').unwrap() + 1
            };
            let col = abs_pos - line_start;
            self.find_matches.push((line, col));
            start = abs_pos + 1;
        }
    }

    fn navigate_to_match(&mut self, index: usize) {
        if let Some(&(line, col)) = self.find_matches.get(index) {
            self.current_match = Some(index);
            let end_col = col + self.find_query.len();
            self.content.move_to(text_editor::Cursor {
                position: text_editor::Position { line, column: col },
                selection: Some(text_editor::Position {
                    line,
                    column: end_col,
                }),
            });
        }
    }

    fn search_panel(&self) -> Element<'_, Message> {
        let match_info = if self.find_query.is_empty() {
            String::new()
        } else if self.find_matches.is_empty() {
            String::from("No matches")
        } else {
            let idx = self.current_match.map(|i| i + 1).unwrap_or(0);
            format!("{}/{}", idx, self.find_matches.len())
        };

        let line_count = self.content.line_count();

        container(
            column![
                row![
                    text("Find:").size(14).width(60),
                    text_input("Search...", &self.find_query)
                        .size(14)
                        .on_input(Message::FindQueryChanged)
                        .on_submit(Message::FindNext)
                        .width(Length::Fill),
                    text(match_info).size(12).width(80),
                    checkbox(self.case_sensitive).label("Aa").on_toggle(Message::ToggleCaseSensitive).size(14),
                    dialog_button("Find Next", Message::FindNext),
                    dialog_button("Find Prev", Message::FindPrevious),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
                row![
                    text("Replace:").size(14).width(60),
                    text_input("Replace with...", &self.replace_text)
                        .size(14)
                        .on_input(Message::ReplaceTextChanged)
                        .width(Length::Fill),
                    dialog_button("Replace", Message::ReplaceOne),
                    dialog_button("Replace All", Message::ReplaceAll),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
                row![
                    text(format!("Go To Line (1-{}):", line_count)).size(14),
                    text_input("Line number...", &self.goto_line)
                        .size(14)
                        .on_input(Message::GoToLineChanged)
                        .on_submit(Message::GoToLineSubmit)
                        .width(200),
                    dialog_button("Go", Message::GoToLineSubmit),
                    iced::widget::Space::new().width(Length::Fill),
                    dialog_button("X", Message::ClosePanel),
                ]
                .spacing(6)
                .align_y(iced::Alignment::Center),
            ]
            .spacing(4),
        )
        .padding([6, 8])
        .style(|theme: &Theme| container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            ..Default::default()
        })
        .into()
    }

    fn menu_bar(&self) -> Element<'_, Message> {
        let file_menu = Menu::new(vec![
            Item::new(menu_item("New", "Ctrl+N", Message::New)),
            Item::new(menu_item("Open", "Ctrl+O", Message::Open)),
            Item::new(menu_item("Save", "Ctrl+S", Message::Save)),
            Item::new(menu_item("Save As", "Ctrl+Shift+S", Message::SaveAs)),
            Item::new(separator()),
            Item::new(menu_item("Exit", "Alt+F4", Message::Exit)),
        ])
        .max_width(220.0);

        let edit_menu = Menu::new(vec![
            Item::new(menu_item_disabled("Undo")),
            Item::new(separator()),
            Item::new(menu_item("Cut", "Ctrl+X", Message::Cut)),
            Item::new(menu_item("Copy", "Ctrl+C", Message::Copy)),
            Item::new(menu_item("Paste", "Ctrl+V", Message::Paste)),
            Item::new(menu_item("Delete", "Del", Message::Delete)),
            Item::new(separator()),
            Item::new(menu_item("Find", "Ctrl+F", Message::TogglePanel)),
            Item::new(menu_item("Replace", "Ctrl+H", Message::TogglePanel)),
            Item::new(menu_item("Go To Line", "Ctrl+G", Message::TogglePanel)),
            Item::new(separator()),
            Item::new(menu_item("Select All", "Ctrl+A", Message::SelectAll)),
            Item::new(separator()),
            Item::new(menu_item("Format Document", "F5", Message::FormatDocument)),
        ])
        .max_width(220.0);

        let wrap_label = if self.word_wrap { "Word Wrap ✓" } else { "Word Wrap" };
        let format_menu = Menu::new(vec![
            Item::new(menu_item(wrap_label, "", Message::ToggleWordWrap)),
            Item::new(separator()),
            Item::new(menu_item("Zoom In", "Ctrl+=", Message::ZoomIn)),
            Item::new(menu_item("Zoom Out", "Ctrl+-", Message::ZoomOut)),
        ])
        .max_width(220.0);

        let view_menu = Menu::new(vec![
            Item::new(menu_item_disabled("Status Bar")),
        ])
        .max_width(220.0);

        let help_menu = Menu::new(vec![
            Item::new(menu_item("About F4", "", Message::ShowAbout)),
        ])
        .max_width(220.0);

        let bar = MenuBar::new(vec![
            Item::with_menu(menu_root("File"), file_menu),
            Item::with_menu(menu_root("Edit"), edit_menu),
            Item::with_menu(menu_root("Format"), format_menu),
            Item::with_menu(menu_root("View"), view_menu),
            Item::with_menu(menu_root("Help"), help_menu),
        ])
        .spacing(4.0);

        bar.into()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                if self.ctrl_held {
                    if let text_editor::Action::Edit(text_editor::Edit::Insert(_)) = &action {
                        return Task::none();
                    }
                }
                let is_edit = action.is_edit();
                self.content.perform(action);
                if is_edit {
                    self.is_modified = true;
                    if self.show_panel {
                        self.find_all_matches();
                    }
                }
                Task::none()
            }
            Message::New => {
                self.content = text_editor::Content::new();
                self.current_file = None;
                self.is_modified = false;
                self.show_panel = false;
                self.find_matches.clear();
                self.current_match = None;
                Task::none()
            }
            Message::Open => Task::perform(
                async {
                    let handle = rfd::AsyncFileDialog::new()
                        .add_filter("Text Files", &["txt", "md", "rs", "toml", "json", "yaml", "yml", "xml", "html", "css", "js", "ts", "py", "sh"])
                        .add_filter("All Files", &["*"])
                        .pick_file()
                        .await?;
                    let path = handle.path().to_path_buf();
                    let text = std::fs::read_to_string(&path).ok()?;
                    Some((path, text))
                },
                Message::FileOpened,
            ),
            Message::FileOpened(Some((path, text))) => {
                self.content = text_editor::Content::with_text(&text);
                self.current_file = Some(path);
                self.is_modified = false;
                Task::none()
            }
            Message::FileOpened(None) => Task::none(),
            Message::Save => {
                if let Some(path) = self.current_file.clone() {
                    let text = self.content.text();
                    Task::perform(
                        async move {
                            std::fs::write(&path, &text).ok()?;
                            Some(path)
                        },
                        Message::FileSaved,
                    )
                } else {
                    self.update(Message::SaveAs)
                }
            }
            Message::SaveAs => {
                let text = self.content.text();
                Task::perform(
                    async move {
                        let handle = rfd::AsyncFileDialog::new()
                            .add_filter("Text Files", &["txt"])
                            .add_filter("All Files", &["*"])
                            .save_file()
                            .await?;
                        let path = handle.path().to_path_buf();
                        std::fs::write(&path, &text).ok()?;
                        Some(path)
                    },
                    Message::FileSaved,
                )
            }
            Message::FileSaved(Some(path)) => {
                self.current_file = Some(path);
                self.is_modified = false;
                Task::none()
            }
            Message::FileSaved(None) => Task::none(),
            Message::Exit => iced::exit(),
            Message::Undo => Task::none(),
            Message::Cut => {
                if let Some(selected) = self.content.selection() {
                    let _ = arboard::Clipboard::new()
                        .and_then(|mut cb| cb.set_text(selected));
                    self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                    self.is_modified = true;
                }
                Task::none()
            }
            Message::Copy => {
                if let Some(selected) = self.content.selection() {
                    let _ = arboard::Clipboard::new()
                        .and_then(|mut cb| cb.set_text(selected));
                }
                Task::none()
            }
            Message::Paste => {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    if let Ok(content) = cb.get_text() {
                        self.content.perform(text_editor::Action::Edit(
                            text_editor::Edit::Paste(Arc::new(content)),
                        ));
                        self.is_modified = true;
                    }
                }
                Task::none()
            }
            Message::Delete => {
                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                self.is_modified = true;
                Task::none()
            }
            Message::SelectAll => {
                self.content.perform(text_editor::Action::SelectAll);
                Task::none()
            }
            Message::FormatDocument => {
                let indent_size = 4;
                let original = self.content.text();
                let mut level: usize = 0;
                let formatted: String = original
                    .lines()
                    .map(|line| {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            return String::new();
                        }
                        let first_word = trimmed.split_whitespace().next().unwrap_or("");
                        let closes_brace = trimmed.starts_with('}')
                            || trimmed.starts_with(']')
                            || trimmed.starts_with(')');
                        let closes_keyword = matches!(
                            first_word,
                            "end" | "end;" | "end," | "endif"
                                | "endfor" | "endwhile" | "endfunction"
                                | "else" | "elseif" | "elif"
                                | "elsif" | "except" | "catch"
                                | "finally" | "when" | "rescue"
                        );
                        if closes_brace || closes_keyword {
                            level = level.saturating_sub(1);
                        }
                        let result = format!("{}{}", " ".repeat(level * indent_size), trimmed);
                        for ch in trimmed.chars() {
                            match ch {
                                '{' | '[' | '(' => level += 1,
                                '}' | ']' | ')' => level = level.saturating_sub(1),
                                _ => {}
                            }
                        }
                        let opens_keyword = matches!(
                            first_word,
                            "if" | "else" | "elseif" | "elif"
                                | "elsif" | "for" | "while" | "do"
                                | "loop" | "begin" | "case" | "switch"
                                | "try" | "catch" | "except" | "finally"
                                | "def" | "class" | "module" | "when"
                                | "rescue" | "unless" | "until"
                        );
                        let ends_with_opener = trimmed.ends_with("then")
                            || trimmed.ends_with("do")
                            || trimmed.ends_with("repeat");
                        let has_function = trimmed.contains("function")
                            && !trimmed.starts_with("end");
                        if opens_keyword || ends_with_opener || has_function {
                            if !trimmed.contains("end")
                                || trimmed.contains("function")
                                || closes_keyword
                            {
                                level += 1;
                            }
                        }
                        result
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if formatted != original.trim_end_matches('\n') {
                    self.content = text_editor::Content::with_text(&formatted);
                    self.is_modified = true;
                }
                Task::none()
            }
            Message::TogglePanel => {
                self.show_panel = !self.show_panel;
                if self.show_panel {
                    self.find_all_matches();
                } else {
                    self.find_matches.clear();
                    self.current_match = None;
                }
                Task::none()
            }
            Message::ClosePanel => {
                self.show_panel = false;
                self.find_matches.clear();
                self.current_match = None;
                Task::none()
            }
            Message::FindQueryChanged(query) => {
                self.find_query = query;
                self.find_all_matches();
                Task::none()
            }
            Message::ReplaceTextChanged(text) => {
                self.replace_text = text;
                Task::none()
            }
            Message::GoToLineChanged(line) => {
                self.goto_line = line;
                Task::none()
            }
            Message::ToggleCaseSensitive(val) => {
                self.case_sensitive = val;
                self.find_all_matches();
                Task::none()
            }
            Message::FindNext => {
                if self.find_matches.is_empty() {
                    return Task::none();
                }
                let next = match self.current_match {
                    Some(i) => (i + 1) % self.find_matches.len(),
                    None => 0,
                };
                self.navigate_to_match(next);
                Task::none()
            }
            Message::FindPrevious => {
                if self.find_matches.is_empty() {
                    return Task::none();
                }
                let prev = match self.current_match {
                    Some(0) => self.find_matches.len() - 1,
                    Some(i) => i - 1,
                    None => self.find_matches.len() - 1,
                };
                self.navigate_to_match(prev);
                Task::none()
            }
            Message::ReplaceOne => {
                if let Some(idx) = self.current_match {
                    if idx < self.find_matches.len() {
                        let (line, col) = self.find_matches[idx];
                        let end_col = col + self.find_query.len();
                        self.content.move_to(text_editor::Cursor {
                            position: text_editor::Position { line, column: col },
                            selection: Some(text_editor::Position {
                                line,
                                column: end_col,
                            }),
                        });
                        self.content.perform(text_editor::Action::Edit(
                            text_editor::Edit::Paste(Arc::new(self.replace_text.clone())),
                        ));
                        self.is_modified = true;
                        self.find_all_matches();
                        if !self.find_matches.is_empty() {
                            let next = idx.min(self.find_matches.len() - 1);
                            self.navigate_to_match(next);
                        }
                    }
                } else if !self.find_matches.is_empty() {
                    self.navigate_to_match(0);
                }
                Task::none()
            }
            Message::ReplaceAll => {
                if self.find_matches.is_empty() || self.find_query.is_empty() {
                    return Task::none();
                }
                let original = self.content.text();
                let replaced = if self.case_sensitive {
                    original.replace(&self.find_query, &self.replace_text)
                } else {
                    let mut result = original.clone();
                    let lower_query = self.find_query.to_lowercase();
                    let mut search_start = 0;
                    while let Some(pos) = result[search_start..].to_lowercase().find(&lower_query) {
                        let abs_pos = search_start + pos;
                        result.replace_range(abs_pos..abs_pos + self.find_query.len(), &self.replace_text);
                        search_start = abs_pos + self.replace_text.len();
                    }
                    result
                };
                if replaced != original {
                    self.content = text_editor::Content::with_text(&replaced);
                    self.is_modified = true;
                    self.find_all_matches();
                }
                Task::none()
            }
            Message::GoToLineSubmit => {
                if let Ok(line_num) = self.goto_line.trim().parse::<usize>() {
                    let target = line_num.saturating_sub(1);
                    let max_line = self.content.line_count().saturating_sub(1);
                    let line = target.min(max_line);
                    self.content.move_to(text_editor::Cursor {
                        position: text_editor::Position { line, column: 0 },
                        selection: None,
                    });
                    self.goto_line.clear();
                }
                Task::none()
            }
            Message::ToggleWordWrap => {
                self.word_wrap = !self.word_wrap;
                Task::none()
            }
            Message::ZoomIn => {
                self.scale = (self.scale + 0.1).min(3.0);
                Task::none()
            }
            Message::ZoomOut => {
                self.scale = (self.scale - 0.1).max(0.5);
                Task::none()
            }
            Message::CtrlPressed => {
                self.ctrl_held = true;
                Task::none()
            }
            Message::CtrlReleased => {
                self.ctrl_held = false;
                Task::none()
            }
            Message::ShowAbout => {
                self.show_about = true;
                Task::none()
            }
            Message::CloseAbout => {
                self.show_about = false;
                Task::none()
            }
        }
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let cursor = self.content.cursor();
        let line = cursor.position.line + 1;
        let col = cursor.position.column + 1;
        let lines = self.content.line_count();
        let zoom = (self.scale * 100.0).round() as u32;

        container(
            row![
                text(format!("Ln {}, Col {}", line, col)).size(12),
                iced::widget::Space::new().width(Length::Fill),
                text(format!("{} lines", lines)).size(12),
                iced::widget::Space::new().width(20),
                text(format!("{}%", zoom)).size(12),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([2, 8])
        .style(|theme: &Theme| container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            ..Default::default()
        })
        .into()
    }

    fn about_dialog(&self) -> Element<'_, Message> {
        let dialog = container(
            column![
                text("F4").size(20),
                text("A lightweight text editor").size(14),
                text("Dunno what else to say...").size(14),
                text("Built in rust btw").size(14),
                iced::widget::Space::new().height(10),
                button(text("OK").size(14))
                    .padding([4, 20])
                    .on_press(Message::CloseAbout),
            ]
            .spacing(6)
            .align_x(iced::Alignment::Center),
        )
        .padding(20)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(palette.background.strong.color.into()),
                border: iced::Border {
                    radius: 8.0.into(),
                    width: 1.0,
                    color: palette.background.weak.color,
                },
                ..Default::default()
            }
        });

        center(dialog).into()
    }

    fn view(&self) -> Element<'_, Message> {
        let mut col = column![self.menu_bar()];

        if self.show_panel {
            col = col.push(self.search_panel());
        }

        let wrapping = if self.word_wrap {
            Wrapping::Word
        } else {
            Wrapping::None
        };

        col = col.push(
            text_editor(&self.content)
                .height(Fill)
                .wrapping(wrapping)
                .on_action(Message::Edit),
        );

        col = col.push(self.status_bar());

        if self.show_about {
            stack![col, self.about_dialog()].into()
        } else {
            col.into()
        }
    }
}

fn dialog_button(label: &str, msg: Message) -> Element<'_, Message> {
    button(text(label).size(13))
        .padding([3, 8])
        .on_press(msg)
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();
            let base = button::Style {
                text_color: palette.background.base.text,
                background: Some(palette.background.strong.color.into()),
                border: iced::Border {
                    radius: 3.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            };
            match status {
                button::Status::Hovered | button::Status::Pressed => button::Style {
                    background: Some(palette.primary.strong.color.into()),
                    text_color: palette.primary.strong.text,
                    ..base
                },
                _ => base,
            }
        })
        .into()
}

fn menu_root(label: &str) -> Element<'_, Message> {
    button(text(label).size(14))
        .padding([4, 8])
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();
            let base = button::Style {
                text_color: palette.background.base.text,
                background: None,
                ..Default::default()
            };
            match status {
                button::Status::Hovered | button::Status::Pressed => button::Style {
                    background: Some(palette.background.weak.color.into()),
                    ..base
                },
                _ => base,
            }
        })
        .into()
}

fn menu_item<'a>(label: &'a str, shortcut: &'a str, msg: Message) -> Element<'a, Message> {
    button(
        row![
            text(label).size(14).width(Length::Fill),
            text(shortcut)
                .size(12)
                .color(iced::Color::from_rgb(0.5, 0.5, 0.5)),
        ]
        .width(Length::Fill),
    )
    .padding([4, 12])
    .width(Length::Fill)
    .style(|theme: &Theme, status| {
        let palette = theme.extended_palette();
        let base = button::Style {
            text_color: palette.background.base.text,
            background: None,
            ..Default::default()
        };
        match status {
            button::Status::Hovered | button::Status::Pressed => button::Style {
                background: Some(palette.primary.strong.color.into()),
                text_color: palette.primary.strong.text,
                ..Default::default()
            },
            _ => base,
        }
    })
    .on_press(msg)
    .into()
}

fn menu_item_disabled(label: &str) -> Element<'_, Message> {
    button(text(label).size(14).color(iced::Color::from_rgb(0.4, 0.4, 0.4)))
        .padding([4, 12])
        .width(Length::Fill)
        .style(|_theme: &Theme, _status| button::Style {
            text_color: iced::Color::from_rgb(0.4, 0.4, 0.4),
            background: None,
            ..Default::default()
        })
        .into()
}

fn separator<'a>() -> Element<'a, Message> {
    iced::widget::container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(1)
        .style(|theme: &Theme| iced::widget::container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            ..Default::default()
        })
        .into()
}
