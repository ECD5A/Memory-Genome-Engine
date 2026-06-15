from __future__ import annotations

import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "sdk" / "python"))

from mge_sdk import MemoryGenomeClient  # noqa: E402


def main() -> None:
    store_path = Path(tempfile.mkdtemp(prefix="mge-python-agent-host-")) / ".memory-genome"
    client = MemoryGenomeClient(
        store_path,
        command=["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"],
        cwd=ROOT,
    )

    client.init(profile="fast")

    task = "prepare local agent host integration smoke"
    focused_packet = client.recall(
        task,
        mode="focused",
        scope="mandate_2",
        max_items=5,
    )
    assert focused_packet["debug"]["recall_mode"] == "focused"

    # Fake local work. No external LLM/API call is made here.
    work_result = "Python agent host completed a fake integration task using ContextPacket memory."
    cell_id = client.remember(
        work_result,
        kind="tool_result",
        scope="mandate_2",
        markers=["topic:agent_host", "lang:python"],
        trust="tool_observed",
        sensitivity="private",
    )

    checkpoint = client.checkpoint()
    assert checkpoint["hot_cells"] == 1

    broad_packet = client.recall(
        "agent host integration task",
        mode="broad",
        scope="mandate_2",
        max_items=10,
    )
    assert any(item["content"] == work_result for item in broad_packet["relevant_memory"])

    seal = client.seal()
    assert seal["hot_cells_sealed"] == 1

    sealed_packet = client.recall(
        "agent host integration task",
        mode="focused",
        scope="mandate_2",
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
