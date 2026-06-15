use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::screens::{self, action_line, field_line, selected_line};
use crate::tui::widgets::toggle::toggle_text;

pub fn render(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(14),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let settings = &app.settings;
    let lines = vec![
        field_line(
            app.settings_selected == 0,
            tr(app.language, TKey::Language),
            app.language.code(),
        ),
        field_line(
            app.settings_selected == 1,
            tr(app.language, TKey::DefaultRecallMode),
            &settings.default_recall_mode.to_string(),
        ),
        field_line(
            app.settings_selected == 2,
            tr(app.language, TKey::IndexKind),
            &settings.index_kind.to_string(),
        ),
        selected_line(
            app.settings_selected == 3,
            toggle_text(
                app.language,
                settings.human_dashboard,
                tr(app.language, TKey::HumanDashboard),
            ),
        ),
        selected_line(
            app.settings_selected == 4,
            toggle_text(
                app.language,
                settings.timing_diagnostics,
                tr(app.language, TKey::TimingDiagnostics),
            ),
        ),
        selected_line(
            app.settings_selected == 5,
            toggle_text(
                app.language,
                settings.debug_json_output,
                tr(app.language, TKey::DebugJsonOutput),
            ),
        ),
        selected_line(
            app.settings_selected == 6,
            toggle_text(
                app.language,
                settings.markdown_export,
                tr(app.language, TKey::MarkdownExport),
            ),
        ),
        selected_line(
            app.settings_selected == 7,
            toggle_text(
                app.language,
                settings.experimental_features,
                tr(app.language, TKey::ExperimentalFeatures),
            ),
        ),
        action_line(
            app.settings_selected == 8,
            tr(app.language, TKey::ApplyIndexKind),
        ),
        Line::from(""),
        Line::from(format!(
            "{}: {}",
            tr(app.language, TKey::StorePath),
            app.service.store_path().display()
        )),
    ];
    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::Settings)),
        layout[0],
    );
    screens::render_status(frame, app, layout[1]);
    screens::render_footer(frame, app, layout[2], TKey::FooterSettings);
}
