#!/usr/bin/env python3
"""Smoke-tests the release `clippings lsp` server against the tilliX
benchmark repository (Plan 2, Task 16 / spec section 13): time to the
first full tree, file-churn latency over `workspace/didChangeWatchedFiles`,
and that events under `node_modules` never trigger a rescan.

The server registers its file watchers with the client (dynamic
registration); this script plays the client, so it sends every
`workspace/didChangeWatchedFiles` notification itself -- nothing here
relies on a real filesystem watcher.

Safe to re-run: every source file it edits is restored to its original
bytes, even on failure (try/finally), and it never creates anything under
`node_modules` -- those events are synthetic paths only, used to prove the
server does not stat or rescan them.

Usage:
    cargo build --release            # once
    python3 scripts/smoke-tillix.py [--root ~/tilli/tilliX] [--build]

Prints one line per measurement, then a final `SUMMARY {...}` JSON line.
Exit code is 0 only if the tilliX working tree ended exactly as it
started, the node_modules check saw no rescan, and the server exited 0.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import select
import subprocess
import sys
import time
from pathlib import Path
from typing import Optional

TAGS = ["BUG", "HACK", "FIXME", "TODO", "XXX", "[ ]", "QUESTION"]
SOURCE_EXTS = {".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".rs"}
CHURN_FILES = 20
NODE_MODULES_FILES = 50
NODE_MODULES_WINDOW_S = 1.0
COUNT_RE = re.compile(r"\$\(check\)\s+(\d+)")


def log(msg: str) -> None:
    print(msg, flush=True)


class Framed:
    """Reads/writes Content-Length-framed JSON-RPC over a pipe, with an
    optional deadline per read so the churn / node_modules checks can
    bound their wait instead of blocking forever."""

    def __init__(self, proc: subprocess.Popen):
        self.proc = proc
        self.fd = proc.stdout.fileno()
        self.buf = b""

    def send(self, message: dict) -> None:
        body = json.dumps(message).encode()
        header = b"Content-Length: %d\r\n\r\n" % len(body)
        self.proc.stdin.write(header + body)
        self.proc.stdin.flush()

    def _fill(self, deadline: Optional[float]) -> bool:
        if deadline is not None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                return False
            r, _, _ = select.select([self.fd], [], [], remaining)
            if not r:
                return False
        else:
            select.select([self.fd], [], [], None)
        chunk = os.read(self.fd, 65536)
        if not chunk:
            raise EOFError("server closed stdout")
        self.buf += chunk
        return True

    def recv(self, deadline: Optional[float] = None) -> Optional[dict]:
        """Reads one message, or returns None once `deadline` passes."""
        while b"\r\n\r\n" not in self.buf:
            if not self._fill(deadline):
                return None
        idx = self.buf.index(b"\r\n\r\n")
        header = self.buf[:idx].decode()
        length = None
        for line in header.split("\r\n"):
            if line.lower().startswith("content-length"):
                length = int(line.split(":", 1)[1].strip())
        if length is None:
            raise ValueError(f"no Content-Length header: {header!r}")
        body_start = idx + 4
        need = body_start + length
        while len(self.buf) < need:
            if not self._fill(deadline):
                return None
        body = self.buf[body_start:need]
        self.buf = self.buf[need:]
        return json.loads(body)


def uri(path: Path) -> str:
    return "file://" + str(path)


def count_from_status(params: dict) -> Optional[int]:
    m = COUNT_RE.search(params.get("statusBar", {}).get("text", ""))
    return int(m.group(1)) if m else None


def wait_for_count(client: Framed, target: int, timeout: float) -> float:
    """Reads statuses until one reports `target`. Returns elapsed ms."""
    deadline = time.monotonic() + timeout
    start = time.monotonic()
    seen: list[Optional[int]] = []
    while True:
        m = client.recv(deadline=deadline)
        if m is None:
            raise TimeoutError(
                f"count never reached {target} within {timeout}s; saw {seen}"
            )
        if m.get("method") == "clippings/status":
            n = count_from_status(m["params"])
            seen.append(n)
            if n == target:
                return (time.monotonic() - start) * 1000.0


def drain_for(client: Framed, seconds: float) -> list[dict]:
    """Reads every message that arrives within `seconds`, then returns."""
    deadline = time.monotonic() + seconds
    seen = []
    while True:
        m = client.recv(deadline=deadline)
        if m is None:
            return seen
        seen.append(m)


def rg_files(root: Path) -> list[Path]:
    out = subprocess.run(
        ["rg", "--files", str(root)], capture_output=True, text=True, check=True
    )
    return [Path(line) for line in out.stdout.splitlines() if line]


def clippings_scan_count(binary: Path, config: Path, root: Path) -> tuple[int, int]:
    out = subprocess.run(
        [str(binary), "scan", "--json", "--config", str(config), str(root)],
        capture_output=True,
        text=True,
        check=True,
    )
    data = json.loads(out.stdout)
    todos = sum(len(f["todos"]) for f in data["files"])
    return todos, len(data["files"])


def pick_churn_files(files: list[Path], n: int) -> list[Path]:
    picked = []
    for p in files:
        if p.suffix in SOURCE_EXTS:
            try:
                size = p.stat().st_size
            except OSError:
                continue
            if 0 < size < 500_000:
                picked.append(p)
        if len(picked) >= n:
            break
    if len(picked) < n:
        raise SystemExit(
            f"only found {len(picked)} candidate source files under {SOURCE_EXTS}, need {n}"
        )
    return picked


def git_status(root: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(root), "status", "--porcelain"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default="~/tilli/tilliX")
    parser.add_argument(
        "--build", action="store_true", help="run cargo build --release first"
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parent.parent
    tillix = Path(os.path.expanduser(args.root)).resolve()
    binary = repo_root / "target" / "release" / "clippings"

    if args.build or not binary.exists():
        log("building release binary...")
        subprocess.run(["cargo", "build", "--release"], cwd=repo_root, check=True)

    pre_status = git_status(tillix)

    file_count = len(rg_files(tillix))
    log(f"scan-set file count (rg --files): {file_count}")

    bench_dir = repo_root / "target" / "bench"
    bench_dir.mkdir(parents=True, exist_ok=True)
    tags_config = bench_dir / "tags.json"
    tags_config.write_text(json.dumps({"tags": TAGS}))
    scan_todos, scan_files = clippings_scan_count(binary, tags_config, tillix)
    log(f"clippings scan todo count: {scan_todos} todos in {scan_files} files")

    all_files = rg_files(tillix)
    churn_files = pick_churn_files(all_files, CHURN_FILES)
    log(f"churn files: {len(churn_files)} picked")

    summary: dict = {
        "file_count": file_count,
        "scan_todos": scan_todos,
        "scan_files": scan_files,
    }
    originals: dict[Path, bytes] = {}
    error: Optional[str] = None

    proc = subprocess.Popen(
        [str(binary), "lsp"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        bufsize=0,
    )
    client = Framed(proc)
    try:
        t0 = time.monotonic()
        client.send(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "workspaceFolders": [{"uri": uri(tillix), "name": "tilliX"}],
                    "capabilities": {
                        "workspace": {
                            "didChangeWatchedFiles": {"dynamicRegistration": True}
                        }
                    },
                    "initializationOptions": {
                        "protocolVersion": 1,
                        "settings": {
                            "general": {"tags": TAGS, "statusBar": "total"}
                        },
                    },
                },
            }
        )
        init_deadline = time.monotonic() + 60.0
        r = client.recv(deadline=init_deadline)
        if r is None:
            raise TimeoutError("no response to initialize")
        client.send({"jsonrpc": "2.0", "method": "initialized", "params": {}})

        baseline = None
        first_tree_ms = None
        first_tree_deadline = time.monotonic() + 60.0
        while True:
            m = client.recv(deadline=first_tree_deadline)
            if m is None:
                raise TimeoutError("no first full tree within 60s")
            if m.get("method") == "clippings/status":
                p = m["params"]
                if p["scanning"] is False and p["statusBar"]["text"] != "$(check) 0":
                    first_tree_ms = (time.monotonic() - t0) * 1000.0
                    baseline = count_from_status(p)
                    log(
                        f"first full tree after {first_tree_ms:.0f} ms: "
                        f"{p['statusBar']['text']}"
                    )
                    break
        summary["first_tree_ms"] = first_tree_ms
        summary["baseline_count"] = baseline
        summary["count_matches_scan"] = baseline == scan_todos
        if baseline != scan_todos:
            log(
                f"FAILED: LSP baseline count {baseline} != "
                f"clippings scan count {scan_todos}"
            )

        # --- (a) file churn: append a todo to 20 admitted files, measure
        # the round trip, restore them exactly, and confirm the count
        # comes back down.
        for path in churn_files:
            originals[path] = path.read_bytes()
        for i, path in enumerate(churn_files):
            addition = originals[path]
            if not addition.endswith(b"\n"):
                addition += b"\n"
            addition += f"// TODO churn {i}\n".encode()
            path.write_bytes(addition)
        client.send(
            {
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWatchedFiles",
                "params": {
                    "changes": [{"uri": uri(p), "type": 2} for p in churn_files]
                },
            }
        )
        churn_up_ms = wait_for_count(client, baseline + CHURN_FILES, timeout=30.0)
        log(f"churn up (+{CHURN_FILES}) after {churn_up_ms:.0f} ms")
        summary["churn_up_ms"] = churn_up_ms

        for path, data in originals.items():
            path.write_bytes(data)
        client.send(
            {
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWatchedFiles",
                "params": {
                    "changes": [{"uri": uri(p), "type": 2} for p in churn_files]
                },
            }
        )
        churn_down_ms = wait_for_count(client, baseline, timeout=30.0)
        log(f"churn down (restored) after {churn_down_ms:.0f} ms")
        summary["churn_down_ms"] = churn_down_ms

        # --- (b) node_modules: a Created event for the directory itself
        # and for 50 file paths under it -- none of it touches disk.
        node_modules = tillix / "node_modules"
        nm_changes = [{"uri": uri(node_modules), "type": 1}]
        for i in range(NODE_MODULES_FILES):
            nm_changes.append(
                {"uri": uri(node_modules / f"pkg{i}" / "index.js"), "type": 1}
            )
        client.send(
            {
                "jsonrpc": "2.0",
                "method": "workspace/didChangeWatchedFiles",
                "params": {"changes": nm_changes},
            }
        )
        seen = drain_for(client, NODE_MODULES_WINDOW_S)
        statuses = [m["params"] for m in seen if m.get("method") == "clippings/status"]
        saw_scanning = any(p["scanning"] for p in statuses)
        counts = [count_from_status(p) for p in statuses]
        saw_count_change = any(n is not None and n != baseline for n in counts)
        # Silence alone would also fit a hung server: require an answer.
        client.send(
            {
                "jsonrpc": "2.0",
                "id": 2,
                "method": "clippings/children",
                "params": {"parent": None},
            }
        )
        alive_deadline = time.monotonic() + 2.0
        alive = False
        while not alive:
            m = client.recv(deadline=alive_deadline)
            if m is None:
                break
            if m.get("method") == "clippings/status":
                saw_scanning |= bool(m["params"]["scanning"])
            alive = m.get("id") == 2 and "result" in m
        summary["node_modules_alive"] = alive
        node_modules_ok = alive and not saw_scanning and not saw_count_change
        log(
            f"node_modules events: {len(statuses)} status message(s) in "
            f"{NODE_MODULES_WINDOW_S:.0f}s, scanning seen={saw_scanning}, "
            f"count change={saw_count_change} -> "
            f"{'OK, no rescan' if node_modules_ok else 'FAILED'}"
        )
        summary["node_modules_ok"] = node_modules_ok
        summary["node_modules_status_count"] = len(statuses)

        # --- shutdown ---
        client.send({"jsonrpc": "2.0", "id": 3, "method": "shutdown"})
        shutdown_deadline = time.monotonic() + 10.0
        while True:
            m = client.recv(deadline=shutdown_deadline)
            if m is None:
                raise TimeoutError("no shutdown response within 10s")
            if m.get("id") == 3:
                break
        client.send({"jsonrpc": "2.0", "method": "exit"})
        exit_code = proc.wait(timeout=10)
        log(f"exit code {exit_code}")
        summary["exit_code"] = exit_code
    except Exception as e:  # noqa: BLE001 - report and still clean up below
        error = f"{type(e).__name__}: {e}"
        log(f"ERROR: {error}")
    finally:
        # Belt and suspenders: restore anything still modified, whatever
        # went wrong and wherever it happened.
        for path, data in originals.items():
            if path.exists() and path.read_bytes() != data:
                path.write_bytes(data)
        if proc.poll() is None:
            proc.kill()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                pass

    post_status = git_status(tillix)
    clean = pre_status == post_status
    summary["tillix_git_status_unchanged"] = clean
    log(f"tilliX git status unchanged: {clean}")
    if not clean:
        log("PRE  git status --porcelain:\n" + pre_status)
        log("POST git status --porcelain:\n" + post_status)

    log("SUMMARY " + json.dumps(summary))
    ok = (
        error is None
        and clean
        and summary.get("node_modules_ok") is True
        and summary.get("count_matches_scan") is True
        and summary.get("exit_code") == 0
    )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
