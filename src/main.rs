use iced::widget::{button, column, row, text, text_editor};
use iced::{keyboard, window, Element, Fill, Font, Length, Subscription, Task, Theme};
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
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                content: text_editor::Content::new(),
                current_file: None,
                is_modified: false,
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
        keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } => {
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
                        _ => {}
                    }
                }

                if modifiers.is_empty() {
                    if let keyboard::Key::Named(keyboard::key::Named::F5) = key.as_ref() {
                        return Some(Message::FormatDocument);
                    }
                }

                None
            }
            _ => None,
        })
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
            Item::new(menu_item("Select All", "Ctrl+A", Message::SelectAll)),
            Item::new(separator()),
            Item::new(menu_item("Format Document", "F5", Message::FormatDocument)),
        ])
        .max_width(220.0);

        let format_menu = Menu::new(vec![
            Item::new(menu_item_disabled("Word Wrap")),
            Item::new(menu_item_disabled("Font...")),
        ])
        .max_width(220.0);

        let view_menu = Menu::new(vec![
            Item::new(menu_item_disabled("Status Bar")),
        ])
        .max_width(220.0);

        let help_menu = Menu::new(vec![
            Item::new(menu_item_disabled("About F4")),
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
                let is_edit = action.is_edit();
                self.content.perform(action);
                if is_edit {
                    self.is_modified = true;
                }
                Task::none()
            }
            Message::New => {
                self.content = text_editor::Content::new();
                self.current_file = None;
                self.is_modified = false;
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
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            self.menu_bar(),
            text_editor(&self.content)
                .height(Fill)
                .on_action(Message::Edit)
        ]
        .into()
    }
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
