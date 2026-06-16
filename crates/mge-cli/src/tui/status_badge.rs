use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::i18n::Language;
use crate::tui::theme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadgeKind {
    Ok,
    Warn,
    Error,
}

pub fn badge_text(_language: Language, kind: BadgeKind) -> String {
    match kind {
        BadgeKind::Ok => "[OK]".to_string(),
        BadgeKind::Warn => "[WARN]".to_string(),
        BadgeKind::Error => "[ERR]".to_string(),
    }
}

pub fn badge_style(kind: BadgeKind) -> Style {
    match kind {
        BadgeKind::Ok => theme::success(),
        BadgeKind::Warn => theme::warning(),
        BadgeKind::Error => theme::error(),
    }
}

pub fn badge_line(language: Language, kind: BadgeKind, message: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(badge_text(language, kind), badge_style(kind)),
        Span::raw(format!(" {message}")),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_badges_include_text_not_only_color() {
        assert!(badge_text(Language::En, BadgeKind::Ok).contains("OK"));
        assert!(badge_text(Language::Ru, BadgeKind::Error).contains("ERR"));
        assert!(badge_text(Language::En, BadgeKind::Ok).is_ascii());
    }
}
