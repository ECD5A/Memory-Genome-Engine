use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::screens;

pub fn render(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(10), Constraint::Length(1)])
        .split(area);

    let lines = vec![
        Line::from(tr(app.language, TKey::HelpText)),
        Line::from(""),
        Line::from(tr(app.language, TKey::HelpWorkflow)),
        Line::from(tr(app.language, TKey::HelpLanguage)),
        Line::from(tr(app.language, TKey::HelpSafety)),
        Line::from(""),
        Line::from(format!(
            "- {} / {} / {}",
            tr(app.language, TKey::RecallMemory),
            tr(app.language, TKey::AddMemoryCell),
            tr(app.language, TKey::SealHotMemory)
        )),
        Line::from(format!(
            "- {} / {} / {}",
            tr(app.language, TKey::StoreStatus),
            tr(app.language, TKey::BenchmarkIndexes),
            tr(app.language, TKey::ExportImportMarkdown)
        )),
    ];

    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::Help)),
        layout[0],
    );
    screens::render_footer(frame, app, layout[1], TKey::FooterScreen);
}
