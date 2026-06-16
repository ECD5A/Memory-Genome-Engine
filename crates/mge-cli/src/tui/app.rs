use std::io::{self, Stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use mge_core::{ContextPacket, IndexKind, RecallMode, SealReport};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use crate::app_service::{
    AppService, DashboardSummary, DoctorReport, IndexBenchmarkReport, RecallInput, RememberInput,
};
use crate::tui::banner;
use crate::tui::i18n::{tr, Language, TKey};
use crate::tui::input;
use crate::tui::screens;
use crate::tui::status_badge::BadgeKind;
use crate::tui::theme;

#[derive(Clone, Debug)]
pub struct TuiOptions {
    pub store: PathBuf,
    pub passphrase_env: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Setup,
    Dashboard,
    Recall,
    AddMemory,
    Seal,
    Status,
    Benchmark,
    ExportImport,
    Settings,
    Help,
}

#[derive(Clone, Debug)]
pub struct TuiSettings {
    pub human_dashboard: bool,
    pub timing_diagnostics: bool,
    pub debug_json_output: bool,
    pub markdown_export: bool,
    pub experimental_features: bool,
    pub default_recall_mode: RecallMode,
    pub index_kind: IndexKind,
}

impl Default for TuiSettings {
    fn default() -> Self {
        Self {
            human_dashboard: true,
            timing_diagnostics: true,
            debug_json_output: false,
            markdown_export: true,
            experimental_features: false,
            default_recall_mode: RecallMode::Focused,
            index_kind: IndexKind::ExactMarkerPage,
        }
    }
}

#[derive(Clone, Debug)]
pub struct StatusMessage {
    pub kind: BadgeKind,
    pub text: String,
}

pub struct TuiApp {
    pub service: AppService,
    pub language: Language,
    pub screen: Screen,
    pub previous_screen: Screen,
    navigation_stack: Vec<Screen>,
    pub dashboard: DashboardSummary,
    pub dashboard_selected: usize,
    pub form_selected: usize,
    pub settings_selected: usize,
    pub recall_input: RecallInput,
    pub recall_result: Option<ContextPacket>,
    pub remember_input: RememberInput,
    pub seal_confirm: bool,
    pub seal_result: Option<SealReport>,
    pub benchmark_report: Option<IndexBenchmarkReport>,
    pub export_path: Option<PathBuf>,
    pub settings: TuiSettings,
    pub status_message: Option<StatusMessage>,
}

impl TuiApp {
    pub fn new(options: TuiOptions) -> Self {
        let service = AppService::new(options.store, options.passphrase_env);
        let dashboard = service.dashboard();
        let initial_screen = if dashboard.doctor.initialized {
            Screen::Dashboard
        } else {
            Screen::Setup
        };
        let mut settings = TuiSettings::default();
        if let Some(stats) = &dashboard.stats {
            settings.index_kind = stats.current_index_kind;
        }
        Self {
            service,
            language: Language::En,
            screen: initial_screen,
            previous_screen: Screen::Dashboard,
            navigation_stack: Vec::new(),
            dashboard,
            dashboard_selected: 0,
            form_selected: 0,
            settings_selected: 0,
            recall_input: RecallInput::default(),
            recall_result: None,
            remember_input: RememberInput::default(),
            seal_confirm: false,
            seal_result: None,
            benchmark_report: None,
            export_path: None,
            settings,
            status_message: None,
        }
    }

    pub fn refresh_dashboard(&mut self) {
        self.dashboard = self.service.dashboard();
        if let Some(stats) = &self.dashboard.stats {
            self.settings.index_kind = stats.current_index_kind;
        }
    }

    pub fn set_status(&mut self, kind: BadgeKind, text: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            kind,
            text: text.into(),
        });
    }

    pub fn active_doctor(&self) -> &DoctorReport {
        &self.dashboard.doctor
    }

    pub fn open_screen(&mut self, screen: Screen) {
        if self.screen != screen {
            self.navigation_stack.push(self.screen);
            if self.navigation_stack.len() > 16 {
                self.navigation_stack.remove(0);
            }
        }
        self.previous_screen = self.screen;
        self.screen = screen;
        self.form_selected = 0;
        self.seal_confirm = false;
    }

    fn go_back(&mut self) {
        if let Some(previous) = self.navigation_stack.pop() {
            self.previous_screen = self.screen;
            self.screen = previous;
        } else if self.screen != Screen::Dashboard {
            self.previous_screen = self.screen;
            self.screen = Screen::Dashboard;
        } else {
            return;
        }
        self.form_selected = 0;
        self.seal_confirm = false;
    }
}

