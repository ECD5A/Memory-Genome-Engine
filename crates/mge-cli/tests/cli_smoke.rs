use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::tempdir;

#[test]
fn cli_milestone_flow_outputs_context_stats_and_validation_json() {
    let dir = tempdir().unwrap();
    let store = dir.path().join(".memory-genome");

    run_mge(&store, &["init"]);
    run_mge(
        &store,
        &[
            "remember",
            "User prefers concise technical explanations",
            "--kind",
            "user_preference",
            "--scope",
            "global",
            "--trust",
            "user_confirmed",
        ],
    );

    let recall = run_mge_json(
        &store,
        &[
            "recall",
            "How should the agent answer technical questions?",
            "--json",
        ],
    );
    assert_eq!(recall["relevant_memory"].as_array().unwrap().len(), 1);
    assert!(recall["relevant_memory"][0]["content"]
        .as_str()
        .unwrap()
        .contains("concise technical"));

    run_mge(&store, &["seal"]);

    let stats = run_mge_json(&store, &["stats", "--json"]);
    assert_eq!(stats["hot_cells"], 0);
    assert_eq!(stats["sealed_pages"], 1);
    assert_eq!(stats["sealed_cells"], 1);
    assert_eq!(stats["current_index_kind"], "exact_marker_page");

    let validation = run_mge_json(&store, &["validate", "--json"]);
    assert_eq!(validation["ok"], true);
    assert_eq!(validation["errors"].as_array().unwrap().len(), 0);
}

#[test]
fn cli_fast_profile_initializes_compact_storage_defaults() {
    let dir = tempdir().unwrap();
    let store = dir.path().join(".memory-genome");

    run_mge(&store, &["init", "--profile", "fast"]);

    let stats = run_mge_json(&store, &["stats", "--json"]);
    assert_eq!(stats["current_page_codec"], "message_pack");
    assert_eq!(stats["current_compression"], "zstd");
    assert_eq!(stats["current_index_kind"], "exact_marker_page");
    assert_eq!(stats["current_page_clusterer"], "scope_kind");
}

fn run_mge(store: &Path, args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_mge"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "mge {:?} failed\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    output
}

fn run_mge_json(store: &Path, args: &[&str]) -> Value {
    let output = run_mge(store, args);
    serde_json::from_slice(&output.stdout).unwrap()
}
