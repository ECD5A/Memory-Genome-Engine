from __future__ import annotations

import sys
import tempfile
import os
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "sdk" / "python"))

from mge_sdk import MemoryGenomeClient  # noqa: E402


def mge_command() -> list[str]:
    if path := os.environ.get("MGE_BIN"):
        return [path]
    return ["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"]


def main() -> None:
    store_path = Path(tempfile.mkdtemp(prefix="mge-python-agent-host-")) / ".memory-genome"
    client = MemoryGenomeClient(
        store_path,
        command=mge_command(),
        cwd=ROOT,
    )

    client.init(profile="fast")

    session = client.remember_session(
        [
            {"role": "user", "content": "Prepare the local integration release"},
            {"role": "assistant", "content": "Use the existing agent host contract"},
            {"role": "user", "content": "Keep the verification result for recall"},
        ],
        session_id="python-agent-host",
        scope="agent_demo",
        markers=["topic:agent_host"],
        max_turns=2,
    )
    assert session["chunks"] == 2

    task = "prepare local agent host integration smoke"
    focused_packet = client.recall(
        task,
        mode="focused",
        scope="agent_demo",
        max_items=5,
    )
    assert focused_packet["debug"]["recall_mode"] == "focused"

    # Fake local work. No external LLM/API call is made here.
    work_result = "Python agent host completed a fake integration task using ContextPacket memory."
    cell_id = client.remember(
        work_result,
        kind="tool_result",
        scope="agent_demo",
        markers=["topic:agent_host", "lang:python"],
        trust="tool_observed",
        sensitivity="private",
    )

    checkpoint = client.checkpoint()
    assert checkpoint["hot_cells"] == 3

    broad_packet = client.recall(
        "agent host integration task",
        mode="broad",
        scope="agent_demo",
        max_items=10,
    )
    assert any(item["content"] == work_result for item in broad_packet["relevant_memory"])

    seal = client.seal()
    assert seal["hot_cells_sealed"] == 3

    sealed_packet = client.recall(
        "agent host integration task",
        mode="focused",
        scope="agent_demo",
        max_items=5,
    )
    assert any(item["content"] == work_result for item in sealed_packet["relevant_memory"])

    validation = client.validate(deep=True)
    assert validation["ok"] is True

    print(
        "python agent host example ok: "
        f"cell={cell_id}, sealed_items={len(sealed_packet['relevant_memory'])}, "
        f"store={store_path}"
    )


if __name__ == "__main__":
    main()