pub fn run(options: TuiOptions) -> Result<()> {
    let mut terminal = enter_terminal()?;
    let mut app = TuiApp::new(options);
    let result = run_loop(&mut terminal, &mut app);
    leave_terminal(&mut terminal)?;
    result
}

fn enter_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn leave_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, app: &mut TuiApp) -> Result<()> {
    loop {
        terminal.draw(|frame| draw(frame, app))?;
        if event::poll(Duration::from_millis(250))? {
            if let Event::Key(key) = event::read()? {
                if !handle_key(app, key)? {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn draw(frame: &mut Frame<'_>, app: &TuiApp) {
    let area = frame.area();
    if area.width < 80 || area.height < 24 {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("Memory Genome Engine");
        let text = Paragraph::new(tr(app.language, TKey::TooSmall))
            .block(block)
            .style(theme::warning());
        frame.render_widget(text, area);
        return;
    }

    if app.screen == Screen::Help {
        screens::help::render(frame, app, area);
        return;
    }

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

    let header = Paragraph::new(banner::banner_lines(app.language)).wrap(Wrap { trim: false });
    frame.render_widget(header, layout[0]);
    draw_screen_content(frame, app, layout[1]);
    screens::render_status(frame, app, layout[2]);
    screens::render_footer(frame, app, layout[3], footer_key(app.screen));
}

fn draw_screen_content(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    match app.screen {
        Screen::Setup => screens::setup::render(frame, app, area),
        Screen::Dashboard => screens::dashboard::render(frame, app, area),
        Screen::Recall => screens::recall::render(frame, app, area),
        Screen::AddMemory => screens::add_memory::render(frame, app, area),
        Screen::Seal => screens::seal::render(frame, app, area),
        Screen::Status => screens::status::render(frame, app, area),
        Screen::Benchmark => screens::benchmark::render(frame, app, area),
        Screen::ExportImport => screens::export_import::render(frame, app, area),
        Screen::Settings => screens::settings::render(frame, app, area),
        Screen::Help => screens::help::render(frame, app, area),
    }
}

fn footer_key(screen: Screen) -> TKey {
    match screen {
        Screen::Dashboard => TKey::FooterDashboard,
        Screen::Settings => TKey::FooterSettings,
        _ => TKey::FooterScreen,
    }
}

fn handle_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    if key.kind != KeyEventKind::Press {
        return Ok(true);
    }

    if input::is_language_key(key) {
        app.language = app.language.toggle();
        return Ok(true);
    }
    if matches!(key.code, KeyCode::F(2)) {
        app.open_screen(Screen::Help);
        return Ok(true);
    }
    if matches!(key.code, KeyCode::Esc) {
        if app.screen == Screen::Dashboard {
            return Ok(true);
        }
        app.go_back();
        return Ok(true);
    }

    match app.screen {
        Screen::Setup => handle_setup_key(app, key),
        Screen::Dashboard => handle_dashboard_key(app, key),
        Screen::Recall => handle_recall_key(app, key),
        Screen::AddMemory => handle_add_memory_key(app, key),
        Screen::Seal => handle_seal_key(app, key),
        Screen::Status => handle_status_key(app, key),
        Screen::Benchmark => handle_benchmark_key(app, key),
        Screen::ExportImport => handle_export_key(app, key),
        Screen::Settings => handle_settings_key(app, key),
        Screen::Help => handle_help_key(app, key),
    }
}

fn handle_setup_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    const ROWS: usize = 5;
    match key.code {
        KeyCode::Up => input::move_up(&mut app.form_selected, ROWS),
        KeyCode::Down => input::move_down(&mut app.form_selected, ROWS),
        KeyCode::Enter => match app.form_selected {
            0 => run_setup(app, false),
            1 => run_setup(app, true),
            2 => run_deep_doctor(app),
            3 => app.open_screen(Screen::Dashboard),
            4 => app.open_screen(Screen::Help),
            _ => {}
        },
        KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(false),
        _ => {}
    }
    Ok(true)
}

fn run_setup(app: &mut TuiApp, encrypted: bool) {
    match app.service.setup_fast(encrypted) {
        Ok(report) => {
            let message = if report.already_initialized {
                tr(app.language, TKey::SetupAlreadyInitialized)
            } else {
                tr(app.language, TKey::SetupReady)
            };
            app.set_status(BadgeKind::Ok, message);
            app.refresh_dashboard();
            if app.dashboard.doctor.initialized {
                app.open_screen(Screen::Dashboard);
            }
        }
        Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
    }
}

fn handle_dashboard_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    const MENU_LEN: usize = 9;
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(false),
        KeyCode::Up => input::move_up(&mut app.dashboard_selected, MENU_LEN),
        KeyCode::Down => input::move_down(&mut app.dashboard_selected, MENU_LEN),
        KeyCode::Enter => match app.dashboard_selected {
            0 => app.open_screen(Screen::Recall),
            1 => app.open_screen(Screen::AddMemory),
            2 => app.open_screen(Screen::Seal),
            3 => app.open_screen(Screen::Status),
            4 => app.open_screen(Screen::Benchmark),
            5 => app.open_screen(Screen::ExportImport),
            6 => app.open_screen(Screen::Settings),
            7 => app.open_screen(Screen::Help),
            8 => return Ok(false),
            _ => {}
        },
        _ => {}
    }
    Ok(true)
}

fn handle_recall_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    const ROWS: usize = 7;
    match key.code {
        KeyCode::Up => input::move_up(&mut app.form_selected, ROWS),
        KeyCode::Down => input::move_down(&mut app.form_selected, ROWS),
        KeyCode::Left => adjust_recall_field(app, false),
        KeyCode::Right => adjust_recall_field(app, true),
        KeyCode::Enter if app.form_selected == 6 => run_recall(app),
        _ => {
            let changed = match app.form_selected {
                0 => input::edit_text(&mut app.recall_input.query, key),
                3 => input::edit_text(&mut app.recall_input.markers, key),
                4 => input::edit_text(&mut app.recall_input.scope, key),
                5 => input::edit_text(&mut app.recall_input.kind, key),
                _ => false,
            };
            if changed {
                app.recall_result = None;
            }
        }
    }
    Ok(true)
}

