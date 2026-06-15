use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let store_root = std::env::temp_dir().join(format!("mge-agent-host-cli-{run_id}"));
    let store = store_root.join(".memory-genome");

    run_mge(&store, &["init", "--profile", "fast"]);

    let task = "prepare local agent host integration smoke";
    let focused = run_mge(
        &store,
        &[
            "recall",
            task,
            "--mode",
            "focused",
            "--scope",
            "mandate_2",
            "--json",
        ],
    );
    assert!(focused.contains("\"recall_mode\""));
    assert!(focused.contains("focused"));

    let fake_work_result =
        "Local agent host completed a fake integration task using a ContextPacket.";
    run_mge(
        &store,
        &[
            "remember",
            fake_work_result,
            "--kind",
            "tool_result",
            "--scope",
            "mandate_2",
            "--trust",
            "tool_observed",
            "--marker",
            "topic:agent_host",
        ],
    );

    let checkpoint = run_mge(&store, &["checkpoint", "--json"]);
    assert!(checkpoint.contains("\"hot_cells\""));
    assert!(checkpoint.contains('1'));

    let broad = run_mge(
        &store,
        &[
            "recall",
            "agent host integration task",
            "--mode",
            "broad",
            "--scope",
            "mandate_2",
            "--json",
        ],
    );
    assert!(broad.contains(fake_work_result));

    run_mge(&store, &["seal"]);

    let validate = run_mge(&store, &["validate", "--deep", "--json"]);
    assert!(validate.contains("\"ok\""));
    assert!(validate.contains("true"));

    println!(
        "agent host cli example ok: store={}",
        store_root.display()
    );
}

fn run_mge(store: &Path, args: &[&str]) -> String {
    let output = Command::new("cargo")
        .args(["run", "-q", "-p", "mge-cli", "--bin", "mge", "--"])
        .arg("--store")
        .arg(store)
        .args(args)
        .output()
        .expect("failed to run mge CLI");

    if !output.status.success() {
        panic!(
            "mge {:?} failed\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).expect("mge output was not UTF-8")
}
