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
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let actions = vec![
        action_line(
            app.form_selected == 0,
            tr(app.language, TKey::ExportMarkdown),
        ),
        action_line(
            app.form_selected == 1,
            tr(app.language, TKey::ImportMarkdown),
        ),
        Line::from(Span::styled(
            tr(app.language, TKey::ImportUnavailable),
            theme::muted(),
        )),
        Line::from(Span::styled(
            tr(app.language, TKey::MarkdownPlaintextWarning),
            theme::warning(),
        )),
    ];
    frame.render_widget(
        screens::paragraph(actions, tr(app.language, TKey::ExportImportMarkdown)),
        layout[0],
    );

    let lines = vec![
        key_value(
            tr(app.language, TKey::ExportPath),
            app.export_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| tr(app.language, TKey::NoData).to_string()),
        ),
        key_value(
            tr(app.language, TKey::ImportMarkdown),
            tr(app.language, TKey::ReadOnlyImportNote),
        ),
    ];
    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::OperationStatus)),
        layout[1],
    );
    screens::render_status(frame, app, layout[2]);
    screens::render_footer(frame, app, layout[3], TKey::FooterScreen);
}
