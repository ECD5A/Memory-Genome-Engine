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
        .constraints([
            Constraint::Length(9),
            Constraint::Min(9),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let actions = vec![
        action_line(app.form_selected == 0, tr(app.language, TKey::Refresh)),
        action_line(app.form_selected == 1, tr(app.language, TKey::DeepDoctor)),
        action_line(app.form_selected == 2, tr(app.language, TKey::ValidateDeep)),
        action_line(
            app.form_selected == 3,
            tr(app.language, TKey::RebuildIndexes),
        ),
    ];
    frame.render_widget(
        screens::paragraph(actions, tr(app.language, TKey::StoreStatus)),
        layout[0],
    );

    render_report(frame, app, layout[1]);
    screens::render_status(frame, app, layout[2]);
    screens::render_footer(frame, app, layout[3], TKey::FooterScreen);
}

fn render_report(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let doctor = app.active_doctor();
    let mut lines = vec![
        key_value(tr(app.language, TKey::StorePath), doctor.store_path.clone()),
        key_value(
            tr(app.language, TKey::Health),
            if doctor.ok {
                tr(app.language, TKey::Ok)
            } else {
                tr(app.language, TKey::Warn)
            },
        ),
        key_value("manifest", doctor.manifest_readable.to_string()),
        key_value("unlock", doctor.unlock_status.clone()),
        key_value(
            tr(app.language, TKey::IndexKind),
            doctor
                .index_kind
                .clone()
                .unwrap_or_else(|| tr(app.language, TKey::NoData).to_string()),
        ),
        key_value(
            tr(app.language, TKey::SealedPages),
            doctor.page_files.to_string(),
        ),
    ];

    if let Some(validation) = &doctor.deep_validation {
        lines.push(Line::from(""));
        lines.push(key_value("validate deep", validation.ok.to_string()));
        lines.push(key_value(
            "checked pages",
            validation.checked_sealed_pages.to_string(),
        ));
        lines.push(key_value(
            "checked cells",
            validation.checked_sealed_cells.to_string(),
        ));
    }
    if !doctor.warnings.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}:", tr(app.language, TKey::Warn)),
            theme::warning(),
        )));
        for warning in &doctor.warnings {
            lines.push(Line::from(format!("- {warning}")));
        }
    }
    if !doctor.issues.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{}:", tr(app.language, TKey::Error)),
            theme::error(),
        )));
        for issue in &doctor.issues {
            lines.push(Line::from(format!("- {issue}")));
        }
    }

    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::Diagnostics)),
        area,
    );
}
