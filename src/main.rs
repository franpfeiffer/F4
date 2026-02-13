use iced::widget::{column, text_editor};
use iced::{Element, Fill, Font, Task, Theme};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .default_font(Font::MONOSPACE)
        .window_size((800.0, 600.0))
        .run()
}

struct App {
    content: text_editor::Content,
}

#[derive(Debug, Clone)]
enum Message {
    Edit(text_editor::Action),
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                content: text_editor::Content::new(),
            },
            Task::none(),
        )
    }

    fn title(&self) -> String {
        String::from("Untitled - 4f")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                self.content.perform(action);
                Task::none()
            }
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
