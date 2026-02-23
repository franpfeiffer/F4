use iced::widget::{button, row, text};
use iced::{Element, Length, Theme};
use iced_aw::menu::{Item, Menu, MenuBar};

use crate::app::App;
use crate::message::{LineNumbers, Message};

impl App {
    pub fn menu_bar(&self) -> Element<'_, Message> {
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
            Item::new(menu_item("Undo", "u", Message::Undo)),
            Item::new(menu_item("Redo", "Ctrl+R", Message::Redo)),
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

        let vim_label = if self.vim_enabled { "Vim Mode ✓" } else { "Vim Mode" };
        let ln_label = match self.line_numbers {
            LineNumbers::None => "Line Numbers",
            LineNumbers::Absolute => "Line Numbers: Absolute ✓",
            LineNumbers::Relative => "Line Numbers: Relative ✓",
        };
        let undo_panel_label = if self.show_undo_panel { "Undo Tree ✓" } else { "Undo Tree" };
        let view_menu = Menu::new(vec![
            Item::new(menu_item_disabled("Status Bar")),
            Item::new(separator()),
            Item::new(menu_item(vim_label, "F6", Message::ToggleVim)),
            Item::new(separator()),
            Item::new(menu_item(ln_label, "", Message::ToggleLineNumbers)),
            Item::new(separator()),
            Item::new(menu_item(undo_panel_label, "Ctrl+T", Message::ToggleUndoPanel)),
        ])
        .max_width(250.0);

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
}

pub fn menu_root(label: &str) -> Element<'_, Message> {
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

pub fn menu_item<'a>(label: &'a str, shortcut: &'a str, msg: Message) -> Element<'a, Message> {
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

pub fn menu_item_disabled(label: &str) -> Element<'_, Message> {
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

pub fn separator<'a>() -> Element<'a, Message> {
    iced::widget::container(iced::widget::Space::new())
        .width(Length::Fill)
        .height(1)
        .style(|theme: &Theme| iced::widget::container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            ..Default::default()
        })
        .into()
}
