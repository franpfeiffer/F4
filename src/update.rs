use iced::widget::text_editor;
use iced::widget::operation;
use iced::Task;
use std::sync::Arc;

use crate::app::{App, EDITOR_ID};
use crate::format::format_document;
use crate::message::{Message, PendingAction, VimMode, VimPending};
use crate::subscription::COMMAND_INPUT_ID;

impl App {
    fn vim_do_delete_lines(&mut self, count: usize) {
        for _ in 0..count {
            self.content.perform(text_editor::Action::Move(text_editor::Motion::Home));
            self.content.perform(text_editor::Action::Select(text_editor::Motion::Down));
            if let Some(sel) = self.content.selection() {
                self.vim_register = sel;
            }
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
        }
    }

    fn vim_do_yank_lines(&mut self, count: usize) {
        let cursor = self.content.cursor();
        let line = cursor.position.line;
        let text = self.content.text();
        let lines: Vec<&str> = text.split('\n').collect();
        let end = (line + count).min(lines.len());
        let yanked: String = lines[line..end].join("\n") + "\n";
        self.vim_register = yanked;
    }

    fn vim_do_motion_op(&mut self, op: char, motion: text_editor::Motion, count: usize) {
        for _ in 0..count {
            self.content.perform(text_editor::Action::Select(motion));
        }
        if let Some(sel) = self.content.selection() {
            self.vim_register = sel.clone();
            if op == 'd' || op == 'c' {
                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                self.is_modified = true;
            }
        }
        if op == 'c' {
            self.vim_mode = VimMode::Insert;
        }
    }

    fn vim_do_text_object(&mut self, op: char, modifier: char, object: char) {
        let text = self.content.text();
        let cursor = self.content.cursor();
        let pos = self.byte_offset_of(cursor.position.line, cursor.position.column, &text);

        let (open, close) = match object {
            '(' | ')' | 'b' => ('(', ')'),
            '[' | ']' => ('[', ']'),
            '{' | '}' | 'B' => ('{', '}'),
            '<' | '>' => ('<', '>'),
            '"' => ('"', '"'),
            '\'' => ('\'', '\''),
            '`' => ('`', '`'),
            'w' => {
                self.vim_do_word_object(op, modifier);
                return;
            }
            _ => return,
        };

        let bytes = text.as_bytes();
        let (start, end) = if open == close {
            let before = bytes[..pos].iter().rposition(|&b| b == open as u8);
            let after = bytes[pos..].iter().position(|&b| b == close as u8).map(|i| pos + i);
            match (before, after) {
                (Some(s), Some(e)) => (s, e),
                _ => return,
            }
        } else {
            let before = self.find_matching_open(&text, pos, open, close);
            let after = self.find_matching_close(&text, pos, open, close);
            match (before, after) {
                (Some(s), Some(e)) => (s, e),
                _ => return,
            }
        };

        let (sel_start, sel_end) = if modifier == 'i' {
            (start + 1, end)
        } else {
            (start, end + 1)
        };

        let yanked = text[sel_start..sel_end].to_string();
        self.vim_register = yanked;

        let start_pos = self.position_of_byte_offset(sel_start, &text);
        let end_pos = self.position_of_byte_offset(sel_end, &text);

        self.content.move_to(text_editor::Cursor {
            position: text_editor::Position { line: start_pos.0, column: start_pos.1 },
            selection: Some(text_editor::Position { line: end_pos.0, column: end_pos.1 }),
        });

        if op == 'd' || op == 'c' {
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
            self.is_modified = true;
        }
        if op == 'c' {
            self.vim_mode = VimMode::Insert;
        }
    }

