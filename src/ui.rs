use iced::widget::{button, center, column, container, row, text};
use iced::{Element, Length, Theme};

use crate::app::App;
use crate::message::Message;

impl App {
    pub fn status_bar(&self) -> Element<'_, Message> {
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

    pub fn about_dialog(&self) -> Element<'_, Message> {
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

    pub fn save_changes_dialog(&self) -> Element<'_, Message> {
        let name = match &self.current_file {
            Some(path) => path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| String::from("Untitled")),
            None => String::from("Untitled"),
        };

        let dialog = container(
            column![
                text(format!("Do you want to save changes to {}?", name)).size(14),
                iced::widget::Space::new().height(10),
                row![
                    dialog_button("Save", Message::ConfirmSave),
                    dialog_button("Don't Save", Message::ConfirmDiscard),
                    dialog_button("Cancel", Message::ConfirmCancel),
                ]
                .spacing(8),
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
}

pub fn dialog_button(label: &str, msg: Message) -> Element<'_, Message> {
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
