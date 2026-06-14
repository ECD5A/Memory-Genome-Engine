from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any, Iterable, Mapping, Sequence


class MgeCommandError(RuntimeError):
    def __init__(
        self,
        command: Sequence[str],
        returncode: int,
        stdout: str,
        stderr: str,
    ) -> None:
        super().__init__(
            f"Memory Genome command failed with exit code {returncode}: {' '.join(command)}\n{stderr}"
        )
        self.command = list(command)
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr


class MemoryGenomeClient:
    """Thin Python wrapper around the Rust `mge` CLI.

    The SDK does not implement storage or recall logic. It delegates to the Rust
    binary and uses JSON only as CLI protocol/debug output.
    """

    def __init__(
        self,
        store_path: str | Path,
        command: Sequence[str] | None = None,
        cwd: str | Path | None = None,
    ) -> None:
        self.store_path = Path(store_path)
        self.command = list(command) if command is not None else _default_command()
        self.cwd = Path(cwd) if cwd is not None else None

    def init(self, profile: str = "fast") -> str:
        return self._run_text(["init", "--profile", profile])

    def remember(
        self,
        content: str,
        *,
        kind: str = "temporary_note",
        scope: str = "global",
        markers: Iterable[str] = (),
        trust: str = "agent_inferred",
        sensitivity: str = "private",
        status: str = "active",
        subject: str | None = None,
    ) -> int:
        args = [
            "remember",
            content,
            "--kind",
            kind,
            "--scope",
            scope,
            "--trust",
            trust,
            "--sensitivity",
            sensitivity,
            "--status",
            status,
        ]
        if subject is not None:
            args.extend(["--subject", subject])
        for marker in markers:
            args.extend(["--marker", marker])

        output = self._run_text(args)
        match = re.search(r"Remembered cell (\d+)", output)
        if match is None:
            raise RuntimeError(f"could not parse remembered cell id from: {output!r}")
        return int(match.group(1))

    def recall(
        self,
        query: str = "",
        *,
        mode: str = "focused",
        scope: str | None = None,
        markers: Iterable[str] = (),
        max_items: int = 5,
        kind: str | None = None,
    ) -> Mapping[str, Any]:
        args = ["recall"]
        if query:
            args.append(query)
        args.extend(["--mode", mode, "--max-items", str(max_items), "--json"])
        if scope is not None:
            args.extend(["--scope", scope])
        if kind is not None:
            args.extend(["--kind", kind])
        for marker in markers:
            args.extend(["--marker", marker])
        return self._run_json(args)

    def seal(self) -> Mapping[str, Any]:
        return self._run_json(["seal"])

    def checkpoint(self) -> Mapping[str, Any]:
        return self._run_json(["checkpoint", "--json"])

    def stats(self) -> Mapping[str, Any]:
        return self._run_json(["stats", "--json"])

    def validate(self, *, deep: bool = False) -> Mapping[str, Any]:
        args = ["validate", "--json"]
        if deep:
            args.insert(1, "--deep")
        return self._run_json(args, allow_failure=True)

    def rebuild_indexes(self) -> Mapping[str, Any]:
        return self._run_json(["rebuild-indexes", "--json"])

    def export_markdown(self, output_path: str | Path | None = None) -> Path:
        self._run_text(["export", "--format", "markdown"])
        default_path = self.store_path / "exports" / "memory.md"
        if output_path is None:
            return default_path

        output_path = Path(output_path)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(default_path, output_path)
        return output_path

    def _run_text(self, args: Sequence[str], *, allow_failure: bool = False) -> str:
        completed = self._run(args)
        if completed.returncode != 0 and not allow_failure:
            raise MgeCommandError(
                completed.args,
                completed.returncode,
                completed.stdout,
                completed.stderr,
            )
        return completed.stdout

    def _run_json(
        self, args: Sequence[str], *, allow_failure: bool = False
    ) -> Mapping[str, Any]:
        output = self._run_text(args, allow_failure=allow_failure)
        return json.loads(output)

    def _run(self, args: Sequence[str]) -> subprocess.CompletedProcess[str]:
        command = [*self.command, "--store", str(self.store_path), *args]
        return subprocess.run(
            command,
            cwd=str(self.cwd) if self.cwd is not None else None,
            capture_output=True,
            text=True,
            check=False,
        )


def _default_command() -> list[str]:
    configured = os.environ.get("MGE_CLI")
    if configured:
        return configured.split()
    return ["mge"]


__all__ = ["MemoryGenomeClient", "MgeCommandError"]
