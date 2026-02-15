use iced::{event, keyboard, window, Event, Subscription};

use crate::app::App;
use crate::message::Message;

impl App {
    pub fn subscription(&self) -> Subscription<Message> {
        event::listen_with(|event, status, _window| {
            if let Event::Window(window::Event::CloseRequested) = &event {
                return Some(Message::WindowCloseRequested);
            }

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
}
