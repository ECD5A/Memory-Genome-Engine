use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::i18n::{tr, Language, TKey};

const BANNER_LEFT_PAD: &str = "    ";
pub const BANNER_RENDER_HEIGHT: u16 = 9;

pub const BANNER_LINES: &[&str] = &[
    "______  ___                                         _________                                       __________              _____",
    "___   |/  /___________ _____________________  __    __  ____/________________________ ________      ___  ____/_____________ ___(_)___________",
    "__  /|_/ /_  _ \\_  __ `__ \\  __ \\_  ___/_  / / /    _  / __ _  _ \\_  __ \\  __ \\_  __ `__ \\  _ \\     __  __/  __  __ \\_  __ `/_  /__  __ \\  _ \\",
    "_  /  / / /  __/  / / / / / /_/ /  /   _  /_/ /     / /_/ / /  __/  / / / /_/ /  / / / / /  __/     _  /___  _  / / /  /_/ /_  / _  / / /  __/",
    "/_/  /_/  \\___//_/ /_/ /_/\\____//_/    _\\__, /      \\____/  \\___//_/ /_/\\____//_/ /_/ /_/\\___/      /_____/  /_/ /_/_\\__, / /_/  /_/ /_/\\___/",
    "                                       /____/                                                                       /____/",
];

pub fn banner_lines(language: Language) -> Vec<Line<'static>> {
    let banner_width = BANNER_LINES
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default();
    let mut lines = BANNER_LINES
        .iter()
        .map(|line| Line::from(rainbow_banner_spans(line, banner_width)))
        .collect::<Vec<_>>();
    lines.push(Line::from(""));
    lines.push(Line::from(subtitle_spans(language, banner_width)));
    lines.push(Line::from(""));
    lines
}

fn subtitle_spans(language: Language, banner_width: usize) -> Vec<Span<'static>> {
    let raw = tr(language, TKey::Subtitle);
    let main = raw
        .strip_suffix(" by ECD5A")
        .unwrap_or(raw)
        .to_ascii_uppercase();
    let separator = "  ::  ";
    let brand = "BY ECD5A";
    let subtitle_width = main.chars().count() + separator.chars().count() + brand.chars().count();
    let subtitle_offset =
        BANNER_LEFT_PAD.chars().count() + banner_width.saturating_sub(subtitle_width) / 2;

    vec![
        Span::raw(" ".repeat(subtitle_offset)),
        Span::styled(
            main,
            Style::default()
                .fg(Color::Rgb(112, 241, 255))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(separator, Style::default().fg(Color::DarkGray)),
        Span::styled(
            brand,
            Style::default()
                .fg(Color::Rgb(255, 216, 92))
                .add_modifier(Modifier::BOLD),
        ),
    ]
}

fn rainbow_banner_spans(line: &str, banner_width: usize) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw(BANNER_LEFT_PAD.to_string())];
    spans.extend(line.chars().enumerate().map(|(column, ch)| {
        Span::styled(
            ch.to_string(),
            Style::default().fg(rainbow_color(column, banner_width + 24)),
        )
    }));
    spans
}

fn rainbow_color(position: usize, width: usize) -> Color {
    const STOPS: &[(u8, u8, u8)] = &[
        (97, 255, 255),
        (0, 178, 255),
        (60, 96, 255),
        (143, 70, 255),
        (255, 48, 235),
        (255, 65, 155),
        (255, 190, 72),
        (94, 255, 135),
    ];

    let width = width.max(2);
    let scaled = position as f32 / (width - 1) as f32 * (STOPS.len() - 1) as f32;
    let left = scaled.floor() as usize;
    let right = (left + 1).min(STOPS.len() - 1);
    let t = scaled - left as f32;
    let (lr, lg, lb) = STOPS[left];
    let (rr, rg, rb) = STOPS[right];
    Color::Rgb(
        lerp_channel(lr, rr, t),
        lerp_channel(lg, rg, t),
        lerp_channel(lb, rb, t),
    )
}

fn lerp_channel(left: u8, right: u8, t: f32) -> u8 {
    (left as f32 + (right as f32 - left as f32) * t).round() as u8
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
        assert!(lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .any(|span| span.content.contains("BY ECD5A")));
        assert_eq!(lines.len(), BANNER_RENDER_HEIGHT as usize);
    }

    #[test]
    fn subtitle_uses_product_tagline_styling() {
        let spans = subtitle_spans(Language::En, 142);
        assert!(spans
            .iter()
            .any(|span| span.content.contains("LOCAL-FIRST MEMORY ENGINE")));
        let brand = spans
            .iter()
            .find(|span| span.content.contains("ECD5A"))
            .unwrap();
        assert!(matches!(brand.style.fg, Some(Color::Rgb(255, 216, 92))));
    }

    #[test]
    fn banner_uses_rgb_gradient_colors() {
        let lines = banner_lines(Language::En);
        let first_colored = lines[0]
            .spans
            .iter()
            .find(|span| !span.content.trim().is_empty())
            .unwrap();
        assert!(matches!(first_colored.style.fg, Some(Color::Rgb(_, _, _))));
    }

    #[test]
    fn banner_bottom_uses_same_horizontal_gradient_as_top() {
        let lines = banner_lines(Language::En);
        let top_color = lines[0].spans[5].style.fg;
        let bottom_color = lines[5].spans[5].style.fg;
        assert_eq!(top_color, bottom_color);
    }
}
