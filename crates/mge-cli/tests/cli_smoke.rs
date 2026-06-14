use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};
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

    fs::remove_file(store.join("indexes").join("marker_index.mgi")).unwrap();
    let failed_validation = run_mge_failure(&store, &["validate", "--deep", "--json"]);
    let failed_report: Value = serde_json::from_slice(&failed_validation.stdout).unwrap();
    assert_eq!(failed_report["ok"], false);
    assert!(failed_report["errors"]
        .as_array()
        .unwrap()
        .iter()
        .any(|error| error
            .as_str()
            .unwrap()
            .contains("active candidate index file missing")));

    let rebuild = run_mge_json(&store, &["rebuild-indexes", "--json"]);
    assert_eq!(rebuild["index_kind"], "exact_marker_page");
    assert_eq!(rebuild["pages_scanned"], 1);
    assert_eq!(rebuild["exact_index_written"], true);
    assert_eq!(rebuild["binary_fuse_index_written"], false);
    assert_eq!(rebuild["pages_unchanged"], true);

    let validation = run_mge_json(&store, &["validate", "--deep", "--json"]);
    assert_eq!(validation["ok"], true);
    let recall_after_rebuild = run_mge_json(
        &store,
        &[
            "recall",
            "How should the agent answer technical questions?",
            "--json",
        ],
    );
    assert_eq!(
        recall_after_rebuild["relevant_memory"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

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
    assert_eq!(stats["current_durability"], "balanced");
}

#[test]
fn cli_checkpoint_and_durability_config_restore_hot_memory() {
    let dir = tempdir().unwrap();
    let store = dir.path().join(".memory-genome");

    run_mge(&store, &["init"]);
    run_mge(&store, &["config", "set", "durability", "safe"]);
    let stats = run_mge_json(&store, &["stats", "--json"]);
    assert_eq!(stats["current_durability"], "safe");

    run_mge(
        &store,
        &[
            "remember",
            "checkpoint durability smoke memory",
            "--kind",
            "project_fact",
            "--scope",
            "checkpoint-smoke",
            "--trust",
            "tool_observed",
            "--marker",
            "tag:checkpoint_smoke",
        ],
    );
    let checkpoint = run_mge_json(&store, &["checkpoint", "--json"]);
    assert_eq!(checkpoint["hot_cells"], 1);
    assert_eq!(checkpoint["durability"], "safe");
    assert!(store.join("hot").join("snapshot.mgs").is_file());

    let recalled = run_mge_json(
        &store,
        &[
            "recall",
            "checkpoint durability",
            "--marker",
            "tag:checkpoint_smoke",
            "--json",
        ],
    );
    assert_eq!(recalled["relevant_memory"].as_array().unwrap().len(), 1);
    assert_eq!(recalled["debug"]["hot_total_cells"], 1);

    run_mge(&store, &["seal"]);
    assert!(!store.join("hot").join("snapshot.mgs").exists());
    let stats = run_mge_json(&store, &["stats", "--json"]);
    assert_eq!(stats["hot_cells"], 0);
    assert_eq!(stats["sealed_cells"], 1);
}

#[test]
fn mcp_server_json_rpc_adapter_supports_agent_workflow() {
    let dir = tempdir().unwrap();
    let store = dir.path().join(".memory-genome");
    let export_path = dir.path().join("agent-memory.md");
    let store_path = store.to_string_lossy().to_string();
    let export_path_string = export_path.to_string_lossy().to_string();

    run_mge(&store, &["init", "--profile", "fast"]);

    let responses = run_mcp_json_lines(&[
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "mge_remember",
            "params": {
                "store_path": store_path.clone(),
                "content": "Agent should recall project memory before making changes",
                "kind": "procedure",
                "scope": "mandate_2",
                "markers": ["topic:agent_integration"],
                "trust": "user_confirmed",
                "sensitivity": "private"
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "mge_recall",
            "params": {
                "store_path": store_path.clone(),
                "query": "agent integration memory",
                "mode": "focused",
                "scope": "mandate_2",
                "max_items": 3
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "mge_checkpoint",
            "params": { "store_path": store_path.clone() }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "mge_seal",
            "params": { "store_path": store_path.clone() }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "mge_stats",
            "params": { "store_path": store_path.clone() }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "mge_validate",
            "params": { "store_path": store_path.clone(), "deep": true }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "mge_rebuild_indexes",
            "params": { "store_path": store_path.clone() }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "mge_export_markdown",
            "params": {
                "store_path": store_path.clone(),
                "output_path": export_path_string.clone()
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "mge_recall",
            "params": {
                "store_path": store_path.clone(),
                "mode": "full_scope"
            }
        }),
    ]);

    assert_eq!(responses.len(), 9);
    assert_eq!(responses[0]["result"]["ok"], true);
    assert_eq!(responses[0]["result"]["cell_id"], 1);
    assert_eq!(responses[0]["result"]["json_runtime_storage"], false);
    assert_eq!(
        responses[1]["result"]["context_packet"]["debug"]["recall_mode"],
        "focused"
    );
    assert_eq!(
        responses[1]["result"]["context_packet"]["relevant_memory"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(responses[3]["result"]["seal"]["hot_cells_sealed"], 1);
    assert_eq!(responses[4]["result"]["stats"]["hot_cells"], 0);
    assert_eq!(responses[4]["result"]["stats"]["sealed_cells"], 1);
    assert_eq!(responses[5]["result"]["validation"]["ok"], true);
    assert_eq!(responses[6]["result"]["rebuild"]["pages_unchanged"], true);
    assert_eq!(responses[7]["result"]["format"], "markdown");
    assert!(export_path.is_file());
    assert!(responses[8]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("full_scope recall requires scope"));

    assert!(!store.join("manifest.json").exists());
    assert!(!store.join("hot").join("hot_cells.jsonl").exists());
}

#[test]
fn synthetic_benchmark_outputs_valid_core_metrics() {
    let dir = tempdir().unwrap();
    let store_root = dir.path().join("bench");
    let output = Command::new(env!("CARGO_BIN_EXE_mge-synthetic-bench"))
        .args([
            "--cells",
            "24",
            "--pages",
            "6",
            "--scopes",
            "3",
            "--markers-per-cell",
            "4",
            "--marker-groups",
            "4",
            "--targeted-queries",
            "3",
            "--noise-queries",
            "1",
            "--repeats",
            "2",
            "--seed",
            "42",
            "--store-root",
        ])
        .arg(&store_root)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "mge-synthetic-bench failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["synthetic_config"]["cells"], 24);
    assert_eq!(
        report["subset_check"]["focused_exact_candidates_subset_of_binary_fuse_candidates"],
        true
    );
    let modes = report["modes"].as_array().unwrap();
    assert_eq!(modes.len(), 2);
    assert_eq!(modes[0]["index_kind"], "exact_marker_page");
    assert_eq!(modes[1]["index_kind"], "binary_fuse_page");
    for mode in modes {
        assert_eq!(mode["total_sealed_pages"], 6);
        assert_eq!(mode["total_cells"], 24);
        assert!(mode["storage_size_bytes"].as_u64().unwrap() > 0);
        assert_eq!(mode["seal_correctness"]["post_seal_hot_cells"], 0);
        assert_eq!(mode["seal_correctness"]["hot_cleared_after_seal"], true);
        assert_eq!(mode["build"]["remember_latency_micros"]["count"], 24);
        assert_eq!(
            mode["hot_recall_modes"]["focused"]["latency_micros"]["count"],
            8
        );
        assert_eq!(
            mode["hot_recall_modes"]["focused"]["hot_total_cells"]["avg"],
            24
        );
        assert!(
            mode["hot_recall_modes"]["focused"]["hot_candidate_cells"]["avg"]
                .as_u64()
                .unwrap()
                <= 24
        );
        assert_eq!(
            mode["recall_modes"]["focused"]["latency_micros"]["count"],
            8
        );
        assert_eq!(mode["recall_modes"]["focused"]["hot_total_cells"]["avg"], 0);
        assert_eq!(
            mode["recall_modes"]["focused"]["timing_breakdown_micros"]["total_recall"]["count"],
            8
        );
        assert_eq!(
            mode["recall_modes"]["focused"]["timing_breakdown_micros"]["page_decode"]["count"],
            8
        );
        assert_eq!(
            mode["recall_modes"]["focused"]["timing_breakdown_micros"]["scoring_cache_build"]
                ["count"],
            8
        );
        assert!(
            mode["recall_modes"]["focused"]["decoded_page_cache_hits"]["count"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            mode["recall_modes"]["focused"]["decoded_page_cache_misses"]["count"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            mode["recall_modes"]["focused"]["scoring_cache_misses"]["count"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            mode["recall_modes"]["focused"]["pages_considered"]["count"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert!(
            mode["recall_modes"]["focused"]["cells_ranked"]["count"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_eq!(mode["recall_modes"]["broad"]["latency_micros"]["count"], 8);
        assert_eq!(
            mode["recall_modes"]["full_scope"]["latency_micros"]["count"],
            8
        );
        assert!(
            mode["index_lookup"]["latency_micros"]["count"]
                .as_u64()
                .unwrap()
                > 0
        );
        assert_eq!(mode["page_decode"]["pages_decoded"], 12);
        assert_eq!(mode["context_packet_build"]["latency_micros"]["count"], 2);
    }
}

#[test]
fn corpus_benchmark_outputs_valid_core_metrics() {
    let dir = tempdir().unwrap();
    let corpus = dir.path().join("corpus");
    fs::create_dir_all(corpus.join("src")).unwrap();
    fs::create_dir_all(corpus.join("docs")).unwrap();
    fs::write(
        corpus.join("src").join("lib.rs"),
        "pub fn alpha_engine() { let marker = \"alpha corpus memory\"; }\n\
         pub fn beta_engine() { let marker = \"beta corpus memory\"; }\n",
    )
    .unwrap();
    fs::write(
        corpus.join("docs").join("notes.md"),
        "# Corpus Notes\n\nAlpha module keeps compact page memory.\nBeta module checks recall timing.\n",
    )
    .unwrap();
    fs::write(
        corpus.join("Cargo.toml"),
        "[package]\nname = \"corpus-smoke\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    fs::write(corpus.join("ignored.bin"), [0, 159, 146, 150]).unwrap();
    let outside = dir.path().join("outside.md");
    fs::write(
        &outside,
        "# Outside\n\nThis file must not be followed through a symlink.\n",
    )
    .unwrap();
    let symlink_created = create_file_symlink(&outside, &corpus.join("outside-link.md")).is_ok();

    let store_root = dir.path().join("corpus-bench-store");
    let output = Command::new(env!("CARGO_BIN_EXE_mge-corpus-bench"))
        .args([
            "--corpus",
            corpus.to_str().unwrap(),
            "--store-root",
            store_root.to_str().unwrap(),
            "--profile",
            "small",
            "--max-files",
            "8",
            "--max-bytes",
            "20000",
            "--max-file-bytes",
            "10000",
            "--chunk-bytes",
            "256",
            "--chunk-lines",
            "2",
            "--targeted-queries",
            "2",
            "--noise-queries",
            "1",
            "--repeats",
            "2",
            "--seed",
            "7",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "mge-corpus-bench failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let chunks = report["corpus"]["chunks_created"].as_u64().unwrap();
    assert_eq!(report["corpus_config"]["profile"], "small");
    assert_eq!(report["corpus_config"]["generated"], false);
    assert_eq!(report["corpus_config"]["chunk_lines"], 2);
    assert_eq!(report["corpus_config"]["seed"], 7);
    assert_eq!(report["generated_corpus"]["enabled"], false);
    assert!(report["corpus"]["files_imported"].as_u64().unwrap() >= 3);
    assert!(chunks >= 3);
    assert!(
        report["corpus"]["skipped"]["unsupported_extensions"]
            .as_u64()
            .unwrap()
            >= 1
    );
    if symlink_created {
        assert!(
            report["corpus"]["skipped"]["symlinks"].as_u64().unwrap() >= 1,
            "symlink should be skipped instead of followed"
        );
    }
    assert_eq!(
        report["subset_check"]["focused_exact_candidates_subset_of_binary_fuse_candidates"],
        true
    );
    assert_eq!(
        report["comparison"]["sealed_cold_avg_micros"]["focused"]["exact_marker_page"]
            .as_u64()
            .is_some(),
        true
    );
    assert_eq!(
        report["comparison"]["sealed_cold_avg_micros"]["broad"]["binary_fuse_page"]
            .as_u64()
            .is_some(),
        true
    );
    assert_eq!(
        report["comparison"]["sealed_repeated_avg_micros"]["full_scope"]["exact_marker_page"]
            .as_u64()
            .is_some(),
        true
    );
    assert_eq!(
        report["comparison"]["sealed_repeated_timing_avg_micros"]["focused"]["page_decode"]
            ["exact_marker_page"]
            .as_u64()
            .is_some(),
        true
    );
    assert_eq!(
        report["comparison"]["sealed_repeated_timing_avg_micros"]["focused"]["scoring_cache_build"]
            ["binary_fuse_page"]
            .as_u64()
            .is_some(),
        true
    );
    assert_eq!(
        report["comparison"]["sealed_repeated_timing_avg_micros"]["focused"]
            ["context_packet_build"]["exact_marker_page"]
            .as_u64()
            .is_some(),
        true
    );
    assert!(
        report["comparison"]["page_shape"]["avg_encoded_page_bytes"]["exact_marker_page"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        report["comparison"]["sealed_repeated_locality"]["focused"]["decoded_page_cache_hits"]
            ["exact_marker_page"]
            .as_u64()
            .is_some()
    );
    assert!(
        report["comparison"]["sealed_repeated_locality"]["focused"]["scoring_cache_misses"]
            ["binary_fuse_page"]
            .as_u64()
            .is_some()
    );
    assert!(report["comparison"]["sealed_repeated_locality"]["focused"]
        ["sealed_cells_skipped_before_token_scoring"]["exact_marker_page"]
        .as_u64()
        .is_some());
    assert!(report["comparison"]["sealed_repeated_locality"]["focused"]
        ["sealed_cells_token_scored"]["binary_fuse_page"]
        .as_u64()
        .is_some());
    assert!(
        report["comparison"]["top_bottlenecks_avg_micros"]["exact_marker_page"]
            ["sealed_repeated_focused"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["component"].as_str().is_some()
                && entry["avg_micros"].as_u64().is_some())
    );
    assert!(report["recommendation"]["main_bottleneck"]
        .as_str()
        .is_some());
    assert!(report["recommendation"]["signals"]["binary_fuse_helped"]
        .as_bool()
        .is_some());
    assert!(
        report["recommendation"]["shares_percent"]["sealed_repeated_focused_exact"]["page_decode"]
            .as_u64()
            .is_some()
    );
    assert!(report["recommendation"]["human_summary"]
        .as_array()
        .unwrap()
        .iter()
        .any(|line| line
            .as_str()
            .unwrap_or("")
            .contains("Suggested next core step")));

    let modes = report["modes"].as_array().unwrap();
    assert_eq!(modes.len(), 2);
    assert_eq!(modes[0]["index_kind"], "exact_marker_page");
    assert_eq!(modes[1]["index_kind"], "binary_fuse_page");
    for mode in modes {
        assert_eq!(mode["total_cells"].as_u64().unwrap(), chunks);
        assert!(mode["total_sealed_pages"].as_u64().unwrap() > 0);
        assert!(mode["avg_encoded_page_bytes"].as_u64().unwrap() > 0);
        assert_eq!(mode["validation"]["validate_deep_ok"], true);
        assert_eq!(mode["validation"]["rebuild_indexes_ok"], true);
        assert_eq!(mode["validation"]["validate_after_rebuild_ok"], true);
        assert_eq!(mode["build"]["remember_latency_micros"]["count"], chunks);
        assert_eq!(
            mode["sealed_recall_modes"]["cold"]["focused"]["latency_micros"]["count"],
            6
        );
        assert_eq!(
            mode["sealed_recall_modes"]["repeated"]["focused"]["latency_micros"]["count"],
            6
        );
        assert_eq!(
            mode["sealed_recall_modes"]["repeated"]["focused"]["timing_breakdown_micros"]
                ["scoring_cache_build"]["count"],
            6
        );
        assert!(
            mode["sealed_recall_modes"]["repeated"]["focused"]["decoded_page_cache_hits"]["count"]
                .as_u64()
                .unwrap()
                > 0
        );
    }

    for kind in ["exact_marker_page", "binary_fuse_page"] {
        let mode_root = store_root.join(kind);
        assert!(mode_root.join("manifest.mgm").is_file());
        assert!(mode_root.join("dictionary").join("markers.mgd").is_file());
        assert!(mode_root.join("pages").is_dir());
        assert!(mode_root.join("indexes").join("page_index.mgi").is_file());
        assert!(!mode_root.join("manifest.json").exists());
        assert!(!mode_root.join("hot").join("hot_cells.jsonl").exists());
    }
}

#[cfg(unix)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn create_file_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[test]
fn corpus_benchmark_generated_small_profile_outputs_recommendation() {
    let dir = tempdir().unwrap();
    let store_root = dir.path().join("generated-small-store");
    let output = Command::new(env!("CARGO_BIN_EXE_mge-corpus-bench"))
        .args([
            "--generated",
            "--profile",
            "small",
            "--store-root",
            store_root.to_str().unwrap(),
            "--max-files",
            "12",
            "--max-bytes",
            "120000",
            "--max-file-bytes",
            "40000",
            "--targeted-queries",
            "3",
            "--noise-queries",
            "1",
            "--repeats",
            "1",
            "--seed",
            "3",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "mge-corpus-bench generated small failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["corpus_config"]["generated"], true);
    assert_eq!(report["corpus_config"]["profile"], "small");
    assert_eq!(report["generated_corpus"]["enabled"], true);
    assert!(
        report["generated_corpus"]["categories"]
            .as_array()
            .unwrap()
            .len()
            >= 5
    );
    assert!(report["corpus"]["extensions_count"].as_u64().unwrap() >= 4);
    assert!(
        report["corpus"]["skipped"]["unsupported_extensions"]
            .as_u64()
            .unwrap()
            >= 1
    );
    assert_eq!(
        report["subset_check"]["focused_exact_candidates_subset_of_binary_fuse_candidates"],
        true
    );
    assert!(report["recommendation"]["suggested_next_core_step"]
        .as_str()
        .is_some());
    assert!(store_root.join("generated-corpus").is_dir());
    assert!(store_root
        .join("exact_marker_page")
        .join("manifest.mgm")
        .is_file());
    assert!(!store_root
        .join("exact_marker_page")
        .join("manifest.json")
        .exists());
}

#[test]
fn corpus_benchmark_generated_medium_profile_accepts_overrides() {
    let dir = tempdir().unwrap();
    let store_root = dir.path().join("generated-medium-store");
    let output = Command::new(env!("CARGO_BIN_EXE_mge-corpus-bench"))
        .args([
            "--generated",
            "--profile",
            "medium",
            "--store-root",
            store_root.to_str().unwrap(),
            "--max-files",
            "10",
            "--max-bytes",
            "100000",
            "--max-file-bytes",
            "30000",
            "--chunk-lines",
            "4",
            "--targeted-queries",
            "2",
            "--noise-queries",
            "1",
            "--repeats",
            "1",
            "--seed",
            "11",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "mge-corpus-bench generated medium failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["corpus_config"]["profile"], "medium");
    assert_eq!(report["corpus_config"]["max_files"], 10);
    assert_eq!(report["corpus_config"]["chunk_lines"], 4);
    assert!(report["corpus"]["files_imported"].as_u64().unwrap() > 0);
    assert!(
        report["recommendation"]["signals"]["repeated_recall_locality_benefit_percent"]
            .as_i64()
            .is_some()
    );
}

#[test]
fn corpus_benchmark_rejects_store_root_inside_corpus_even_when_missing() {
    let dir = tempdir().unwrap();
    let corpus = dir.path().join("corpus");
    fs::create_dir_all(&corpus).unwrap();
    fs::write(corpus.join("notes.md"), "# Notes\n\nAlpha corpus memory.\n").unwrap();

    let nested_store = corpus.join("missing").join("store");
    let output = Command::new(env!("CARGO_BIN_EXE_mge-corpus-bench"))
        .args([
            "--corpus",
            corpus.to_str().unwrap(),
            "--store-root",
            nested_store.to_str().unwrap(),
            "--profile",
            "small",
            "--repeats",
            "1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("--store-root must be outside --corpus-root"));
    assert!(!nested_store.exists());
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

fn run_mcp_json_lines(requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_mge-mcp-server"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        for request in requests {
            writeln!(stdin, "{}", serde_json::to_string(request).unwrap()).unwrap();
        }
    }

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "mge-mcp-server failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
