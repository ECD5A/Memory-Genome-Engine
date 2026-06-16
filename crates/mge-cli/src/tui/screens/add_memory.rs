use ratatui::layout::Rect;
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::screens::{self, action_line, field_line};

pub fn render(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let input = &app.remember_input;
    let lines = vec![
        field_line(
            app.form_selected == 0,
            tr(app.language, TKey::TitleSummary),
            &input.subject,
        ),
        field_line(
            app.form_selected == 1,
            tr(app.language, TKey::Content),
            &input.content,
        ),
        field_line(
            app.form_selected == 2,
            tr(app.language, TKey::Markers),
            &input.markers,
        ),
        field_line(
            app.form_selected == 3,
            tr(app.language, TKey::Scope),
            &input.scope,
        ),
        field_line(
            app.form_selected == 4,
            tr(app.language, TKey::Kind),
            &input.kind,
        ),
        field_line(
            app.form_selected == 5,
            tr(app.language, TKey::Status),
            &input.status,
        ),
        field_line(
            app.form_selected == 6,
            tr(app.language, TKey::Trust),
            &input.trust,
        ),
        field_line(
            app.form_selected == 7,
            tr(app.language, TKey::Sensitivity),
            &input.sensitivity,
        ),
        action_line(app.form_selected == 8, tr(app.language, TKey::SaveMemory)),
    ];
    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::AddMemoryCell)),
        area,
    );
}
