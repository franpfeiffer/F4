use std::hash::Hash;

use iced::advanced::subscription::{EventStream, Hasher, Recipe};
use iced::{event, keyboard, window, Event, Subscription};
use iced::advanced::subscription;
use iced::futures::StreamExt;

use crate::app::App;
use crate::message::{Message, VimMode, VimPending};

struct AppSubscription {
    vim_enabled: bool,
    vim_mode: VimMode,
    vim_operator: Option<char>,
    vim_awaits_char: bool,
}

impl Recipe for AppSubscription {
    type Output = Message;

    fn hash(&self, state: &mut Hasher) {
        self.vim_enabled.hash(state);
        self.vim_mode.hash(state);
        self.vim_operator.hash(state);
        self.vim_awaits_char.hash(state);
    }

    fn stream(self: Box<Self>, input: EventStream) -> iced::futures::stream::BoxStream<'static, Message> {
        let vim_enabled = self.vim_enabled;
        let vim_mode = self.vim_mode;
        let vim_operator = self.vim_operator;
        let vim_awaits_char = self.vim_awaits_char;
        input
            .filter_map(move |raw_event| {
                let msg = handle_event(raw_event, vim_enabled, vim_mode.clone(), vim_operator, vim_awaits_char);
                std::future::ready(msg)
            })
            .boxed()
    }
}

fn handle_event(raw_event: subscription::Event, vim_enabled: bool, vim_mode: VimMode, vim_operator: Option<char>, vim_awaits_char: bool) -> Option<Message> {
    let subscription::Event::Interaction { event, status, .. } = raw_event else {
        return None;
    };

    if let Event::Window(window::Event::CloseRequested) = &event {
        return Some(Message::WindowCloseRequested);
    }

    match &event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Control),
            ..
        }) => {
            return Some(Message::CtrlPressed);
        }
        Event::Keyboard(keyboard::Event::KeyReleased {
            key: keyboard::Key::Named(keyboard::key::Named::Control),
            ..
        }) => {
            return Some(Message::CtrlReleased);
        }
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            if !modifiers.control() {
                return Some(Message::CtrlReleased);
            }
        }
        _ => {}
    }

    if let Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        modifiers,
        physical_key,
        ..
    }) = event
    {
        if modifiers.control() {
            match physical_key {
                keyboard::key::Physical::Code(keyboard::key::Code::Equal) => {
                    return Some(Message::ZoomIn)
                }
                keyboard::key::Physical::Code(keyboard::key::Code::Minus) => {
                    return Some(Message::ZoomOut)
                }
                _ => {}
            }
        }

        if modifiers.is_empty() {
            if let keyboard::Key::Named(keyboard::key::Named::F6) = key.as_ref() {
                return Some(Message::ToggleVim);
            }
        }

        if vim_enabled && vim_mode == VimMode::Normal {
            if modifiers.control() {
                match key.as_ref() {
                    keyboard::Key::Character("d") => return Some(Message::VimKey('\x04')),
                    keyboard::Key::Character("u") => return Some(Message::VimKey('\x15')),
                    keyboard::Key::Character("r") => return Some(Message::VimKey('\x12')),
                    _ => {}
                }
                return None;
            }
            if modifiers.is_empty() {
                match key.as_ref() {
                    keyboard::Key::Named(keyboard::key::Named::Escape) => {
                        return Some(Message::VimEnterNormal);
                    }
                    keyboard::Key::Character(ch) => {
                        if let Some(c) = ch.chars().next() {
                            if vim_awaits_char || vim_operator.is_some() {
                                return Some(Message::VimKey(c));
                            }
                            return match c {
                                'i' => Some(Message::VimEnterInsert),
                                'a' => Some(Message::VimEnterInsertAppend),
                                'o' => Some(Message::VimEnterInsertNewlineBelow),
                                _ => Some(Message::VimKey(c)),
                            };
                        }
                    }
                    _ => {}
                }
                return None;
            }
            if modifiers.shift() && !modifiers.control() {
                match key.as_ref() {
                    keyboard::Key::Character(ch) => {
                        if let Some(c) = ch.chars().next() {
                            return match c {
                                'I' => Some(Message::VimEnterInsertLineStart),
                                'A' => Some(Message::VimEnterInsertLineEnd),
                                'O' => Some(Message::VimEnterInsertNewlineAbove),
                                'G' => Some(Message::VimKey('G')),
                                'P' => Some(Message::VimKey('P')),
                                'J' => Some(Message::VimKey('J')),
                                'D' => Some(Message::VimKey('D')),
                                'C' => Some(Message::VimKey('C')),
                                _ => None,
                            };
                        }
                    }
                    _ => {}
                }
                return None;
            }
            return None;
        }

        if vim_enabled && vim_mode == VimMode::Insert && modifiers.is_empty() {
            if let keyboard::Key::Named(keyboard::key::Named::Escape) = key.as_ref() {
                return Some(Message::VimEnterNormal);
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
}

impl App {
    pub fn subscription(&self) -> Subscription<Message> {
        let vim_awaits_char = matches!(
            self.vim_pending,
            Some(VimPending::ReplaceChar) | Some(VimPending::FindChar)
        );
        subscription::from_recipe(AppSubscription {
            vim_enabled: self.vim_enabled,
            vim_mode: self.vim_mode.clone(),
            vim_operator: self.vim_operator,
            vim_awaits_char,
        })
    }
}
