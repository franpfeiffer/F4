use iced::widget::text_editor;
use iced::Task;
use std::sync::Arc;

use crate::app::App;
use crate::format::format_document;
use crate::message::{Message, PendingAction};

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
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
                if self.is_modified {
                    self.pending_action = Some(PendingAction::New);
                    return Task::none();
                }
                self.content = text_editor::Content::new();
                self.current_file = None;
                self.is_modified = false;
                self.show_panel = false;
                self.find_matches.clear();
                self.current_match = None;
                Task::none()
            }
            Message::Open => {
                if self.is_modified {
                    self.pending_action = Some(PendingAction::Open);
                    return Task::none();
                }
                Task::perform(
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
            )
            }
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
                if let Some(action) = self.pending_action.take() {
                    match action {
                        PendingAction::New => self.update(Message::New),
                        PendingAction::Open => self.update(Message::Open),
                        PendingAction::Exit => iced::exit(),
                    }
                } else {
                    Task::none()
                }
            }
            Message::FileSaved(None) => Task::none(),
            Message::Exit => {
                if self.is_modified {
                    self.pending_action = Some(PendingAction::Exit);
                    return Task::none();
                }
                iced::exit()
            }
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
                let original = self.content.text();
                let formatted = format_document(&original);
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
            Message::WindowCloseRequested => {
                if self.is_modified {
                    self.pending_action = Some(PendingAction::Exit);
                    Task::none()
                } else {
                    iced::exit()
                }
            }
            Message::ConfirmSave => {
                self.update(Message::Save)
            }
            Message::ConfirmDiscard => {
                let action = self.pending_action.take();
                match action {
                    Some(PendingAction::New) => {
                        self.is_modified = false;
                        self.update(Message::New)
                    }
                    Some(PendingAction::Open) => {
                        self.is_modified = false;
                        self.update(Message::Open)
                    }
                    Some(PendingAction::Exit) => iced::exit(),
                    None => Task::none(),
                }
            }
            Message::ConfirmCancel => {
                self.pending_action = None;
                Task::none()
            }
        }
    }
}
