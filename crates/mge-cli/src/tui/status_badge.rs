use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::i18n::{tr, Language, TKey};
use crate::tui::theme;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadgeKind {
    Ok,
    Warn,
    Error,
}

pub fn badge_text(language: Language, kind: BadgeKind) -> String {
    match kind {
        BadgeKind::Ok => format!("[✓] {}", tr(language, TKey::Ok)),
        BadgeKind::Warn => format!("[!] {}", tr(language, TKey::Warn)),
        BadgeKind::Error => format!("[x] {}", tr(language, TKey::Error)),
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
        assert!(badge_text(Language::Ru, BadgeKind::Error).contains("ОШИБКА"));
        assert!(badge_text(Language::En, BadgeKind::Ok).contains("✓"));
    }
}
