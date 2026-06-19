use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::screens::{self, action_line, field_line};

pub fn render(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(14)])
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
        action_line(
            app.settings_selected == 3,
            tr(app.language, TKey::ApplyIndexKind),
        ),
        Line::from(""),
        Line::from(format!(
            "{}: {}",
            tr(app.language, TKey::StorePath),
            app.service.store_path().display()
        )),
        Line::from(tr(app.language, TKey::SettingsPersistenceNote)),
    ];
    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::Settings)),
        layout[0],
    );
}