    fn vim_do_word_object(&mut self, op: char, modifier: char) {
        let text = self.content.text();
        let cursor = self.content.cursor();
        let pos = self.byte_offset_of(cursor.position.line, cursor.position.column, &text);
        let bytes = text.as_bytes();

        let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

        let mut start = pos;
        while start > 0 && is_word(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = pos;
        while end < bytes.len() && is_word(bytes[end]) {
            end += 1;
        }

        if modifier == 'a' && end < bytes.len() && bytes[end] == b' ' {
            end += 1;
        }

        let yanked = text[start..end].to_string();
        self.vim_register = yanked;

        let start_pos = self.position_of_byte_offset(start, &text);
        let end_pos = self.position_of_byte_offset(end, &text);

        self.content.move_to(text_editor::Cursor {
            position: text_editor::Position { line: start_pos.0, column: start_pos.1 },
            selection: Some(text_editor::Position { line: end_pos.0, column: end_pos.1 }),
        });

        if op == 'd' || op == 'c' {
            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
            self.is_modified = true;
        }
        if op == 'c' {
            self.vim_mode = VimMode::Insert;
        }
    }

    fn find_matching_open(&self, text: &str, pos: usize, open: char, close: char) -> Option<usize> {
        let bytes = text.as_bytes();
        let mut depth = 0i32;
        let mut i = pos as i64;
        while i >= 0 {
            let b = bytes[i as usize];
            if b == close as u8 { depth += 1; }
            else if b == open as u8 {
                if depth == 0 { return Some(i as usize); }
                depth -= 1;
            }
            i -= 1;
        }
        None
    }

    fn find_matching_close(&self, text: &str, pos: usize, open: char, close: char) -> Option<usize> {
        let bytes = text.as_bytes();
        let mut depth = 0i32;
        let mut i = pos;
        while i < bytes.len() {
            let b = bytes[i];
            if b == open as u8 { depth += 1; }
            else if b == close as u8 {
                if depth == 0 { return Some(i); }
                depth -= 1;
            }
            i += 1;
        }
        None
    }

    fn vim_reset_pending(&mut self) {
        self.vim_count = String::new();
    }

    fn vim_do_find_char(&mut self, ch: char, forward: bool, inclusive: bool, _count: usize) {
        let text = self.content.text();
        let cursor = self.content.cursor();
        let pos = self.byte_offset_of(cursor.position.line, cursor.position.column, &text);
        let line_start = text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line_end = text[pos..].find('\n').map(|i| pos + i).unwrap_or(text.len());
        let line_text = &text[line_start..line_end];
        let col = pos - line_start;

        let found_col = if forward {
            line_text[col + 1..].find(ch).map(|i| col + 1 + i)
        } else {
            line_text[..col].rfind(ch)
        };

        if let Some(target_col) = found_col {
            let final_col = if inclusive {
                target_col
            } else if forward {
                target_col.saturating_sub(1)
            } else {
                (target_col + 1).min(line_text.len())
            };
            let target_line = cursor.position.line;
            self.content.move_to(text_editor::Cursor {
                position: text_editor::Position { line: target_line, column: final_col },
                selection: None,
            });
        }
    }

    fn vim_do_jump_matching(&mut self) {
        let text = self.content.text();
        let cursor = self.content.cursor();
        let pos = self.byte_offset_of(cursor.position.line, cursor.position.column, &text);
        let bytes = text.as_bytes();
        let ch = bytes.get(pos).copied().unwrap_or(0) as char;
        let (open, close, forward) = match ch {
            '(' => ('(', ')', true),
            ')' => ('(', ')', false),
            '[' => ('[', ']', true),
            ']' => ('[', ']', false),
            '{' => ('{', '}', true),
            '}' => ('{', '}', false),
            _ => return,
        };
        let target = if forward {
            self.find_matching_close(&text, pos + 1, open, close)
        } else {
            self.find_matching_open(&text, pos.saturating_sub(1), open, close)
        };
        if let Some(t) = target {
            let p = self.position_of_byte_offset(t, &text);
            self.content.move_to(text_editor::Cursor {
                position: text_editor::Position { line: p.0, column: p.1 },
                selection: None,
            });
        }
    }

    fn vim_word_under_cursor(&self) -> String {
        let text = self.content.text();
        let cursor = self.content.cursor();
        let pos = self.byte_offset_of(cursor.position.line, cursor.position.column, &text);
        let bytes = text.as_bytes();
        let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let mut start = pos;
        while start > 0 && is_word(bytes[start - 1]) { start -= 1; }
        let mut end = pos;
        while end < bytes.len() && is_word(bytes[end]) { end += 1; }
        text[start..end].to_string()
    }

    fn byte_offset_of(&self, line: usize, col: usize, text: &str) -> usize {
        let mut offset = 0;
        for (i, l) in text.split('\n').enumerate() {
            if i == line {
                return offset + col.min(l.len());
            }
            offset += l.len() + 1;
        }
        offset
    }

    fn position_of_byte_offset(&self, offset: usize, text: &str) -> (usize, usize) {
        let mut line = 0;
        let mut col = 0;
        for (i, ch) in text.char_indices() {
            if i == offset { return (line, col); }
            if ch == '\n' { line += 1; col = 0; } else { col += 1; }
        }
        (line, col)
    }
}

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Edit(action) => {
                if self.ctrl_held {
                    if let text_editor::Action::Edit(text_editor::Edit::Insert(_)) = &action {
                        return Task::none();
                    }
                }
                if self.vim_enabled && self.vim_mode == VimMode::Normal {
                    if let text_editor::Action::Edit(_) = &action {
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
            Message::ToggleVim => {
                self.vim_enabled = !self.vim_enabled;
                self.vim_mode = VimMode::Insert;
                self.vim_pending = None;
                self.vim_count = String::new();
                self.vim_operator = None;
                self.vim_command = String::new();
                Task::none()
            }
            Message::VimEnterNormal => {
                self.vim_mode = VimMode::Normal;
                self.vim_pending = None;
                self.vim_count = String::new();
                self.vim_operator = None;
                self.vim_command = String::new();
                if self.show_panel {
                    self.show_panel = false;
                    self.find_matches.clear();
                    self.current_match = None;
                }
                Task::none()
            }
            Message::VimEnterCommand => {
                self.vim_mode = VimMode::Command;
                self.vim_command = String::new();
                operation::focus(COMMAND_INPUT_ID)
            }
            Message::VimCommandChanged(cmd) => {
                self.vim_command = cmd;
                Task::none()
            }
            Message::VimCommandSubmit => {
                let cmd = self.vim_command.trim().to_string();
                self.vim_mode = VimMode::Normal;
                self.vim_command = String::new();
                match cmd.as_str() {
                    "w" => return self.update(Message::Save),
                    "w!" => return self.update(Message::SaveAs),
                    "q" => return self.update(Message::Exit),
                    "q!" => return iced::exit(),
                    "wq" | "x" => {
                        let save = self.update(Message::Save);
                        return save;
                    }
                    _ => {}
                }
                Task::none()
            }
            Message::VimEnterInsert => {
                self.vim_mode = VimMode::Insert;
                eprintln!("VimEnterInsert -> focusing editor");
                operation::focus_next()
            }
            Message::VimEnterInsertAppend => {
                self.vim_mode = VimMode::Insert;
                self.content.perform(text_editor::Action::Move(text_editor::Motion::Right));
                operation::focus(EDITOR_ID)
            }
            Message::VimEnterInsertLineStart => {
                self.vim_mode = VimMode::Insert;
                self.content.perform(text_editor::Action::Move(text_editor::Motion::Home));
                operation::focus(EDITOR_ID)
            }
            Message::VimEnterInsertLineEnd => {
                self.vim_mode = VimMode::Insert;
                self.content.perform(text_editor::Action::Move(text_editor::Motion::End));
                operation::focus(EDITOR_ID)
            }
            Message::VimEnterInsertNewlineBelow => {
                self.vim_mode = VimMode::Insert;
                self.content.perform(text_editor::Action::Move(text_editor::Motion::End));
                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Enter));
                self.is_modified = true;
                operation::focus(EDITOR_ID)
            }
            Message::VimEnterInsertNewlineAbove => {
                self.vim_mode = VimMode::Insert;
                self.content.perform(text_editor::Action::Move(text_editor::Motion::Home));
                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Enter));
                self.content.perform(text_editor::Action::Move(text_editor::Motion::Up));
                self.is_modified = true;
                operation::focus(EDITOR_ID)
            }
            Message::VimKey(c) => {
                let count = self.vim_count.parse::<usize>().unwrap_or(1);

                if let Some(VimPending::ReplaceChar) = self.vim_pending.take() {
                    self.vim_count = String::new();
                    self.vim_operator = None;
                    self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                    self.content.perform(text_editor::Action::Edit(text_editor::Edit::Insert(c)));
                    self.content.perform(text_editor::Action::Move(text_editor::Motion::Left));
                    self.is_modified = true;
                    return Task::none();
                }

                if let Some(VimPending::FindChar) = self.vim_pending.take() {
                    let forward = self.vim_find_last.map(|(_, f, _)| f).unwrap_or(true);
                    let inclusive = self.vim_find_last.map(|(_, _, i)| i).unwrap_or(true);
                    self.vim_find_last = Some((c, forward, inclusive));
                    self.vim_count = String::new();
                    self.vim_operator = None;
                    self.vim_do_find_char(c, forward, inclusive, 1);
                    return Task::none();
                }

                if let Some(VimPending::TextObjectModifier(modifier)) = self.vim_pending.take() {
                    let op = self.vim_operator.take().unwrap_or('y');
                    self.vim_count = String::new();
                    self.vim_do_text_object(op, modifier, c);
                    return Task::none();
                }

                if c.is_ascii_digit() && (c != '0' || !self.vim_count.is_empty()) {
                    self.vim_count.push(c);
                    return Task::none();
                }

                self.vim_reset_pending();

                match c {
                    'h' | 'j' | 'k' | 'l' | 'w' | 'e' | 'b' | '0' | '$' | 'G' => {
                        let motion = match c {
                            'h' => text_editor::Motion::Left,
                            'j' => text_editor::Motion::Down,
                            'k' => text_editor::Motion::Up,
                            'l' => text_editor::Motion::Right,
                            'w' | 'e' => text_editor::Motion::WordRight,
                            'b' => text_editor::Motion::WordLeft,
                            '0' => text_editor::Motion::Home,
                            '$' => text_editor::Motion::End,
                            'G' => text_editor::Motion::DocumentEnd,
                            _ => unreachable!(),
                        };
                        if let Some(op) = self.vim_operator.take() {
                            self.vim_do_motion_op(op, motion, count);
                        } else {
                            for _ in 0..count {
                                self.content.perform(text_editor::Action::Move(motion));
                            }
                        }
                    }
                    'g' => {
                        if self.vim_pending == Some(VimPending::G) {
                            self.vim_pending = None;
                            self.vim_operator = None;
                            self.content.perform(text_editor::Action::Move(text_editor::Motion::DocumentStart));
                        } else {
                            self.vim_pending = Some(VimPending::G);
                            return Task::none();
                        }
                    }
                    'i' | 'a' if self.vim_operator.is_some() => {
                        self.vim_pending = Some(VimPending::TextObjectModifier(c));
                        return Task::none();
                    }
                    'r' => {
                        self.vim_pending = Some(VimPending::ReplaceChar);
                        return Task::none();
                    }
                    'f' | 't' => {
                        self.vim_find_last = Some(('\0', true, c == 'f'));
                        self.vim_pending = Some(VimPending::FindChar);
                        return Task::none();
                    }
                    'F' | 'T' => {
                        self.vim_find_last = Some(('\0', false, c == 'F'));
                        self.vim_pending = Some(VimPending::FindChar);
                        return Task::none();
                    }
                    ';' => {
                        if let Some((ch, forward, inclusive)) = self.vim_find_last {
                            for _ in 0..count {
                                self.vim_do_find_char(ch, forward, inclusive, 1);
                            }
                        }
                    }
                    ',' => {
                        if let Some((ch, forward, inclusive)) = self.vim_find_last {
                            for _ in 0..count {
                                self.vim_do_find_char(ch, !forward, inclusive, 1);
                            }
                        }
                    }
                    '%' => {
                        self.vim_do_jump_matching();
                    }
                    '*' => {
                        let word = self.vim_word_under_cursor();
                        if !word.is_empty() {
                            self.find_query = word;
                            self.find_all_matches();
                            let next = self.current_match.map(|i| (i + 1) % self.find_matches.len().max(1)).unwrap_or(0);
                            if !self.find_matches.is_empty() {
                                self.navigate_to_match(next);
                            }
                        }
                    }
                    'n' => {
                        if !self.find_matches.is_empty() {
                            let next = self.current_match.map(|i| (i + 1) % self.find_matches.len()).unwrap_or(0);
                            self.navigate_to_match(next);
                        }
                    }
                    'N' => {
                        if !self.find_matches.is_empty() {
                            let prev = match self.current_match {
                                Some(0) => self.find_matches.len() - 1,
                                Some(i) => i - 1,
                                None => self.find_matches.len() - 1,
                            };
                            self.navigate_to_match(prev);
                        }
                    }
                    'J' => {
                        for _ in 0..count {
                            self.content.perform(text_editor::Action::Move(text_editor::Motion::End));
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Insert(' ')));
                            self.content.perform(text_editor::Action::Move(text_editor::Motion::Left));
                        }
                        self.is_modified = true;
                    }
                    '~' => {
                        let text = self.content.text();
                        let cursor = self.content.cursor();
                        let pos = self.byte_offset_of(cursor.position.line, cursor.position.column, &text);
                        if let Some(ch) = text[pos..].chars().next() {
                            let toggled = if ch.is_uppercase() {
                                ch.to_lowercase().next().unwrap_or(ch)
                            } else {
                                ch.to_uppercase().next().unwrap_or(ch)
                            };
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Insert(toggled)));
                            self.content.perform(text_editor::Action::Move(text_editor::Motion::Left));
                            self.is_modified = true;
                        }
                    }
                    'u' => {
                        self.content.perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
                        self.content.perform(text_editor::Action::Edit(text_editor::Edit::Backspace));
                    }
                    '\x12' => {
                        // Ctrl+R redo — not directly available in iced, noop for now
                    }
                    '\x04' => {
                        for _ in 0..15 * count {
                            self.content.perform(text_editor::Action::Move(text_editor::Motion::Down));
                        }
                    }
                    '\x15' => {
                        for _ in 0..15 * count {
                            self.content.perform(text_editor::Action::Move(text_editor::Motion::Up));
                        }
                    }
                    'd' => {
                        if self.vim_operator == Some('d') {
                            self.vim_do_delete_lines(count);
                            self.vim_operator = None;
                            self.is_modified = true;
                        } else {
                            self.vim_operator = Some('d');
                            self.vim_count = String::new();
                            return Task::none();
                        }
                    }
                    'D' => {
                        self.vim_do_motion_op('d', text_editor::Motion::End, 1);
                        self.is_modified = true;
                    }
                    'y' => {
                        if self.vim_operator == Some('y') {
                            self.vim_do_yank_lines(count);
                            self.vim_operator = None;
                        } else {
                            self.vim_operator = Some('y');
                            self.vim_count = String::new();
                            return Task::none();
                        }
                    }
                    'c' => {
                        if self.vim_operator == Some('c') {
                            self.vim_do_delete_lines(count);
                            self.vim_operator = None;
                            self.vim_mode = VimMode::Insert;
                            self.is_modified = true;
                        } else {
                            self.vim_operator = Some('c');
                            self.vim_count = String::new();
                            return Task::none();
                        }
                    }
                    'C' => {
                        self.vim_do_motion_op('c', text_editor::Motion::End, 1);
                        self.is_modified = true;
                    }
                    'x' => {
                        for _ in 0..count {
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                        }
                        self.is_modified = true;
                    }
                    's' => {
                        for _ in 0..count {
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                        }
                        self.vim_mode = VimMode::Insert;
                        self.is_modified = true;
                    }
                    'p' => {
                        if !self.vim_register.is_empty() {
                            let text = self.vim_register.clone();
                            if text.ends_with('\n') {
                                self.content.perform(text_editor::Action::Move(text_editor::Motion::End));
                                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Enter));
                                let paste = text.trim_end_matches('\n').to_string();
                                self.content.perform(text_editor::Action::Edit(
                                    text_editor::Edit::Paste(Arc::new(paste)),
                                ));
                            } else {
                                self.content.perform(text_editor::Action::Move(text_editor::Motion::Right));
                                self.content.perform(text_editor::Action::Edit(
                                    text_editor::Edit::Paste(Arc::new(text)),
                                ));
                            }
                            self.is_modified = true;
                        }
                    }
                    'P' => {
                        if !self.vim_register.is_empty() {
                            let text = self.vim_register.clone();
                            if text.ends_with('\n') {
                                self.content.perform(text_editor::Action::Move(text_editor::Motion::Home));
                                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Enter));
                                self.content.perform(text_editor::Action::Move(text_editor::Motion::Up));
                                let paste = text.trim_end_matches('\n').to_string();
                                self.content.perform(text_editor::Action::Edit(
                                    text_editor::Edit::Paste(Arc::new(paste)),
                                ));
                            } else {
                                self.content.perform(text_editor::Action::Edit(
                                    text_editor::Edit::Paste(Arc::new(text)),
                                ));
                            }
                            self.is_modified = true;
                        }
                    }
                    '>' => {
                        for _ in 0..count {
                            self.content.perform(text_editor::Action::Move(text_editor::Motion::Home));
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new("    ".to_string()))));
                        }
                        self.is_modified = true;
                    }
                    '<' => {
                        for _ in 0..count {
                            self.content.perform(text_editor::Action::Move(text_editor::Motion::Home));
                            let text = self.content.text();
                            let cursor = self.content.cursor();
                            let line_text = text.lines().nth(cursor.position.line).unwrap_or("");
                            let spaces = line_text.len() - line_text.trim_start_matches(' ').len();
                            let remove = spaces.min(4);
                            for _ in 0..remove {
                                self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                            }
                        }
                        self.is_modified = true;
                    }
                    _ => {}
                }
                self.vim_count = String::new();
                if self.vim_mode == VimMode::Insert {
                    operation::focus(EDITOR_ID)
                } else {
                    Task::none()
                }
            }
        }
    }
}
