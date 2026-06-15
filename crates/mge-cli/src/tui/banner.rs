use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use crate::tui::i18n::{tr, Language, TKey};
use crate::tui::theme;

const BANNER_LEFT_PAD: &str = "    ";

pub const BANNER_LINES: &[&str] = &[
    "______  ___                                         _________                                       __________              _____",
    "___   |/  /___________ _____________________  __    __  ____/________________________ ________      ___  ____/_____________ ___(_)___________",
    "__  /|_/ /_  _ \\_  __ `__ \\  __ \\_  ___/_  / / /    _  / __ _  _ \\_  __ \\  __ \\_  __ `__ \\  _ \\     __  __/  __  __ \\_  __ `/_  /__  __ \\  _ \\",
    "_  /  / / /  __/  / / / / / /_/ /  /   _  /_/ /     / /_/ / /  __/  / / / /_/ /  / / / / /  __/     _  /___  _  / / /  /_/ /_  / _  / / /  __/",
    "/_/  /_/  \\___//_/ /_/ /_/\\____//_/    _\\__, /      \\____/  \\___//_/ /_/\\____//_/ /_/ /_/\\___/      /_____/  /_/ /_/_\\__, / /_/  /_/ /_/\\___/",
    "                                       /____/                                                                       /____/",
];

pub fn banner_lines(language: Language) -> Vec<Line<'static>> {
    let colors = [
        Color::LightCyan,
        Color::LightCyan,
        Color::Blue,
        Color::Blue,
        Color::LightMagenta,
        Color::LightMagenta,
    ];
    let mut lines = BANNER_LINES
        .iter()
        .enumerate()
        .map(|(index, line)| {
            Line::from(Span::styled(
                format!("{BANNER_LEFT_PAD}{line}"),
                Style::default().fg(colors[index % colors.len()]),
            ))
        })
        .collect::<Vec<_>>();
    lines.push(Line::from(""));
    let banner_width = BANNER_LINES
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default();
    let subtitle = tr(language, TKey::Subtitle);
    let subtitle_offset =
        BANNER_LEFT_PAD.chars().count() + banner_width.saturating_sub(subtitle.chars().count()) / 2;
    lines.push(Line::from(Span::styled(
        format!("{}{}", " ".repeat(subtitle_offset), subtitle),
        theme::title(),
    )));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_is_readable_without_decoding_ascii_art() {
        let joined = BANNER_LINES.join("\n");
        assert!(joined.contains("______"));
        assert!(joined.contains("\\____/"));
        assert_eq!(BANNER_LINES.len(), 6);
    }

    #[test]
    fn banner_has_left_padding_and_centered_signature() {
        let lines = banner_lines(Language::En);
        assert!(lines[0].spans[0].content.starts_with(BANNER_LEFT_PAD));
        assert!(lines.last().unwrap().spans[0].content.contains("by ECD5A"));
    }
}