fn adjust_recall_field(app: &mut TuiApp, forward: bool) {
    match app.form_selected {
        1 => {
            app.recall_input.mode = cycle_recall_mode(app.recall_input.mode, forward);
        }
        2 if forward => app.recall_input.max_items = app.recall_input.max_items.saturating_add(1),
        2 => app.recall_input.max_items = app.recall_input.max_items.saturating_sub(1).max(1),
        _ => {}
    }
}

fn run_recall(app: &mut TuiApp) {
    match app.service.recall(app.recall_input.clone()) {
        Ok(packet) => {
            let count = packet.relevant_memory.len();
            app.recall_result = Some(packet);
            app.set_status(
                BadgeKind::Ok,
                format!("{}: {count}", tr(app.language, TKey::Results)),
            );
        }
        Err(err) => {
            app.recall_result = None;
            app.set_status(BadgeKind::Error, err.to_string());
        }
    }
}

fn handle_add_memory_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    const ROWS: usize = 9;
    match key.code {
        KeyCode::Up => input::move_up(&mut app.form_selected, ROWS),
        KeyCode::Down => input::move_down(&mut app.form_selected, ROWS),
        KeyCode::Enter if app.form_selected == 8 => save_memory(app),
        _ => {
            match app.form_selected {
                0 => input::edit_text(&mut app.remember_input.subject, key),
                1 => input::edit_text(&mut app.remember_input.content, key),
                2 => input::edit_text(&mut app.remember_input.markers, key),
                3 => input::edit_text(&mut app.remember_input.scope, key),
                4 => input::edit_text(&mut app.remember_input.kind, key),
                5 => input::edit_text(&mut app.remember_input.status, key),
                6 => input::edit_text(&mut app.remember_input.trust, key),
                7 => input::edit_text(&mut app.remember_input.sensitivity, key),
                _ => false,
            };
        }
    }
    Ok(true)
}

