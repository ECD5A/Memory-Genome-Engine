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
    store_path = Path(tempfile.mkdtemp(prefix="mge-python-example-")) / ".memory-genome"
    client = MemoryGenomeClient(
        store_path,
        command=mge_command(),
        cwd=ROOT,
    )

    client.init(profile="fast")
    cell_id = client.remember(
        "Agent should recall ContextPacket memory before editing the project.",
        kind="procedure",
        scope="agent_demo",
        markers=["topic:agent_integration"],
        trust="user_confirmed",
        sensitivity="private",
    )

    hot_packet = client.recall(
        "agent integration context packet",
        mode="focused",
        scope="agent_demo",
        max_items=3,
    )
    assert hot_packet["relevant_memory"]

    checkpoint = client.checkpoint()
    assert checkpoint["hot_cells"] == 1

    seal = client.seal()
    assert seal["hot_cells_sealed"] == 1

    sealed_packet = client.recall(
        "agent integration context packet",
        mode="broad",
        scope="agent_demo",
        max_items=5,
    )
    assert sealed_packet["relevant_memory"]

    validation = client.validate(deep=True)
    assert validation["ok"] is True

    rebuild = client.rebuild_indexes()
    assert rebuild["pages_unchanged"] is True

    markdown_path = client.export_markdown()
    assert markdown_path.is_file()

    print(
        f"python sdk example ok: cell={cell_id}, "
        f"items={len(sealed_packet['relevant_memory'])}, store={store_path}"
    )


if __name__ == "__main__":
    main()
