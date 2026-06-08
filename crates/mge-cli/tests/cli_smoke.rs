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
    assert_eq!(recall["debug"]["recall_mode"], "focused");
    assert!(recall["relevant_memory"][0]["content"]
        .as_str()
        .unwrap()
        .contains("concise technical"));

    run_mge(&store, &["seal"]);

    let stats = run_mge_json(&store, &["stats", "--json"]);
    assert_eq!(stats["hot_cells"], 0);
    assert_eq!(stats["sealed_pages"], 1);
    assert_eq!(stats["sealed_cells"], 1);
    assert_eq!(stats["current_page_codec"], "message_pack");
    assert_eq!(stats["current_index_kind"], "exact_marker_page");

    let validation = run_mge_json(&store, &["validate", "--json"]);
    assert_eq!(validation["ok"], true);
    assert_eq!(validation["errors"].as_array().unwrap().len(), 0);

    run_mge(&store, &["export"]);
    assert!(store.join("manifest.mgm").is_file());
    assert!(store.join("dictionary").join("markers.mgd").is_file());
    assert!(store.join("hot").join("hot.mgl").is_file());
    assert!(store.join("indexes").join("page_index.mgi").is_file());
    assert!(store.join("indexes").join("marker_index.mgi").is_file());
    assert!(store.join("indexes").join("fuse_index.mgi").is_file());
    assert!(store.join("exports").join("memory.md").is_file());

    assert!(!store.join("manifest.json").exists());
    assert!(!store.join("markers.json").exists());
    assert!(!store.join("hot").join("hot_cells.jsonl").exists());
}

#[test]
fn cli_recall_modes_support_broad_and_full_scope() {
    let dir = tempdir().unwrap();
    let store = dir.path().join(".memory-genome");

    run_mge(&store, &["init"]);
    for index in 0..7 {
        run_mge(
            &store,
            &[
                "remember",
                &format!("alpha module task memory {index}"),
                "--kind",
                "project_fact",
                "--scope",
                "alpha-module",
                "--trust",
                "tool_observed",
                "--marker",
                "tag:alpha",
            ],
        );
    }

    let broad = run_mge_json(
        &store,
        &["recall", "alpha module task", "--mode", "broad", "--json"],
    );
    assert_eq!(broad["debug"]["recall_mode"], "broad");
    assert_eq!(broad["debug"]["max_items"], 20);
    assert_eq!(broad["relevant_memory"].as_array().unwrap().len(), 7);

    run_mge(&store, &["seal"]);
    let full_scope = run_mge_json(
        &store,
        &[
            "recall",
            "--mode",
            "full-scope",
            "--scope",
            "alpha-module",
            "--json",
        ],
    );
    assert_eq!(full_scope["debug"]["recall_mode"], "full_scope");
    assert_eq!(full_scope["debug"]["full_scope_used"], true);
    assert_eq!(full_scope["relevant_memory"].as_array().unwrap().len(), 7);

    let failed = run_mge_failure(&store, &["recall", "--mode", "full-scope"]);
    assert!(String::from_utf8_lossy(&failed.stderr).contains("full-scope recall requires"));
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

fn run_mge_failure(store: &Path, args: &[&str]) -> Output {
    let output = Command::new(env!("CARGO_BIN_EXE_mge"))
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "mge {:?} unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    output
}
