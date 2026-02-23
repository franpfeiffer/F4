use iced::Color;
use iced::advanced::text::highlighter::{self, Highlighter};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub struct FindHighlightSettings {
    pub matches: Vec<(usize, usize)>,
    pub query_len: usize,
    pub current_match: Option<usize>,
}

pub struct FindHighlighter {
    settings: FindHighlightSettings,
    current_line: usize,
}

pub struct FindHighlight {
    pub is_current: bool,
}

impl Highlighter for FindHighlighter {
    type Settings = FindHighlightSettings;
    type Highlight = FindHighlight;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, FindHighlight)>;

    fn new(settings: &Self::Settings) -> Self {
        Self {
            settings: settings.clone(),
            current_line: 0,
        }
    }

    fn update(&mut self, new_settings: &Self::Settings) {
        if self.settings != *new_settings {
            self.settings = new_settings.clone();
            self.current_line = 0;
        }
    }

    fn change_line(&mut self, line: usize) {
        self.current_line = line;
    }

    fn highlight_line(&mut self, _line: &str) -> Self::Iterator<'_> {
        let line = self.current_line;
        self.current_line += 1;

        let mut spans = Vec::new();
        if self.settings.query_len == 0 {
            return spans.into_iter();
        }

        for (i, &(match_line, col)) in self.settings.matches.iter().enumerate() {
            if match_line == line {
                let is_current = self.settings.current_match == Some(i);
                spans.push((col..col + self.settings.query_len, FindHighlight { is_current }));
            }
        }

        spans.into_iter()
    }

    fn current_line(&self) -> usize {
        self.current_line
    }
}

pub fn format_highlight(
    highlight: &FindHighlight,
    _theme: &iced::Theme,
) -> highlighter::Format<iced::Font> {
    highlighter::Format {
        color: Some(if highlight.is_current {
            Color::from_rgb(1.0, 0.6, 0.0)
        } else {
            Color::from_rgb(1.0, 0.9, 0.2)
        }),
        font: None,
    }
}
