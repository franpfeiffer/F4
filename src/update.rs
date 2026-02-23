use iced::widget::text_editor;
use iced::widget::operation;
use iced::Task;
use std::sync::Arc;

use crate::app::{App, EDITOR_ID};
use crate::format::format_document;
use crate::message::{LineNumbers, Message, PendingAction, VimMode, VimPending};
use crate::subscription::{COMMAND_INPUT_ID, SEARCH_INPUT_ID};
use crate::undo_tree;

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

    fn vim_move_to_with_block(&mut self, line: usize, col: usize) {
        let text = self.content.text();
        let char_count = text.lines().nth(line).map(|l| l.chars().count()).unwrap_or(0);
        let col = if char_count == 0 { 0 } else { col.min(char_count.saturating_sub(1)) };
        self.vim_col = col;
        self.content.move_to(text_editor::Cursor {
            position: text_editor::Position { line, column: col },
            selection: None,
        });
    }

    fn vim_apply_block_cursor(&mut self) {
        let cursor = self.content.cursor();
        self.vim_col = cursor.position.column;
    }

    fn vim_normal_move(&mut self, c: char, count: usize) {
        let text = self.content.text();
        let lines: Vec<Vec<char>> = text.lines().map(|l| l.chars().collect()).collect();
        let max_line = lines.len().saturating_sub(1);
        let cursor = self.content.cursor();
        let line = cursor.position.line.min(max_line);

        let char_count = |l: usize| lines.get(l).map(|v| v.len()).unwrap_or(0);
        let is_word = |c: char| c.is_alphanumeric() || c == '_';

        match c {
            'j' | 'k' => {
                let new_line = if c == 'j' {
                    (line + count).min(max_line)
                } else {
                    line.saturating_sub(count)
                };
                let len = char_count(new_line);
                let col = if len == 0 { 0 } else { self.vim_col.min(len.saturating_sub(1)) };
                self.vim_move_to_with_block(new_line, col);
            }
            'h' => {
                let cur_col = cursor.position.column;
                let col = cur_col.saturating_sub(count);
                self.vim_col = col;
                self.vim_move_to_with_block(line, col);
            }
            'l' => {
                let cur_col = cursor.position.column;
                let len = char_count(line);
                let col = if len == 0 { 0 } else { (cur_col + count).min(len.saturating_sub(1)) };
                self.vim_col = col;
                self.vim_move_to_with_block(line, col);
            }
            '0' => {
                self.vim_col = 0;
                self.vim_move_to_with_block(line, 0);
            }
            '$' => {
                let col = char_count(line).saturating_sub(1);
                self.vim_col = col;
                self.vim_move_to_with_block(line, col);
            }
            'G' => {
                self.vim_col = 0;
                self.vim_move_to_with_block(max_line, 0);
            }
            'w' | 'e' => {
                for _ in 0..count {
                    self.content.perform(text_editor::Action::Move(text_editor::Motion::WordRight));
                }
                let c = self.content.cursor();
                self.vim_col = c.position.column;
                self.vim_move_to_with_block(c.position.line, c.position.column);
            }
            'b' => {
                let mut cur_line = line;
                let mut col = cursor.position.column;
                for _ in 0..count {
                    let chars = match lines.get(cur_line) { Some(v) => v, None => break };
                    let mut i = col;
                    if i > 0 { i -= 1; }
                    while i > 0 && !is_word(chars[i]) { i -= 1; }
                    while i > 0 && is_word(chars[i - 1]) { i -= 1; }
                    col = i;
                }
                self.vim_col = col;
                self.vim_move_to_with_block(cur_line, col);
            }
            _ => {}
        }
    }

    fn vim_reset_pending(&mut self) {
        self.vim_count = String::new();
    }

    fn vim_visual_selected_text(&self, head_line: usize, head_col: usize) -> String {
        let Some((anchor_line, anchor_col)) = self.vim_visual_anchor else { return String::new() };
        let text = self.content.text();
        let lines: Vec<&str> = text.lines().collect();

        if self.vim_mode == VimMode::VisualLine {
            let (start, end) = if anchor_line <= head_line {
                (anchor_line, head_line)
            } else {
                (head_line, anchor_line)
            };
            lines[start..=end.min(lines.len().saturating_sub(1))].join("\n") + "\n"
        } else {
            let ((sl, sc), (el, ec)) = if (anchor_line, anchor_col) <= (head_line, head_col) {
                ((anchor_line, anchor_col), (head_line, head_col))
            } else {
                ((head_line, head_col), (anchor_line, anchor_col))
            };
            if sl == el {
                lines.get(sl).map(|l| {
                    let chars: Vec<char> = l.chars().collect();
                    chars[sc..=(ec.min(chars.len().saturating_sub(1)))].iter().collect()
                }).unwrap_or_default()
            } else {
                let mut result = String::new();
                for l in sl..=el.min(lines.len().saturating_sub(1)) {
                    let line = lines.get(l).unwrap_or(&"");
                    let chars: Vec<char> = line.chars().collect();
                    if l == sl {
                        result.push_str(&chars[sc..].iter().collect::<String>());
                        result.push('\n');
                    } else if l == el {
                        result.push_str(&chars[..=ec.min(chars.len().saturating_sub(1))].iter().collect::<String>());
                    } else {
                        result.push_str(line);
                        result.push('\n');
                    }
                }
                result
            }
        }
    }

    fn vim_visual_apply_selection(&mut self, head_line: usize, head_col: usize) {
        let Some((anchor_line, anchor_col)) = self.vim_visual_anchor else { return };
        let text = self.content.text();
        let lines: Vec<&str> = text.lines().collect();
        let max_line = lines.len().saturating_sub(1);
        let anchor_line = anchor_line.min(max_line);
        let head_line = head_line.min(max_line);

        let line_end_col = |l: usize| lines.get(l).map(|s| s.chars().count()).unwrap_or(0);

        if self.vim_mode == VimMode::VisualLine {
            let (start_line, end_line) = if anchor_line <= head_line {
                (anchor_line, head_line)
            } else {
                (head_line, anchor_line)
            };
            self.content.move_to(text_editor::Cursor {
                position: text_editor::Position { line: start_line, column: 0 },
                selection: Some(text_editor::Position { line: end_line, column: line_end_col(end_line) }),
            });
        } else {
            self.content.move_to(text_editor::Cursor {
                position: text_editor::Position { line: head_line, column: head_col },
                selection: Some(text_editor::Position { line: anchor_line, column: anchor_col }),
            });
        }
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

    fn take_snapshot(&self) -> undo_tree::Snapshot {
        let cursor = self.content.cursor();
        undo_tree::Snapshot {
            text: self.content.text(),
            cursor_line: cursor.position.line,
            cursor_col: cursor.position.column,
        }
    }

    fn push_snapshot(&mut self) {
        let snap = self.take_snapshot();
        self.undo_tree.push(snap);
        self.last_snapshot_tick = self.changedtick;
    }

    fn apply_snapshot(&mut self, snap: &undo_tree::Snapshot) {
        self.content = text_editor::Content::with_text(&snap.text);
        self.vim_move_to_with_block(snap.cursor_line, snap.cursor_col);
    }

    fn preview_text(snapshot: &crate::undo_tree::Snapshot, current: &str) -> String {
        let snap_lines: Vec<&str> = snapshot.text.lines().collect();
        let cur_lines: Vec<&str> = current.lines().collect();
        let first_diff = snap_lines.iter().zip(cur_lines.iter())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| snap_lines.len().min(cur_lines.len()));
        let center = first_diff;
        let start = center.saturating_sub(3);
        let end = (center + 17).min(snap_lines.len());
        snap_lines[start..end].join("\n")
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
                    self.changedtick += 1;
                    if self.show_panel {
                        self.find_all_matches();
                    }
                }
                if self.vim_enabled && self.vim_mode == VimMode::Normal {
                    self.vim_apply_block_cursor();
                    return operation::focus(EDITOR_ID);
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
                self.undo_tree.reset(undo_tree::Snapshot { text: String::new(), cursor_line: 0, cursor_col: 0 });
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
                self.undo_tree.reset(undo_tree::Snapshot { text: text.clone(), cursor_line: 0, cursor_col: 0 });
                Task::none()
            }
            Message::FileOpened(None) => Task::none(),
            Message::Save => {
                if self.changedtick != self.last_snapshot_tick {
                    self.push_snapshot();
                }
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
            Message::Undo => {
                if let Some(snap) = self.undo_tree.undo() {
                    self.apply_snapshot(&snap);
                    self.is_modified = true;
                }
                Task::none()
            }
            Message::Redo => {
                if let Some(snap) = self.undo_tree.redo() {
                    self.apply_snapshot(&snap);
                    self.is_modified = true;
                }
                Task::none()
            }
            Message::Tick => Task::none(),
            Message::ToggleUndoPanel => {
                self.show_undo_panel = !self.show_undo_panel;
                if !self.show_undo_panel {
                    self.undo_panel_focused = false;
                }
                Task::none()
            }
            Message::UndoPanelFocusToggle => {
                if self.show_undo_panel {
                    self.undo_panel_focused = !self.undo_panel_focused;
                    if self.undo_panel_focused && self.selected_undo_node.is_none() {
                        self.selected_undo_node = Some(self.undo_tree.current);
                    }
                }
                Task::none()
            }
            Message::UndoPanelMoveSelection(delta) => {
                if !self.show_undo_panel || !self.undo_panel_focused {
                    return Task::none();
                }
                let n = self.undo_tree.nodes.len();
                if n == 0 { return Task::none(); }
                let current = self.selected_undo_node.unwrap_or(self.undo_tree.current);
                let new_id = if delta > 0 {
                    (current + 1).min(n - 1)
                } else {
                    current.saturating_sub(1)
                };
                self.selected_undo_node = Some(new_id);
                let current = self.content.text();
                self.undo_preview_text = self.undo_tree.nodes.get(new_id)
                    .map(|n| Self::preview_text(&n.snapshot, &current))
                    .unwrap_or_default();
                Task::none()
            }
            Message::UndoPanelConfirm => {
                if let Some(id) = self.selected_undo_node {
                    return self.update(Message::UndoTreeJump(id));
                }
                Task::none()
            }
            Message::UndoTreeSelect(id) => {
                if self.selected_undo_node == Some(id) {
                    return self.update(Message::UndoTreeJump(id));
                }
                self.selected_undo_node = Some(id);
                let current = self.content.text();
                self.undo_preview_text = self.undo_tree.nodes.get(id)
                    .map(|n| Self::preview_text(&n.snapshot, &current))
                    .unwrap_or_default();
                Task::none()
            }
            Message::UndoTreeJump(id) => {
                if let Some(snap) = self.undo_tree.jump_to(id) {
                    self.apply_snapshot(&snap);
                    self.is_modified = true;
                    self.selected_undo_node = None;
                    self.undo_panel_focused = false;
                }
                operation::focus(EDITOR_ID)
            }
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
                        self.push_snapshot();
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
                    self.push_snapshot();
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
                        self.push_snapshot();
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
                    self.push_snapshot();
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
            Message::VimEnterSearch(forward) => {
                self.vim_mode = VimMode::Search;
                self.vim_search_forward = forward;
                self.vim_search_query = String::new();
                operation::focus(SEARCH_INPUT_ID)
            }
            Message::VimSearchChanged(q) => {
                self.vim_search_query = q;
                Task::none()
            }
            Message::VimSearchSubmit => {
                let query = self.vim_search_query.clone();
                self.vim_mode = VimMode::Normal;
                if !query.is_empty() {
                    self.find_query = query;
                    self.case_sensitive = false;
                    self.find_all_matches();
                    if !self.find_matches.is_empty() {
                        let cursor = self.content.cursor();
                        let cur_line = cursor.position.line;
                        let cur_col = cursor.position.column;
                        let next = if self.vim_search_forward {
                            self.find_matches.iter().position(|&(l, c)| {
                                (l, c) > (cur_line, cur_col)
                            }).unwrap_or(0)
                        } else {
                            self.find_matches.iter().rposition(|&(l, c)| {
                                (l, c) < (cur_line, cur_col)
                            }).unwrap_or(self.find_matches.len().saturating_sub(1))
                        };
                        self.navigate_to_match(next);
                    }
                }
                operation::focus(EDITOR_ID)
            }
            Message::ToggleLineNumbers => {
                self.line_numbers = match self.line_numbers {
                    LineNumbers::None => LineNumbers::Absolute,
                    LineNumbers::Absolute => LineNumbers::Relative,
                    LineNumbers::Relative => LineNumbers::None,
                };
                Task::none()
            }
            Message::ToggleVim => {
                self.vim_enabled = !self.vim_enabled;
                self.vim_mode = VimMode::Insert;
                self.vim_pending = None;
                self.vim_count = String::new();
                self.vim_operator = None;
                self.vim_command = String::new();
                self.vim_visual_anchor = None;
                self.vim_visual_head = (0, 0);
                Task::none()
            }
            Message::VimEnterNormal => {
                if self.vim_mode == VimMode::Insert {
                    if self.changedtick != self.last_snapshot_tick {
                        self.push_snapshot();
                    }
                }
                self.vim_mode = VimMode::Normal;
                self.vim_pending = None;
                self.vim_count = String::new();
                self.vim_operator = None;
                self.vim_command = String::new();
                self.vim_visual_anchor = None;
                self.vim_visual_head = (0, 0);
                if self.show_panel {
                    self.show_panel = false;
                    self.find_matches.clear();
                    self.current_match = None;
                }
                self.vim_apply_block_cursor();
                operation::focus(EDITOR_ID)
            }
            Message::VimEnterVisual => {
                let cursor = self.content.cursor();
                let (line, col) = (cursor.position.line, cursor.position.column);
                self.vim_mode = VimMode::Visual;
                self.vim_visual_anchor = Some((line, col));
                self.vim_visual_head = (line, col);
                self.vim_count = String::new();
                self.vim_operator = None;
                self.vim_visual_apply_selection(line, col);
                operation::focus(EDITOR_ID)
            }
            Message::VimEnterVisualLine => {
                let cursor = self.content.cursor();
                let (line, col) = (cursor.position.line, cursor.position.column);
                self.vim_mode = VimMode::VisualLine;
                self.vim_visual_anchor = Some((line, col));
                self.vim_visual_head = (line, col);
                self.vim_count = String::new();
                self.vim_operator = None;
                self.vim_visual_apply_selection(line, col);
                operation::focus(EDITOR_ID)
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

                if self.vim_mode == VimMode::Visual || self.vim_mode == VimMode::VisualLine {
                    let text = self.content.text();
                    let lines: Vec<&str> = text.lines().collect();
                    let (mut hl, mut hc) = self.vim_visual_head;

                    let moved = match c {
                        'h' => { for _ in 0..count { if hc > 0 { hc -= 1; } } true }
                        'l' => {
                            for _ in 0..count {
                                let len = lines.get(hl).map(|l| l.chars().count()).unwrap_or(0);
                                if hc < len { hc += 1; }
                            }
                            true
                        }
                        'j' => {
                            for _ in 0..count {
                                if hl + 1 < lines.len() { hl += 1; }
                            }
                            let len = lines.get(hl).map(|l| l.chars().count()).unwrap_or(0);
                            hc = hc.min(len);
                            true
                        }
                        'k' => {
                            for _ in 0..count { hl = hl.saturating_sub(1); }
                            let len = lines.get(hl).map(|l| l.chars().count()).unwrap_or(0);
                            hc = hc.min(len);
                            true
                        }
                        '0' => { hc = 0; true }
                        '$' => {
                            hc = lines.get(hl).map(|l| l.chars().count().saturating_sub(1)).unwrap_or(0);
                            true
                        }
                        'w' | 'e' => {
                            for _ in 0..count {
                                let chars: Vec<char> = lines.get(hl).unwrap_or(&"").chars().collect();
                                let mut i = hc;
                                while i < chars.len() && chars[i].is_alphanumeric() { i += 1; }
                                loop {
                                    while i < chars.len() && !chars[i].is_alphanumeric() { i += 1; }
                                    if i < chars.len() { break; }
                                    if hl + 1 >= lines.len() { break; }
                                    hl += 1; i = 0;
                                    if lines.get(hl).and_then(|l| l.chars().next()).map(|c| c.is_alphanumeric()).unwrap_or(false) { break; }
                                }
                                hc = i;
                            }
                            true
                        }
                        'b' => {
                            for _ in 0..count {
                                let line = lines.get(hl).unwrap_or(&"");
                                let chars: Vec<char> = line.chars().collect();
                                let mut i = hc;
                                if i > 0 { i -= 1; }
                                while i > 0 && !chars[i].is_alphanumeric() { i -= 1; }
                                while i > 0 && chars[i - 1].is_alphanumeric() { i -= 1; }
                                hc = i;
                            }
                            true
                        }
                        'G' => { hl = lines.len().saturating_sub(1); hc = 0; true }
                        'g' => {
                            if self.vim_pending == Some(VimPending::G) {
                                self.vim_pending = None;
                                hl = 0; hc = 0;
                                self.vim_visual_head = (hl, hc);
                                self.vim_visual_apply_selection(hl, hc);
                                self.vim_count = String::new();
                            } else {
                                self.vim_pending = Some(VimPending::G);
                            }
                            return Task::none();
                        }
                        _ => false,
                    };

                    if moved {
                        self.vim_visual_head = (hl, hc);
                        self.vim_visual_apply_selection(hl, hc);
                        self.vim_count = String::new();
                        return Task::none();
                    }

                    match c {
                        'y' => {
                            self.vim_register = self.vim_visual_selected_text(hl, hc);
                            self.vim_visual_apply_selection(hl, hc);
                            self.vim_mode = VimMode::Normal;
                            self.vim_visual_anchor = None;
                        }
                        'd' | 'x' => {
                            self.vim_register = self.vim_visual_selected_text(hl, hc);
                            self.vim_visual_apply_selection(hl, hc);
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                            self.vim_mode = VimMode::Normal;
                            self.vim_visual_anchor = None;
                            self.is_modified = true;
                            self.push_snapshot();
                        }
                        'c' => {
                            self.vim_register = self.vim_visual_selected_text(hl, hc);
                            self.vim_visual_apply_selection(hl, hc);
                            self.content.perform(text_editor::Action::Edit(text_editor::Edit::Delete));
                            self.vim_mode = VimMode::Insert;
                            self.vim_visual_anchor = None;
                            self.is_modified = true;
                            return operation::focus(EDITOR_ID);
                        }
                        _ => {}
                    }
                    self.vim_count = String::new();
                    return Task::none();
                }

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
                        if let Some(op) = self.vim_operator.take() {
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
                            self.vim_do_motion_op(op, motion, count);
                        } else {
                            self.vim_normal_move(c, count);
                        }
                    }
                    'g' => {
                        if self.vim_pending == Some(VimPending::G) {
                            self.vim_pending = None;
                            self.vim_operator = None;
                            self.vim_normal_move('G', 1);
                            self.vim_move_to_with_block(0, 0);
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
                    'u' => { return self.update(Message::Undo); }
                    '\x12' => { return self.update(Message::Redo); }
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
                            self.push_snapshot();
                        } else {
                            self.vim_operator = Some('d');
                            self.vim_count = String::new();
                            return Task::none();
                        }
                    }
                    'D' => {
                        self.vim_do_motion_op('d', text_editor::Motion::End, 1);
                        self.is_modified = true;
                        self.push_snapshot();
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
                            self.push_snapshot();
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
                            self.push_snapshot();
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
                } else if self.vim_mode == VimMode::Normal {
                    self.vim_apply_block_cursor();
                    operation::focus(EDITOR_ID)
                } else {
                    Task::none()
                }
            }
        }
    }
}
