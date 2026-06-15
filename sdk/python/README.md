# Memory Genome Engine Python SDK

This package is a thin local wrapper around the Rust `mge` CLI. It does not implement storage, recall, indexing, sealing, or validation logic in Python.

JSON returned by the CLI is protocol/debug output only. Runtime storage remains binary.

## Local Use From The Repository

Run the example from the repository root:

```bash
python examples/python_basic_usage.py
```

For local scripts, import from the repository path:

```python
import sys
from pathlib import Path

repo = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(repo / "sdk" / "python"))

from mge_sdk import MemoryGenomeClient
```

Use the checked-out Rust CLI during development:

```python
client = MemoryGenomeClient(
    ".memory-genome",
    command=["cargo", "run", "-q", "-p", "mge-cli", "--bin", "mge", "--"],
    cwd=repo,
)
```

## Editable Install

No package has been published. For local development only:

```bash
python -m pip install -e sdk/python
```

## Smoke

```bash
python -c "import mge_sdk; print(mge_sdk.MemoryGenomeClient)"
python examples/python_basic_usage.py
python examples/python_agent_host.py
```

## Errors

- `MgeCommandError`: local CLI process failed.
- `MgeProtocolError`: structured JSON-RPC/MCP adapter error.

Use `result_or_raise_mcp_error(response)` when talking directly to `mge-mcp-server`.
