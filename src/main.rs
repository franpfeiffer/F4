use iced::widget::{column, text_editor};
use iced::{keyboard, window, Element, Fill, Font, Subscription, Task, Theme};
use std::path::PathBuf;

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

                None
            }
            _ => None,
        })
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
        }
    }

    fn view(&self) -> Element<'_, Message> {
        column![
            text_editor(&self.content)
                .height(Fill)
                .on_action(Message::Edit)
        ]
        .into()
    }
}
