use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::Line;
use ratatui::Frame;

use crate::tui::app::TuiApp;
use crate::tui::i18n::{tr, TKey};
use crate::tui::screens::{self, action_line, key_value};

pub fn render(frame: &mut Frame<'_>, app: &TuiApp, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(10)])
        .split(area);

    frame.render_widget(
        screens::paragraph(
            vec![action_line(true, tr(app.language, TKey::RunBenchmark))],
            tr(app.language, TKey::BenchmarkIndexes),
        ),
        layout[0],
    );

    let mut lines = Vec::new();
    if let Some(report) = &app.benchmark_report {
        lines.push(key_value(
            format!("{} {}", tr(app.language, TKey::ExactMarkerPage), "latency").as_str(),
            screens::micros(report.exact_recall_micros),
        ));
        lines.push(key_value(
            format!("{} {}", tr(app.language, TKey::BinaryFusePage), "latency").as_str(),
            screens::micros(report.binary_fuse_recall_micros),
        ));
        lines.push(Line::from(""));
        lines.push(key_value(
            format!("{} exact", tr(app.language, TKey::CandidatePages)).as_str(),
            report.exact_candidate_pages.to_string(),
        ));
        lines.push(key_value(
            format!("{} binary_fuse", tr(app.language, TKey::CandidatePages)).as_str(),
            report.binary_fuse_candidate_pages.to_string(),
        ));
        lines.push(key_value(
            tr(app.language, TKey::LoadedPages),
            format!(
                "exact={} binary_fuse={}",
                report.exact_loaded_pages, report.binary_fuse_loaded_pages
            ),
        ));
        lines.push(key_value(
            tr(app.language, TKey::ScannedCells),
            format!(
                "exact={} binary_fuse={}",
                report.exact_cells_scanned, report.binary_fuse_cells_scanned
            ),
        ));
        lines.push(key_value(
            tr(app.language, TKey::ResultCount),
            format!(
                "exact={} binary_fuse={}",
                report.exact_result_count, report.binary_fuse_result_count
            ),
        ));
        lines.push(key_value(
            tr(app.language, TKey::FalsePositivePages),
            report.false_positive_pages.to_string(),
        ));
        lines.push(key_value(
            "exact candidates covered by binary_fuse",
            report.exact_subset_binary_fuse.to_string(),
        ));
    } else {
        lines.push(Line::from(tr(app.language, TKey::NoData)));
    }

    frame.render_widget(
        screens::paragraph(lines, tr(app.language, TKey::BenchmarkResult)),
        layout[1],
    );
}
