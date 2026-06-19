# Memory Genome Engine
# Copyright (c) 2026 ECD5A
# Project: https://github.com/ECD5A/Memory-Genome-Engine
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at:
#
#     https://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
from pathlib import Path
from typing import Any, Iterable, Literal, Mapping, Sequence, TypedDict, cast


RecallMode = Literal["focused", "broad", "full_scope", "full-scope"]


class RememberOptions(TypedDict, total=False):
    kind: str
    scope: str
    markers: list[str]
    trust: str
    sensitivity: str
    status: str
    subject: str


class SessionTurnInput(TypedDict):
    role: str
    content: str


class SessionRememberResult(TypedDict):
    turns: int
    chunks: int
    cells: list[Mapping[str, Any]]


class ContextMemoryItem(TypedDict, total=False):
    kind: str
    content: str
    trust: str
    status: str
    scope: str
    sensitivity: str
    markers: list[str]


class ContextPacketDebug(TypedDict, total=False):
    recall_mode: str
    max_items: int
    index_kind: str
    returned_items: int
    total_recall_micros: int


class ContextPacket(TypedDict, total=False):
    query: str
    relevant_memory: list[ContextMemoryItem]
    constraints: list[str]
    warnings: list[str]
    debug: ContextPacketDebug


class StoreStats(TypedDict, total=False):
    hot_cells: int
    sealed_pages: int
    sealed_cells: int
    marker_count: int
    current_security_mode: str
    current_index_kind: str
    store_size_bytes: int


class SecurityConfig(TypedDict, total=False):
    mode: str
    payload_encryption: bool
    session_unlock_required: bool
    metadata_plaintext: bool
    implementation_status: str


class ValidationReport(TypedDict, total=False):
    ok: bool
    index_kind: str
    checked_hot_cells: int
    checked_sealed_pages: int
    checked_sealed_cells: int
    errors: list[str]
    warnings: list[str]


class McpError(TypedDict, total=False):
    code: int
    message: str
    tool_name: str
    recoverable: bool
    protocol_version: str
    integration_schema_version: int
    details: Mapping[str, Any]


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


class MgeProtocolError(RuntimeError):
    def __init__(
        self,
        *,
        code: int,
        message: str,
        tool_name: str,
        recoverable: bool,
        details: Mapping[str, Any] | None = None,
    ) -> None:
        super().__init__(f"{tool_name}: {message}")
        self.code = code
        self.tool_name = tool_name
        self.recoverable = recoverable
        self.details = details or {}


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
        passphrase_env: str | None = None,
    ) -> None:
        self.store_path = Path(store_path)
        self.command = list(command) if command is not None else _default_command()
        self.cwd = Path(cwd) if cwd is not None else None
        self.passphrase_env = passphrase_env

    def init(self, profile: str = "fast", *, encrypted: bool = False) -> str:
        args = ["init", "--profile", profile]
        if encrypted:
            args.append("--encrypted")
        args.extend(self._security_args())
        return self._run_text(args)

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
        args.extend(self._security_args())

        output = self._run_text(args)
        match = re.search(r"Remembered cell (\d+)", output)
        if match is None:
            raise RuntimeError(f"could not parse remembered cell id from: {output!r}")
        return int(match.group(1))

    def recall(
        self,
        query: str = "",
        *,
        mode: RecallMode = "focused",
        scope: str | None = None,
        markers: Iterable[str] = (),
        max_items: int = 5,
        kind: str | None = None,
    ) -> ContextPacket:
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
        args.extend(self._security_args())
        return cast(ContextPacket, self._run_json(args))

    def remember_session(
        self,
        turns: Iterable[SessionTurnInput],
        *,
        session_id: str | None = None,
        kind: str = "project_fact",
        scope: str = "global",
        subject: str | None = None,
        markers: Iterable[str] = (),
        trust: str = "tool_observed",
        sensitivity: str = "private",
        status: str = "active",
        max_turns: int = 8,
        max_bytes: int = 4096,
    ) -> SessionRememberResult:
        args = [
            "remember-session",
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
            "--max-turns",
            str(max_turns),
            "--max-bytes",
            str(max_bytes),
            "--json",
        ]
        if session_id is not None:
            args.extend(["--session-id", session_id])
        if subject is not None:
            args.extend(["--subject", subject])
        for marker in markers:
            args.extend(["--marker", marker])
        for turn in turns:
            args.extend(["--turn", f"{turn['role']}={turn['content']}"])
        args.extend(self._security_args())
        return cast(SessionRememberResult, self._run_json(args))

    def seal(self) -> Mapping[str, Any]:
        return self._run_json(["seal", *self._security_args()])

    def checkpoint(self) -> Mapping[str, Any]:
        return self._run_json(["checkpoint", "--json", *self._security_args()])

    def stats(self) -> StoreStats:
        return cast(StoreStats, self._run_json(["stats", "--json", *self._security_args()]))

    def security_config(self) -> SecurityConfig:
        return cast(SecurityConfig, self._run_json(["config", "security", "--json"]))

    def validate(self, *, deep: bool = False) -> ValidationReport:
        args = ["validate", "--json"]
        if deep:
            args.insert(1, "--deep")
        args.extend(self._security_args())
        return cast(ValidationReport, self._run_json(args, allow_failure=True))

    def rebuild_indexes(self) -> Mapping[str, Any]:
        return self._run_json(["rebuild-indexes", "--json", *self._security_args()])

    def export_markdown(self, output_path: str | Path | None = None) -> Path:
        self._run_text(["export", "--format", "markdown", *self._security_args()])
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

    def _security_args(self) -> list[str]:
        if self.passphrase_env is None:
            return []
        return ["--passphrase-env", self.passphrase_env]

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


def result_or_raise_mcp_error(response: Mapping[str, Any]) -> Mapping[str, Any]:
    """Return JSON-RPC result or raise a typed protocol error.

    This helper is for callers that talk directly to `mge-mcp-server`.
    """

    error = response.get("error")
    if isinstance(error, Mapping):
        typed_error = cast(McpError, error)
        raise MgeProtocolError(
            code=int(typed_error.get("code", -32000)),
            message=str(typed_error.get("message", "unknown MCP error")),
            tool_name=str(typed_error.get("tool_name", "unknown")),
            recoverable=bool(typed_error.get("recoverable", False)),
            details=typed_error.get("details"),
        )

    result = response.get("result")
    if isinstance(result, Mapping):
        return result

    raise MgeProtocolError(
        code=-32603,
        message="JSON-RPC response has no result or structured error",
        tool_name="unknown",
        recoverable=False,
    )


__all__ = [
    "ContextMemoryItem",
    "ContextPacket",
    "ContextPacketDebug",
    "McpError",
    "MemoryGenomeClient",
    "MgeCommandError",
    "MgeProtocolError",
    "RecallMode",
    "RememberOptions",
    "SessionRememberResult",
    "SessionTurnInput",
    "SecurityConfig",
    "StoreStats",
    "ValidationReport",
    "result_or_raise_mcp_error",
]
