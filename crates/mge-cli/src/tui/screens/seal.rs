use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::screens::{self, action_line, key_value};
use crate::tui::theme;

pub fn render(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(10), Constraint::Min(7)])
        .split(area);

    let stats = app.dashboard.stats.as_ref();
    let mut lines = Vec::new();
    if let Some(stats) = stats {
        lines.push(key_value(
            tr(app.language, TKey::HotCells),
            stats.hot_cells.to_string(),
        ));
        lines.push(key_value(
            tr(app.language, TKey::IndexKind),
            stats.current_index_kind.to_string(),
        ));
        lines.push(key_value(
            tr(app.language, TKey::SealedPages),
            stats.sealed_pages.to_string(),
        ));
    }
    lines.push(action_line(
        app.form_selected == 0,
        tr(app.language, TKey::Checkpoint),
    ));
    lines.push(action_line(
        app.form_selected == 1,
        tr(app.language, TKey::SealAction),
    ));
    if app.seal_confirm {
        lines.push(Line::from(Span::styled(
            tr(app.language, TKey::ConfirmSeal),
            theme::warning(),
        )));
    }
    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::SealHotMemory)),
        layout[0],
    );

    render_result(frame, app, layout[1]);
}

fn render_result(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let lines = if let Some(report) = &app.seal_result {
        vec![
            key_value(
                tr(app.language, TKey::HotCells),
                report.hot_cells_sealed.to_string(),
            ),
            key_value(
                tr(app.language, TKey::CreatedPages),
                report.pages_written.to_string(),
            ),
            key_value(
                tr(app.language, TKey::ArchivedHotLog),
                report
                    .archived_hot_log
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| tr(app.language, TKey::NoData).to_string()),
            ),
        ]
    } else {
        vec![Line::from(tr(app.language, TKey::NoData))]
    };
    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::SealResult)),
        area,
    );
}