fn save_memory(app: &mut TuiApp) {
    match app.service.remember(app.remember_input.clone()) {
        Ok(cell_id) => {
            app.set_status(
                BadgeKind::Ok,
                format!("{}: cell {cell_id}", tr(app.language, TKey::Saved)),
            );
            app.remember_input.subject.clear();
            app.remember_input.content.clear();
            app.remember_input.markers.clear();
            app.refresh_dashboard();
        }
        Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
    }
}

fn handle_seal_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    const ROWS: usize = 2;
    match key.code {
        KeyCode::Up => input::move_up(&mut app.form_selected, ROWS),
        KeyCode::Down => input::move_down(&mut app.form_selected, ROWS),
        KeyCode::Enter if app.form_selected == 0 => run_checkpoint(app),
        KeyCode::Enter if app.form_selected == 1 => {
            if app.seal_confirm {
                run_seal(app);
                app.seal_confirm = false;
            } else {
                app.seal_confirm = true;
                app.set_status(BadgeKind::Warn, tr(app.language, TKey::ConfirmSeal));
            }
        }
        _ => {}
    }
    Ok(true)
}

fn run_checkpoint(app: &mut TuiApp) {
    match app.service.checkpoint() {
        Ok(report) => {
            app.set_status(
                BadgeKind::Ok,
                format!(
                    "{}: {} hot cells -> {}",
                    tr(app.language, TKey::Checkpoint),
                    report.hot_cells,
                    report.snapshot_path.display()
                ),
            );
            app.refresh_dashboard();
        }
        Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
    }
}

fn run_seal(app: &mut TuiApp) {
    match app.service.seal() {
        Ok(report) => {
            app.set_status(
                BadgeKind::Ok,
                format!(
                    "{}: {} hot cell(s), {} page(s)",
                    tr(app.language, TKey::SealResult),
                    report.hot_cells_sealed,
                    report.pages_written
                ),
            );
            app.seal_result = Some(report);
            app.refresh_dashboard();
        }
        Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
    }
}

fn handle_status_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    const ROWS: usize = 4;
    match key.code {
        KeyCode::Up => input::move_up(&mut app.form_selected, ROWS),
        KeyCode::Down => input::move_down(&mut app.form_selected, ROWS),
        KeyCode::Enter => match app.form_selected {
            0 => {
                app.refresh_dashboard();
                app.set_status(BadgeKind::Ok, tr(app.language, TKey::Refresh));
            }
            1 => run_deep_doctor(app),
            2 => run_validate_deep(app),
            3 => run_rebuild_indexes(app),
            _ => {}
        },
        _ => {}
    }
    Ok(true)
}

fn run_deep_doctor(app: &mut TuiApp) {
    match app.service.doctor(true) {
        Ok(report) => {
            app.dashboard.doctor = report;
            app.set_status(BadgeKind::Ok, tr(app.language, TKey::DeepDoctor));
        }
        Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
    }
}

