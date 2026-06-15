use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::tui::i18n::{tr, Language, TKey};
use crate::tui::theme;

pub fn toggle_text(language: Language, enabled: bool, label: &str) -> String {
    let state = if enabled {
        tr(language, TKey::On)
    } else {
        tr(language, TKey::Off)
    };
    let symbol = if enabled { "●" } else { "○" };
    format!("[{symbol}] {state:<5} {label}")
}

pub fn toggle_style(enabled: bool) -> Style {
    if enabled {
        theme::success()
    } else {
        theme::muted()
    }
}

#[allow(dead_code)]
pub fn toggle_line(language: Language, enabled: bool, label: &str) -> Line<'static> {
    Line::from(Span::styled(
        toggle_text(language, enabled, label),
        toggle_style(enabled),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_formats_english_and_russian() {
        assert!(toggle_text(Language::En, true, "Timing").contains("ON"));
        assert!(toggle_text(Language::Ru, false, "Timing").contains("ВЫКЛ"));
        assert!(toggle_text(Language::En, true, "Timing").contains("●"));
        assert!(toggle_text(Language::En, false, "Timing").contains("○"));
    }
}
