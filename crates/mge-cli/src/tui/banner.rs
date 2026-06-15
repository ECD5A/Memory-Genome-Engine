use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::tui::i18n::{tr, Language, TKey};
use crate::tui::theme;

pub const BANNER_LINES: &[&str] = &[
    "___   |/  /_  ****/**  ****/",
    "__  /|*/ /*  / __ __  **/",
    "_  /  / / / /*/ / _  /***",
    "/*/  /*/  _**_/  /**___/",
];

pub fn banner_lines(language: Language) -> Vec<Line<'static>> {
    let colors = [
        Color::Cyan,
        Color::Blue,
        Color::LightMagenta,
        Color::Magenta,
        Color::Green,
    ];
    let mut lines = Vec::new();
    for (index, line) in BANNER_LINES.iter().enumerate() {
        lines.push(Line::from(Span::styled(
            (*line).to_string(),
            Style::default().fg(colors[index % colors.len()]),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        tr(language, TKey::ProductName).to_string(),
        theme::title(),
    )));
    lines.push(Line::from(Span::styled(
        tr(language, TKey::Subtitle).to_string(),
        theme::subtitle(),
    )));
    lines
}