fn run_validate_deep(app: &mut TuiApp) {
    match app.service.validate_deep() {
        Ok(report) if report.ok => {
            app.set_status(BadgeKind::Ok, tr(app.language, TKey::ValidateDeep))
        }
        Ok(report) => app.set_status(
            BadgeKind::Error,
            format!(
                "{}: {} {}",
                tr(app.language, TKey::ValidateDeep),
                report.errors.len(),
                tr(app.language, TKey::Error)
            ),
        ),
        Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
    }
}

fn run_rebuild_indexes(app: &mut TuiApp) {
    match app.service.rebuild_indexes() {
        Ok(report) => app.set_status(
            BadgeKind::Ok,
            format!(
                "{}: {} page(s)",
                tr(app.language, TKey::RebuildIndexes),
                report.pages_scanned
            ),
        ),
        Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
    }
}

fn handle_benchmark_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    match key.code {
        KeyCode::Enter | KeyCode::Char(' ') => match app.service.run_small_index_benchmark() {
            Ok(report) => {
                app.benchmark_report = Some(report);
                app.set_status(BadgeKind::Ok, tr(app.language, TKey::BenchmarkResult));
            }
            Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
        },
        _ => {}
    }
    Ok(true)
}

fn handle_export_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    const ROWS: usize = 2;
    match key.code {
        KeyCode::Up => input::move_up(&mut app.form_selected, ROWS),
        KeyCode::Down => input::move_down(&mut app.form_selected, ROWS),
        KeyCode::Enter if app.form_selected == 0 => match app.service.export_markdown() {
            Ok(path) => {
                app.export_path = Some(path.clone());
                app.set_status(
                    BadgeKind::Ok,
                    format!(
                        "{}: {}",
                        tr(app.language, TKey::ExportMarkdown),
                        path.display()
                    ),
                );
            }
            Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
        },
        KeyCode::Enter => {
            app.set_status(BadgeKind::Warn, tr(app.language, TKey::ReadOnlyImportNote))
        }
        _ => {}
    }
    Ok(true)
}

fn handle_settings_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    const ROWS: usize = 9;
    match key.code {
        KeyCode::Up => input::move_up(&mut app.settings_selected, ROWS),
        KeyCode::Down => input::move_down(&mut app.settings_selected, ROWS),
        KeyCode::Left => change_setting(app, false),
        KeyCode::Right => change_setting(app, true),
        KeyCode::Char(' ') => toggle_setting(app),
        KeyCode::Enter if app.settings_selected == 8 => apply_index_kind(app),
        KeyCode::Enter => toggle_setting(app),
        _ => {}
    }
    Ok(true)
}

fn change_setting(app: &mut TuiApp, forward: bool) {
    match app.settings_selected {
        0 => app.language = app.language.toggle(),
        1 => {
            app.settings.default_recall_mode =
                cycle_recall_mode(app.settings.default_recall_mode, forward);
            app.recall_input.mode = app.settings.default_recall_mode;
        }
        2 => {
            app.settings.index_kind = match app.settings.index_kind {
                IndexKind::ExactMarkerPage => IndexKind::BinaryFusePage,
                IndexKind::BinaryFusePage => IndexKind::ExactMarkerPage,
            };
        }
        _ => toggle_setting(app),
    }
}

fn toggle_setting(app: &mut TuiApp) {
    match app.settings_selected {
        0 => app.language = app.language.toggle(),
        3 => app.settings.human_dashboard = !app.settings.human_dashboard,
        4 => app.settings.timing_diagnostics = !app.settings.timing_diagnostics,
        5 => app.settings.debug_json_output = !app.settings.debug_json_output,
        6 => app.settings.markdown_export = !app.settings.markdown_export,
        7 => app.settings.experimental_features = !app.settings.experimental_features,
        8 => apply_index_kind(app),
        _ => {}
    }
}

