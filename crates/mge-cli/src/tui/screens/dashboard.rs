use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem};
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::screens::{self, key_value};
use crate::tui::theme;

pub fn render(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);

    render_menu(frame, app, body[0]);
    render_summary(frame, app, body[1]);
}

fn render_summary(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let doctor = app.active_doctor();
    let stats = app.dashboard.stats.as_ref();
    let health = if doctor.ok {
        tr(app.language, TKey::Ok)
    } else {
        tr(app.language, TKey::Warn)
    };
    let mut lines = vec![
        key_value(
            tr(app.language, TKey::StorePath),
            app.service.store_path().display().to_string(),
        ),
        key_value(tr(app.language, TKey::Health), health),
        key_value(
            tr(app.language, TKey::ActiveLanguage),
            app.language.code().to_string(),
        ),
        key_value(
            tr(app.language, TKey::DefaultRecallMode),
            app.settings.default_recall_mode.to_string(),
        ),
    ];

    if let Some(stats) = stats {
        lines.push(key_value(
            tr(app.language, TKey::HotCells),
            stats.hot_cells.to_string(),
        ));
        lines.push(key_value(
            tr(app.language, TKey::SealedPages),
            stats.sealed_pages.to_string(),
        ));
        lines.push(key_value(
            tr(app.language, TKey::SealedCells),
            stats.sealed_cells.to_string(),
        ));
        lines.push(key_value(
            tr(app.language, TKey::MarkerCount),
            stats.marker_count.to_string(),
        ));
        lines.push(key_value(
            tr(app.language, TKey::IndexKind),
            stats.current_index_kind.to_string(),
        ));
        lines.push(key_value(
            tr(app.language, TKey::StorageSize),
            screens::format_bytes(stats.store_size_bytes),
        ));
    } else {
        lines.push(Line::from(Span::styled(
            tr(app.language, TKey::NotInitialized),
            theme::warning(),
        )));
    }

    if !doctor.warnings.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}:", tr(app.language, TKey::Warn)),
            theme::warning(),
        )));
        for warning in doctor.warnings.iter().take(3) {
            lines.push(Line::from(format!("- {warning}")));
        }
    }
    if !doctor.issues.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}:", tr(app.language, TKey::Error)),
            theme::error(),
        )));
        for issue in doctor.issues.iter().take(3) {
            lines.push(Line::from(format!("- {issue}")));
        }
    }

    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::Dashboard)),
        area,
    );
}

fn render_menu(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let labels = [
        TKey::RecallMemory,
        TKey::AddMemoryCell,
        TKey::SealHotMemory,
        TKey::StoreStatus,
        TKey::BenchmarkIndexes,
        TKey::ExportImportMarkdown,
        TKey::Settings,
        TKey::Help,
        TKey::Exit,
    ];
    let items = labels
        .iter()
        .enumerate()
        .map(|(index, key)| {
            let selected = index == app.dashboard_selected;
            let prefix = if selected { "› " } else { "  " };
            let style = if selected {
                theme::selected()
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(
                format!("{prefix}{}", tr(app.language, *key)),
                style,
            )))
        })
        .collect::<Vec<_>>();

    let menu = List::new(items).block(screens::block(tr(app.language, TKey::Dashboard)));
    frame.render_widget(menu, area);
}
