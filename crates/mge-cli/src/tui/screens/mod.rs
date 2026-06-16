pub mod add_memory;
pub mod benchmark;
pub mod dashboard;
pub mod export_import;
pub mod help;
pub mod recall;
pub mod seal;
pub mod settings;
pub mod setup;
pub mod status;

use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::status_badge::{badge_line, BadgeKind};
use crate::tui::theme;

pub fn block(title: impl Into<String>) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(theme::panel())
        .title(title.into())
}

pub fn selected_line(selected: bool, text: impl Into<String>) -> Line<'static> {
    let prefix = if selected { "› " } else { "  " };
    let style = if selected {
        theme::selected()
    } else {
        theme::normal()
    };
    Line::from(Span::styled(format!("{prefix}{}", text.into()), style))
}

pub fn key_value(label: &str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), theme::muted()),
        Span::raw(value.into()),
    ])
}

pub fn field_line(selected: bool, label: &str, value: &str) -> Line<'static> {
    selected_line(selected, format!("{label}: {value}"))
}

pub fn action_line(selected: bool, label: &str) -> Line<'static> {
    selected_line(selected, format!("[ {label} ]"))
}

pub fn render_footer(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, key: TKey) {
    let footer = Paragraph::new(tr(app.language, key))
        .style(theme::muted())
        .alignment(Alignment::Center);
    frame.render_widget(footer, area);
}

pub fn render_status(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let line = if let Some(message) = &app.status_message {
        badge_line(app.language, message.kind, &message.text)
    } else if app.active_doctor().ok {
        badge_line(
            app.language,
            BadgeKind::Ok,
            tr(app.language, TKey::Unlocked),
        )
    } else {
        badge_line(
            app.language,
            BadgeKind::Warn,
            tr(app.language, TKey::Health),
        )
    };
    let paragraph = Paragraph::new(line).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = 1024.0 * 1024.0 * 1024.0;
    let value = bytes as f64;
    if value >= GB {
        format!("{:.2} GB", value / GB)
    } else if value >= MB {
        format!("{:.2} MB", value / MB)
    } else if value >= KB {
        format!("{:.2} KB", value / KB)
    } else {
        format!("{bytes} B")
    }
}

pub fn micros(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.2} s", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.2} ms", value as f64 / 1_000.0)
    } else {
        format!("{value} us")
    }
}

pub fn paragraph(lines: Vec<Line<'static>>, title: impl Into<String>) -> Paragraph<'static> {
    Paragraph::new(lines)
        .block(block(title))
        .wrap(Wrap { trim: false })
}

pub fn section(lines: Vec<Line<'static>>, title: impl Into<String>) -> Paragraph<'static> {
    let mut body = vec![Line::from(Span::styled(title.into(), theme::title()))];
    if !lines.is_empty() {
        body.push(Line::from(""));
        body.extend(lines);
    }
    Paragraph::new(body).wrap(Wrap { trim: false })
}