fn apply_index_kind(app: &mut TuiApp) {
    match app.service.set_index_kind(app.settings.index_kind) {
        Ok(report) => {
            app.set_status(
                BadgeKind::Ok,
                format!(
                    "{}: {}",
                    tr(app.language, TKey::ApplyIndexKind),
                    report.current.index_kind
                ),
            );
            app.refresh_dashboard();
        }
        Err(err) => app.set_status(BadgeKind::Error, err.to_string()),
    }
}

fn handle_help_key(app: &mut TuiApp, key: KeyEvent) -> Result<bool> {
    if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
        app.go_back();
    }
    Ok(true)
}

fn cycle_recall_mode(mode: RecallMode, forward: bool) -> RecallMode {
    match (mode, forward) {
        (RecallMode::Focused, true) => RecallMode::Broad,
        (RecallMode::Broad, true) => RecallMode::FullScope,
        (RecallMode::FullScope, true) => RecallMode::Focused,
        (RecallMode::Focused, false) => RecallMode::FullScope,
        (RecallMode::Broad, false) => RecallMode::Focused,
        (RecallMode::FullScope, false) => RecallMode::Broad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventState, KeyModifiers};

    #[test]
    fn recall_mode_cycles_both_directions() {
        assert_eq!(
            cycle_recall_mode(RecallMode::Focused, true),
            RecallMode::Broad
        );
        assert_eq!(
            cycle_recall_mode(RecallMode::Focused, false),
            RecallMode::FullScope
        );
    }

    #[test]
    fn settings_default_to_human_safe_values() {
        let settings = TuiSettings::default();
        assert!(settings.human_dashboard);
        assert!(settings.markdown_export);
        assert!(!settings.debug_json_output);
    }

    #[test]
    fn new_tui_opens_setup_for_missing_store() {
        let dir = tempfile::tempdir().unwrap();
        let app = TuiApp::new(TuiOptions {
            store: dir.path().join(".memory-genome"),
            passphrase_env: None,
        });

        assert_eq!(app.screen, Screen::Setup);
    }

    #[test]
    fn new_tui_opens_dashboard_for_initialized_store() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join(".memory-genome");
        AppService::new(&store, None).setup_fast(false).unwrap();

        let app = TuiApp::new(TuiOptions {
            store,
            passphrase_env: None,
        });

        assert_eq!(app.screen, Screen::Dashboard);
    }

    #[test]
    fn repeated_key_events_are_ignored_to_prevent_menu_skips() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = TuiApp::new(TuiOptions {
            store: dir.path().join(".memory-genome"),
            passphrase_env: None,
        });
        let selected = app.form_selected;
        let language = app.language;

        handle_key(
            &mut app,
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Repeat,
                state: KeyEventState::NONE,
            },
        )
        .unwrap();
        handle_key(
            &mut app,
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Repeat,
                state: KeyEventState::NONE,
            },
        )
        .unwrap();

        assert_eq!(app.form_selected, selected);
        assert_eq!(app.language, language);
    }

    #[test]
    fn escape_walks_back_through_tui_navigation() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join(".memory-genome");
        AppService::new(&store, None).setup_fast(false).unwrap();
        let mut app = TuiApp::new(TuiOptions {
            store,
            passphrase_env: None,
        });

        handle_key(
            &mut app,
            KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        )
        .unwrap();
        assert_eq!(app.screen, Screen::Recall);

        handle_key(
            &mut app,
            KeyEvent {
                code: KeyCode::F(2),
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        )
        .unwrap();
        assert_eq!(app.screen, Screen::Help);

        handle_key(
            &mut app,
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        )
        .unwrap();
        assert_eq!(app.screen, Screen::Recall);

        handle_key(
            &mut app,
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        )
        .unwrap();
        assert_eq!(app.screen, Screen::Dashboard);

        assert!(handle_key(
            &mut app,
            KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
        )
        .unwrap());
        assert_eq!(app.screen, Screen::Dashboard);
    }
}
