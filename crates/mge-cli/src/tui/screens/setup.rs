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
            Constraint::Length(12),
            Constraint::Min(8),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    render_actions(frame, app, layout[0]);
    render_guidance(frame, app, layout[1]);
    screens::render_status(frame, app, layout[2]);
    screens::render_footer(frame, app, layout[3], TKey::FooterScreen);
}

fn render_actions(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let doctor = app.active_doctor();
    let mut lines = vec![
        key_value(
            tr(app.language, TKey::StorePath),
            app.service.store_path().display().to_string(),
        ),
        key_value(
            tr(app.language, TKey::Health),
            if doctor.initialized {
                tr(app.language, TKey::SetupAlreadyInitialized)
            } else {
                tr(app.language, TKey::NotInitialized)
            },
        ),
        key_value(
            tr(app.language, TKey::PassphraseEnv),
            app.service
                .passphrase_env_name()
                .unwrap_or("<not set>")
                .to_string(),
        ),
        Line::from(""),
        action_line(
            app.form_selected == 0,
            tr(app.language, TKey::InitFastStore),
        ),
        action_line(
            app.form_selected == 1,
            tr(app.language, TKey::InitEncryptedStore),
        ),
        action_line(app.form_selected == 2, tr(app.language, TKey::DeepDoctor)),
        action_line(app.form_selected == 3, tr(app.language, TKey::Dashboard)),
        action_line(app.form_selected == 4, tr(app.language, TKey::Help)),
    ];
    if doctor.initialized {
        lines.push(Line::from(Span::styled(
            tr(app.language, TKey::SetupAlreadyInitialized),
            theme::success(),
        )));
    }

    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::SetupStore)),
        area,
    );
}

fn render_guidance(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            tr(app.language, TKey::FirstRunHelp),
            theme::normal(),
        )),
        Line::from(""),
        Line::from(tr(app.language, TKey::EncryptedSetupHint)),
        Line::from(tr(app.language, TKey::MgeSetupCommand)),
        Line::from(tr(app.language, TKey::McpSdkGuidance)),
        Line::from(""),
        Line::from(Span::styled(
            tr(app.language, TKey::MarkdownPlaintextWarning),
            theme::warning(),
        )),
    ];
    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::Help)),
        area,
    );
}
