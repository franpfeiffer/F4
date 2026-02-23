#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod cursor_editor;
mod format;
mod highlight;
mod menu;
mod message;
mod search;
mod subscription;
mod ui;
mod undo_tree;
mod undo_tree_widget;
mod update;

use iced::{window, Font};

use app::App;

const ICON: &[u8] = include_bytes!("../assets/icon.png");

fn main() -> iced::Result {
    let icon = window::icon::from_file_data(ICON, None).ok();

    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .scale_factor(|app| app.scale)
        .exit_on_close_request(false)
        .default_font(Font::MONOSPACE)
        .window(window::Settings {
            size: iced::Size::new(800.0, 600.0),
            icon,
            ..window::Settings::default()
        })
        .run()
}
