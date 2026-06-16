use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::screens::{self, action_line, field_line, key_value};
use crate::tui::theme;

pub fn render(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Min(9),
            Constraint::Length(7),
        ])
        .split(area);

    render_form(frame, app, layout[0]);
    render_results(frame, app, layout[1]);
    render_diagnostics(frame, app, layout[2]);
}

fn render_form(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let input = &app.recall_input;
    let lines = vec![
        field_line(
            app.form_selected == 0,
            tr(app.language, TKey::Query),
            &input.query,
        ),
        field_line(
            app.form_selected == 1,
            tr(app.language, TKey::RecallMode),
            &input.mode.to_string(),
        ),
        field_line(
            app.form_selected == 2,
            tr(app.language, TKey::ResultLimit),
            &input.max_items.to_string(),
        ),
        field_line(
            app.form_selected == 3,
            tr(app.language, TKey::Markers),
            &input.markers,
        ),
        field_line(
            app.form_selected == 4,
            tr(app.language, TKey::Scope),
            &input.scope,
        ),
        field_line(
            app.form_selected == 5,
            tr(app.language, TKey::Kind),
            &input.kind,
        ),
        action_line(app.form_selected == 6, tr(app.language, TKey::RunRecall)),
    ];
    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::RecallMemory)),
        area,
    );
}

fn render_results(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let mut lines = Vec::new();
    if let Some(packet) = &app.recall_result {
        if packet.relevant_memory.is_empty() {
            lines.push(Line::from(Span::styled(
                tr(app.language, TKey::EmptyResults),
                theme::muted(),
            )));
        } else {
            for (index, item) in packet.relevant_memory.iter().enumerate() {
                lines.push(Line::from(Span::styled(
                    format!(
                        "{}. {} [kind={}, trust={}, status={}, scope={}]",
                        index + 1,
                        item.content,
                        item.kind,
                        item.trust,
                        item.status,
                        item.scope
                    ),
                    theme::normal(),
                )));
                if !item.markers.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!("   markers: {}", item.markers.join(", ")),
                        theme::muted(),
                    )));
                }
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            tr(app.language, TKey::EmptyResults),
            theme::muted(),
        )));
    }
    frame.render_widget(
        screens::section(lines, tr(app.language, TKey::Results)),
        area,
    );
}

fn render_diagnostics(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let Some(packet) = &app.recall_result else {
        frame.render_widget(
            screens::section(
                vec![Line::from(tr(app.language, TKey::NoData))],
                tr(app.language, TKey::Diagnostics),
            ),
            area,
        );
        return;
    };
    let debug = &packet.debug;
    let lines = vec![
        key_value(
            tr(app.language, TKey::RecallLatency),
            screens::micros(debug.total_recall_micros),
        ),
        key_value(
            tr(app.language, TKey::CandidatePages),
            debug.candidate_pages_returned.to_string(),
        ),
        key_value(
            tr(app.language, TKey::LoadedPages),
            debug.loaded_pages.to_string(),
        ),
        key_value(
            tr(app.language, TKey::ScannedCells),
            debug.cells_scanned.to_string(),
        ),
        key_value(
            tr(app.language, TKey::ResultCount),
            packet.relevant_memory.len().to_string(),
        ),
    ];
    frame.render_widget(
        screens::section(lines, tr(app.language, TKey::Diagnostics)),
        area,
    );
}
