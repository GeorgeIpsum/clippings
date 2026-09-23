#!/usr/bin/env python3
"""Measures the memory of the release `clippings lsp` server over a
session, and of `clippings scan` and `clippings watch`, for
docs/benchmarks/2026-09-memory.md. macOS only: it reads the process's
resident size and physical footprint with `proc_pid_rusage`, `ps` and
`footprint`.

Subcommands:

  session ROOT [--hidden]   One LSP session with phase samples: after
                            initialize, first tree, idle, didOpen of 20
                            source files and the largest file, churn
                            (watched-file bursts plus didChange edits),
                            after churn, repeated clippings/rescan, close.
  first-tree ROOT...        Footprint after the first tree, per root, for
                            the scaling table.
  soak ROOT                 Many cycles of rescan, watched-file churn and
                            edits, sampled every N cycles (--no-writes:
                            rescans only).
  generate DIR KIND         Writes a synthetic repository (see KINDS).
  cli ROOT                  Peak and steady memory of `clippings scan`
                            and `clippings watch`.

A sampler thread reads resident size and footprint every 20 ms, so each
phase also reports the maximum seen since the previous phase. RSS on
macOS counts shared pages such as the dyld cache and the binary's text;
the physical footprint is what Activity Monitor calls Memory, and is the
number to compare.

Safe to run against a real repository: the churn phase appends a line to
files and restores their exact bytes in a `finally` block, as
`scripts/smoke-tillix.py` does, and `git status --porcelain` is compared
before and after. It churns only files without uncommitted changes, and
restores a file only if it still holds the bytes the script wrote, so an
edit someone makes during the run is never overwritten. Nothing else is
written under ROOT.

Usage:
    cargo build --release
    python3 scripts/memprofile.py session ~/tilli/tilliX [--json out.json]
    python3 scripts/memprofile.py generate /tmp/syn-10k files10k
    python3 scripts/memprofile.py first-tree /tmp/syn-10k /tmp/syn-50k
    python3 scripts/memprofile.py cli ~/tilli/tilliX
"""
from __future__ import annotations

import argparse
import ctypes
import importlib.util
import json
import os
import re
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Optional

REPO = Path(__file__).resolve().parent.parent
BINARY = REPO / "target" / "release" / "clippings"

_spec = importlib.util.spec_from_file_location(
    "smoke_tillix", REPO / "scripts" / "smoke-tillix.py"
)
smoke = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(smoke)
Framed, uri, count_from_status = smoke.Framed, smoke.uri, smoke.count_from_status
TAGS = smoke.TAGS

SAMPLE_S = 0.02
MIB = 1024 * 1024


def log(msg: str) -> None:
    print(msg, flush=True)


# ---------------------------------------------------------------- process


class _RusageV4(ctypes.Structure):
    _fields_ = [("uuid", ctypes.c_uint8 * 16), ("v", ctypes.c_uint64 * 40)]


_libproc = ctypes.CDLL("/usr/lib/libproc.dylib")
RESIDENT, FOOTPRINT, LIFETIME_MAX_FOOTPRINT = 6, 7, 28


def rusage(pid: int) -> Optional[dict]:
    """Resident size, physical footprint and lifetime peak footprint, in bytes."""
    r = _RusageV4()
    if _libproc.proc_pid_rusage(pid, 4, ctypes.byref(r)) != 0:
        return None
    return {
        "resident": r.v[RESIDENT],
        "footprint": r.v[FOOTPRINT],
        "lifetime_max_footprint": r.v[LIFETIME_MAX_FOOTPRINT],
    }


def ps_rss(pid: int) -> Optional[int]:
    out = subprocess.run(["ps", "-o", "rss=", "-p", str(pid)], capture_output=True, text=True)
    return int(out.stdout.strip()) * 1024 if out.stdout.strip() else None


def threads(pid: int) -> Optional[int]:
    out = subprocess.run(["ps", "-M", "-p", str(pid)], capture_output=True, text=True)
    lines = [l for l in out.stdout.splitlines()[1:] if l.strip()]
    return len(lines) or None


_UNITS = {"B": 1, "KB": 1024, "MB": MIB, "GB": 1024 * MIB}


def footprint_tool(pid: int) -> tuple[Optional[int], dict]:
    """`footprint PID`: the total and the dirty bytes per category."""
    out = subprocess.run(["footprint", str(pid)], capture_output=True, text=True).stdout
    total = None
    m = re.search(r"Footprint:\s+([\d.]+)\s+(B|KB|MB|GB)", out)
    if m:
        total = int(float(m.group(1)) * _UNITS[m.group(2)])
    cats = {}
    for line in out.splitlines():
        m = re.match(r"\s*([\d.]+)\s+(B|KB|MB|GB)\s+[\d.]+\s+(?:B|KB|MB|GB)\s+[\d.]+\s+(?:B|KB|MB|GB)\s+\d+\s+(.+)$", line)
        if m and m.group(3).strip() != "TOTAL":
            cats[m.group(3).strip()] = int(float(m.group(1)) * _UNITS[m.group(2)])
    return total, cats


class Sampler(threading.Thread):
    """Reads resident size and footprint every 20 ms until stopped."""

    def __init__(self, pid: int):
        super().__init__(daemon=True)
        self.pid = pid
        self.samples: list[tuple[float, int, int]] = []
        self.stop = threading.Event()

    def run(self) -> None:
        while not self.stop.is_set():
            r = rusage(self.pid)
            if r is None:
                return
            self.samples.append((time.monotonic(), r["resident"], r["footprint"]))
            time.sleep(SAMPLE_S)

    def max_since(self, t0: float) -> tuple[int, int]:
        window = [s for s in self.samples if s[0] >= t0]
        if not window:
            return 0, 0
        return max(s[1] for s in window), max(s[2] for s in window)


def read_memlog(path: Optional[Path]) -> list[list[int]]:
    """Lines of the instrumented server's log (examples/memprofile_lsp.rs):
    ms, live requested, live reserved, allocations, zone in use, zone held."""
    if path is None or not path.exists():
        return []
    rows = []
    for line in path.read_text().splitlines():
        parts = line.split()
        if len(parts) == 6:
            rows.append([int(x) for x in parts])
    return rows


class Phases:
    """Takes one sample per phase: ps RSS, rusage, `footprint`, thread count,
    and the sampler's maxima since the previous phase. With the instrumented
    server it also reads the live heap bytes from its log."""

    def __init__(self, pid: int, sampler: Sampler, detail: bool = True,
                 memlog: Optional[Path] = None):
        self.pid, self.sampler, self.detail = pid, sampler, detail
        self.memlog = memlog
        self.log_seen = 0
        self.rows: list[dict] = []
        self.since = time.monotonic()

    def mark(self, name: str, **extra) -> dict:
        r = rusage(self.pid) or {}
        max_rss, max_fp = self.sampler.max_since(self.since)
        row = {
            "phase": name,
            "ps_rss": ps_rss(self.pid),
            "resident": r.get("resident"),
            "footprint": r.get("footprint"),
            "lifetime_max_footprint": r.get("lifetime_max_footprint"),
            "max_resident_since_last": max(max_rss, r.get("resident", 0)),
            "max_footprint_since_last": max(max_fp, r.get("footprint", 0)),
            "threads": threads(self.pid),
            **extra,
        }
        if self.memlog is not None:
            time.sleep(0.05)
            log_rows = read_memlog(self.memlog)
            window = log_rows[self.log_seen:] or log_rows[-1:]
            self.log_seen = len(log_rows)
            if window:
                last = window[-1]
                row.update({"heap_live": last[1], "heap_reserved": last[2],
                            "heap_allocs": last[3], "zone_in_use": last[4],
                            "zone_held": last[5],
                            "heap_live_max_since_last": max(r[1] for r in window),
                            "zone_held_max_since_last": max(r[5] for r in window)})
        if self.detail:
            total, cats = footprint_tool(self.pid)
            row["footprint_tool"] = total
            row["footprint_categories"] = cats
        self.rows.append(row)
        self.since = time.monotonic()
        log(
            f"  {name:<34} rss {mb(row['ps_rss'])}  footprint {mb(row['footprint'])}"
            f"  max since last {mb(row['max_footprint_since_last'])}"
            f"  threads {row['threads']}"
            + (f"  heap {mb(row.get('heap_live'))} (max {mb(row.get('heap_live_max_since_last'))})"
               f"  zones hold {mb(row.get('zone_held'))}" if "heap_live" in row else "")
        )
        return row


def mb(b: Optional[int]) -> str:
    return "   n/a" if b is None else f"{b / MIB:6.1f} MiB"


# ----------------------------------------------------------------- client


class Client:
    """The LSP client side: sends messages and pumps the server's output so
    its writer never blocks, tracking the last status."""

    def __init__(self, proc: subprocess.Popen):
        self.f = Framed(proc)
        self.count: Optional[int] = None
        self.scanning: Optional[bool] = None
        self.next_id = 100
        self.decorations: dict[str, int] = {}

    def send(self, m: dict) -> None:
        self.f.send({"jsonrpc": "2.0", **m})

    def notify(self, method: str, params: dict) -> None:
        self.send({"method": method, "params": params})

    def _seen(self, m: dict) -> None:
        if m.get("method") == "clippings/status":
            self.scanning = m["params"]["scanning"]
            n = count_from_status(m["params"])
            if n is not None:
                self.count = n
        elif m.get("method") == "clippings/decorations":
            u = m["params"]["uri"]
            self.decorations[u] = self.decorations.get(u, 0) + 1
        elif "id" in m and "method" in m:
            # A server request (register/unregisterCapability): answer it.
            self.send({"id": m["id"], "result": None})

    def pump(self, seconds: float) -> None:
        deadline = time.monotonic() + seconds
        while True:
            m = self.f.recv(deadline=deadline)
            if m is None:
                return
            self._seen(m)

    def until(self, pred, timeout: float) -> bool:
        deadline = time.monotonic() + timeout
        while not pred():
            m = self.f.recv(deadline=deadline)
            if m is None:
                return False
            self._seen(m)
        return True

    def request(self, method: str, params, timeout: float = 30.0):
        self.next_id += 1
        rid = self.next_id
        self.send({"id": rid, "method": method, "params": params})
        deadline = time.monotonic() + timeout
        while True:
            m = self.f.recv(deadline=deadline)
            if m is None:
                raise TimeoutError(f"no response to {method}")
            if m.get("id") == rid and "method" not in m:
                return m
            self._seen(m)


def start_server(root: Path, hidden: bool, binary: Optional[Path] = None,
                 env: Optional[dict] = None) -> tuple[subprocess.Popen, Client, dict]:
    """Starts the server: the release binary, or an instrumented example
    binary that runs the same loop without the `lsp` argument."""
    cmd = [str(BINARY), "lsp"] if binary is None else [str(binary)]
    proc = subprocess.Popen(
        cmd,
        env={**os.environ, **(env or {})},
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        bufsize=0,
    )
    c = Client(proc)
    settings = {
        "general": {"tags": TAGS, "statusBar": "total"},
        "filtering": {"includeHiddenFiles": hidden},
    }
    c.send(
        {
            "id": 1,
            "method": "initialize",
            "params": {
                "workspaceFolders": [{"uri": uri(root), "name": root.name}],
                "capabilities": {
                    "workspace": {"didChangeWatchedFiles": {"dynamicRegistration": True}}
                },
                "initializationOptions": {"protocolVersion": 1, "settings": settings},
            },
        }
    )
    if c.f.recv(deadline=time.monotonic() + 30) is None:
        raise TimeoutError("no initialize response")
    return proc, c, settings


def first_tree(c: Client, timeout: float = 600.0) -> float:
    """Sends `initialized` and waits for the first finished scan. Returns ms."""
    t0 = time.monotonic()
    c.notify("initialized", {})
    if not c.until(lambda: c.scanning is True, timeout):
        raise TimeoutError("scan never started")
    if not c.until(lambda: c.scanning is False, timeout):
        raise TimeoutError("no first tree")
    c.pump(0.3)  # the 50 ms view rebuild after the walk lands
    return (time.monotonic() - t0) * 1000.0


def stop_server(proc: subprocess.Popen, c: Client) -> Optional[int]:
    try:
        c.request("shutdown", None, timeout=10)
        c.notify("exit", {})
        return proc.wait(timeout=10)
    finally:
        if proc.poll() is None:
            proc.kill()


def rescan(c: Client, timeout: float = 600.0) -> float:
    t0 = time.monotonic()
    c.notify("clippings/rescan", {})
    c.until(lambda: c.scanning is True, timeout)
    c.until(lambda: c.scanning is False, timeout)
    ms = (time.monotonic() - t0) * 1000.0
    c.pump(0.3)
    return ms


# ---------------------------------------------------------------- session


def git_status(root: Path) -> Optional[str]:
    r = subprocess.run(["git", "-C", str(root), "status", "--porcelain"], capture_output=True, text=True)
    return r.stdout if r.returncode == 0 else None


def dirty_paths(root: Path) -> set[Path]:
    """Files with uncommitted changes: never churned, since someone may be
    editing them."""
    status = git_status(root) or ""
    return {(root / line[3:].split(" -> ")[-1]).resolve() for line in status.splitlines() if len(line) > 3}


def churn_candidates(root: Path, files: list[Path], n: int) -> list[Path]:
    dirty = dirty_paths(root)
    return smoke.pick_churn_files([p for p in files if p.resolve() not in dirty], n)


class Restorer:
    """Remembers each churned file's original bytes and what was last
    written, and restores a file only while it still holds our bytes, so a
    concurrent edit by someone else is never overwritten."""

    def __init__(self, paths: list[Path]):
        self.originals = {p: p.read_bytes() for p in paths}
        self.written = dict(self.originals)
        self.skipped: list[str] = []

    def write(self, p: Path, data: bytes) -> None:
        if p.read_bytes() != self.written[p]:
            raise RuntimeError(f"{p} changed under the churn; stopping")
        p.write_bytes(data)
        self.written[p] = data

    def restore(self) -> None:
        for p, data in self.originals.items():
            current = p.read_bytes() if p.exists() else None
            if current == data:
                continue
            if current == self.written[p]:
                p.write_bytes(data)
                self.written[p] = data
            else:
                self.skipped.append(str(p))
                log(f"WARNING: {p} was changed by someone else; left as is")


def scan_set(root: Path, hidden: bool) -> list[Path]:
    args = ["rg", "--files"] + (["--hidden"] if hidden else []) + [str(root)]
    out = subprocess.run(args, capture_output=True, text=True, check=True).stdout
    return [Path(l) for l in out.splitlines() if l]


def largest_text_file(files: list[Path]) -> Path:
    for p in sorted(files, key=lambda p: p.stat().st_size, reverse=True):
        head = p.read_bytes()[:65536]
        if b"\0" not in head:
            try:
                head.decode("utf-8")
                return p
            except UnicodeDecodeError:
                pass
    raise SystemExit("no text file")


def session(args) -> dict:
    root = Path(os.path.expanduser(args.root)).resolve()
    files = scan_set(root, args.hidden)
    sources = churn_candidates(root, files, 40)
    churn_files, open_files = sources[:20], sources[20:]
    big = largest_text_file(files)
    log(f"root {root}: rg scan set {len(files)} files; big file {big} ({big.stat().st_size} bytes)")
    pre = git_status(root)
    result: dict = {"root": str(root), "hidden": args.hidden, "rg_files": len(files),
                    "big_file": str(big), "big_file_bytes": big.stat().st_size}
    restorer: Optional[Restorer] = None
    memlog, env = None, {}
    if args.binary:
        memlog = Path(args.memlog).resolve()
        env["CLIPPINGS_MEMLOG"] = str(memlog)
        if args.relief_ms:
            env["CLIPPINGS_MEMLOG_RELIEF_MS"] = str(args.relief_ms)
        result["binary"] = args.binary
        result["relief_ms"] = args.relief_ms
    proc, c, _ = start_server(root, args.hidden, Path(args.binary) if args.binary else None, env)
    sampler = Sampler(proc.pid)
    sampler.start()
    ph = Phases(proc.pid, sampler, memlog=memlog)
    try:
        ph.mark("after initialize, before scan")
        ms = first_tree(c)
        result["first_tree_ms"] = ms
        result["todos"] = c.count
        ph.mark("first full tree", ms=round(ms), todos=c.count)
        c.pump(10.0)
        ph.mark("idle 10 s")

        # Open 20 source files and the largest file.
        docs: dict[str, dict] = {}
        for p in open_files + [big]:
            text = p.read_bytes().decode("utf-8", errors="replace")
            u = uri(p)
            docs[u] = {"version": 1, "text": text}
            c.notify("textDocument/didOpen", {"textDocument": {
                "uri": u, "languageId": "plaintext", "version": 1, "text": text}})
        c.until(lambda: all(u in c.decorations for u in docs), 30.0)
        c.pump(1.0)
        after_open_count = c.count
        ph.mark(f"after didOpen of {len(docs)} docs", todos=c.count)

        # Churn: watched-file bursts and didChange edit bursts.
        restorer = Restorer(churn_files)
        originals = restorer.originals
        result["churn_files"] = [str(p) for p in churn_files]
        edited = [uri(open_files[0]), uri(big)]
        t_end = time.monotonic() + args.churn_seconds
        t_mid = time.monotonic() + args.churn_seconds / 2
        mid_marked = False
        cycles = edits = 0
        while time.monotonic() < t_end:
            cycles += 1
            for i, p in enumerate(churn_files):
                data = originals[p]
                if cycles % 2 == 1:
                    data = data + (b"" if data.endswith(b"\n") else b"\n") + f"// TODO churn {i}\n".encode()
                restorer.write(p, data)
            c.notify("workspace/didChangeWatchedFiles",
                     {"changes": [{"uri": uri(p), "type": 2} for p in churn_files]})
            # Twelve edits 20 ms apart, alternately inserting and removing a
            # line in each of two documents, so every cycle nets to no change.
            for k in range(12):
                u = edited[k % 2]
                docs[u]["version"] += 1
                if (k // 2) % 2 == 0:
                    change = {"range": {"start": {"line": 0, "character": 0},
                                        "end": {"line": 0, "character": 0}},
                              "text": f"// TODO edit {edits}\n"}
                else:
                    change = {"range": {"start": {"line": 0, "character": 0},
                                        "end": {"line": 1, "character": 0}}, "text": ""}
                c.notify("textDocument/didChange", {
                    "textDocument": {"uri": u, "version": docs[u]["version"]},
                    "contentChanges": [change]})
                edits += 1
                c.pump(0.02)
            # Long enough for the 150 ms buffer rescan and 500 ms decorations.
            c.pump(0.7)
            if not mid_marked and time.monotonic() >= t_mid:
                ph.mark("during churn (midpoint)")
                mid_marked = True
        restorer.restore()
        c.notify("workspace/didChangeWatchedFiles",
                 {"changes": [{"uri": uri(p), "type": 2} for p in churn_files]})
        restored = c.until(lambda: c.count == after_open_count, 10.0)
        c.pump(2.0)
        result["churn"] = {"seconds": args.churn_seconds, "cycles": cycles,
                           "edits": edits, "count_restored": restored}
        ph.mark("churn stopped + 2 s", todos=c.count)
        c.pump(10.0)
        ph.mark("churn stopped + 12 s idle")

        rescans = []
        for i in range(args.rescans):
            ms = rescan(c)
            rescans.append(ms)
            ph.mark(f"after rescan {i + 1}", ms=round(ms), todos=c.count)
        result["rescan_ms"] = rescans

        for u in docs:
            c.notify("textDocument/didClose", {"textDocument": {"uri": u}})
        c.pump(2.0)
        ph.mark("after didClose of all docs", todos=c.count)
        c.pump(10.0)
        ph.mark("idle 10 s after close")
        result["exit_code"] = stop_server(proc, c)
    finally:
        if restorer is not None:
            restorer.restore()
            result["restore_skipped"] = restorer.skipped
        sampler.stop.set()
        if proc.poll() is None:
            proc.kill()
    post = git_status(root)
    result["git_status_unchanged"] = pre == post
    log(f"git status unchanged: {pre == post}")
    result["phases"] = ph.rows
    return result


# ------------------------------------------------------------------- soak


def soak(args) -> dict:
    """Many cycles of rescan, plus (unless --no-writes) a watched-file churn
    round trip and a didChange edit burst, with one sample per cycle: does
    anything grow?"""
    root = Path(os.path.expanduser(args.root)).resolve()
    files = scan_set(root, args.hidden)
    pre = git_status(root)
    memlog, env = None, {}
    if args.binary:
        memlog = Path(args.memlog).resolve()
        env["CLIPPINGS_MEMLOG"] = str(memlog)
        if args.relief_ms:
            env["CLIPPINGS_MEMLOG_RELIEF_MS"] = str(args.relief_ms)
    proc, c, _ = start_server(root, args.hidden, Path(args.binary) if args.binary else None, env)
    sampler = Sampler(proc.pid)
    sampler.start()
    ph = Phases(proc.pid, sampler, detail=False, memlog=memlog)
    restorer: Optional[Restorer] = None
    result: dict = {"root": str(root), "cycles": args.cycles, "writes": not args.no_writes,
                    "relief_ms": args.relief_ms}
    try:
        first_tree(c)
        ph.mark("first tree", todos=c.count)
        edited = None
        if not args.no_writes:
            churn_files = churn_candidates(root, files, 21)
            edited, churn_files = churn_files[0], churn_files[1:]
            restorer = Restorer(churn_files)
            result["churn_files"] = [str(p) for p in churn_files]
            text = edited.read_bytes().decode("utf-8", errors="replace")
            c.notify("textDocument/didOpen", {"textDocument": {
                "uri": uri(edited), "languageId": "plaintext", "version": 1, "text": text}})
            c.pump(1.0)
        version = 1
        for cycle in range(1, args.cycles + 1):
            if restorer is not None:
                for i, p in enumerate(restorer.originals):
                    data = restorer.originals[p] + f"\n// TODO soak {i}\n".encode()
                    restorer.write(p, data)
                c.notify("workspace/didChangeWatchedFiles",
                         {"changes": [{"uri": uri(p), "type": 2} for p in restorer.originals]})
                c.pump(0.3)
                restorer.restore()
                c.notify("workspace/didChangeWatchedFiles",
                         {"changes": [{"uri": uri(p), "type": 2} for p in restorer.originals]})
                for k in range(12):
                    version += 1
                    change = ({"range": {"start": {"line": 0, "character": 0},
                                         "end": {"line": 0, "character": 0}},
                               "text": f"// TODO soak edit {k}\n"} if k % 2 == 0 else
                              {"range": {"start": {"line": 0, "character": 0},
                                         "end": {"line": 1, "character": 0}}, "text": ""})
                    c.notify("textDocument/didChange", {
                        "textDocument": {"uri": uri(edited), "version": version},
                        "contentChanges": [change]})
                    c.pump(0.02)
                c.pump(0.7)
            ms = rescan(c)
            if cycle in args.heap_at:
                out = subprocess.run(["heap", str(proc.pid)], capture_output=True, text=True).stdout
                path = Path(args.heap_dir) / f"heap_{proc.pid}_cycle{cycle}.txt"
                path.write_text(out)
                result.setdefault("heap_snapshots", []).append(str(path))
            if cycle == 1 or cycle % args.every == 0:
                ph.mark(f"cycle {cycle}", ms=round(ms), todos=c.count)
        result["exit_code"] = stop_server(proc, c)
    finally:
        if restorer is not None:
            restorer.restore()
            result["restore_skipped"] = restorer.skipped
        sampler.stop.set()
        if proc.poll() is None:
            proc.kill()
    result["git_status_unchanged"] = pre == git_status(root)
    log(f"git status unchanged: {result['git_status_unchanged']}")
    result["phases"] = ph.rows
    return result


# ------------------------------------------------------------- first tree


def first_tree_only(args) -> dict:
    out = []
    for r in args.roots:
        root = Path(os.path.expanduser(r)).resolve()
        runs = []
        for _ in range(args.runs):
            proc, c, _ = start_server(root, args.hidden)
            sampler = Sampler(proc.pid)
            sampler.start()
            ph = Phases(proc.pid, sampler, detail=False)
            try:
                ph.mark("after initialize")
                ms = first_tree(c)
                ph.mark("first tree", ms=round(ms), todos=c.count)
                c.pump(3.0)
                idle = ph.mark("idle 3 s")
                ms2 = rescan(c)
                ph.mark("after rescan", ms=round(ms2))
                total, cats = footprint_tool(proc.pid)
                stop_server(proc, c)
            finally:
                sampler.stop.set()
                if proc.poll() is None:
                    proc.kill()
            runs.append({"first_tree_ms": ms, "todos": c.count, "phases": ph.rows,
                         "idle": idle, "footprint_tool": total, "categories": cats})
        log(f"{root}: " + ", ".join(
            f"{mb(x['idle']['footprint'])} idle / peak {mb(x['phases'][-1]['lifetime_max_footprint'])}"
            for x in runs))
        out.append({"root": str(root), "runs": runs})
    return {"first_tree": out}


# -------------------------------------------------------------- generator

BODY = [
    "import {{ thing{n} }} from './thing{n}';",
    "",
    "export function handler{n}(input: string): string {{",
    "  const value = input.trim().toLowerCase();",
    "  if (value.length === 0) {{",
    "    return 'empty';",
    "  }}",
    "  return `${{value}}-{n}`;",
    "}}",
]

KINDS = {
    # files, fraction of files with todos, todos per such file
    "files10k": (10_000, 0.2, 3),
    "files50k": (50_000, 0.2, 3),
    "files100k": (100_000, 0.2, 3),
    "sparse100k": (100_000, 0.01, 1),
    "heavy1k": (1_000, 1.0, 100),
}


def generate(args) -> None:
    """Deterministic synthetic repository: files at depth 4 below the root
    (pkgNN/src/modNN/subN/fileN.ts), about 30 lines each. Files with todos
    get their todos spread through the body."""
    files, frac, per = KINDS[args.kind]
    root = Path(args.dir)
    root.mkdir(parents=True, exist_ok=True)
    every = max(1, round(1 / frac))
    todos = 0
    for i in range(files):
        d = root / f"pkg{i % 40:02d}" / "src" / f"mod{(i // 40) % 25:02d}" / f"sub{(i // 1000) % 10}"
        d.mkdir(parents=True, exist_ok=True)
        lines = []
        for rep in range(3):
            lines += [l.format(n=i * 3 + rep) for l in BODY]
        if i % every == 0:
            extra = []
            for t in range(per):
                extra.append(f"  // TODO fix the edge case number {t} in handler {i}")
                extra += [f"  const step{t} = {t};"] * (0 if per > 10 else 2)
            lines[3:3] = extra
            todos += per
        (d / f"file{i}.ts").write_text("\n".join(lines) + "\n")
    log(f"{root}: {files} files, {todos} todos")


# -------------------------------------------------------------------- cli


def time_l(cmd: list[str]) -> dict:
    r = subprocess.run(["/usr/bin/time", "-l"] + cmd, capture_output=True, text=True)
    m1 = re.search(r"(\d+)\s+maximum resident set size", r.stderr)
    m2 = re.search(r"(\d+)\s+peak memory footprint", r.stderr)
    m3 = re.search(r"([\d.]+) real", r.stderr)
    return {"max_rss": int(m1.group(1)) if m1 else None,
            "peak_footprint": int(m2.group(1)) if m2 else None,
            "real_s": float(m3.group(1)) if m3 else None}


def cli(args) -> dict:
    root = Path(os.path.expanduser(args.root)).resolve()
    bench = REPO / "target" / "bench"
    bench.mkdir(parents=True, exist_ok=True)
    tags = bench / "tags.json"
    tags.write_text(json.dumps({"tags": TAGS}))
    res: dict = {"scan": {}}
    for name, flags in [] if args.skip_scan else [("default", []), ("hidden", ["--hidden"]),
                        ("wide", ["--hidden", "--no-ignore"])]:
        runs = [time_l([str(BINARY), "scan", "--json", "--config", str(tags)] + flags + [str(root)])
                for _ in range(3)]
        res["scan"][name] = runs
        log(f"scan {name}: " + ", ".join(
            f"rss {mb(r['max_rss'])} footprint {mb(r['peak_footprint'])}" for r in runs))

    # watch: steady state, then a churn burst, on the default set.
    pre = git_status(root)
    files = scan_set(root, False)
    churn = [] if args.no_writes else churn_candidates(root, files, 20)
    restorer = Restorer(churn)
    originals = restorer.originals
    res["churn_files"] = [str(p) for p in churn]
    proc = subprocess.Popen([str(BINARY), "watch", "--config", str(tags), str(root)],
                            stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
    sampler = Sampler(proc.pid)
    sampler.start()
    ph = Phases(proc.pid, sampler)
    lines: list[str] = []
    reader = threading.Thread(target=lambda: lines.extend(iter(proc.stdout.readline, "")), daemon=True)
    try:
        t0 = time.monotonic()
        reader.start()
        while not lines and time.monotonic() - t0 < 60:
            time.sleep(0.01)
        ph.mark("watch ready", ready=lines[0].strip() if lines else None)
        time.sleep(10)
        ph.mark("watch idle 10 s")
        for rnd in range(10 if churn else 0):
            for i, p in enumerate(churn):
                data = originals[p]
                if rnd % 2 == 0:
                    data = data + (b"" if data.endswith(b"\n") else b"\n") + f"// TODO churn {i}\n".encode()
                restorer.write(p, data)
            time.sleep(1.0)
        restorer.restore()
        time.sleep(3)
        ph.mark("watch after 10 churn rounds", lines=len(lines))
        time.sleep(10)
        ph.mark("watch idle 10 s after churn")
    finally:
        restorer.restore()
        sampler.stop.set()
        proc.terminate()
        proc.wait(timeout=10)
    res["watch"] = {"phases": ph.rows, "output_lines": len(lines)}
    res["git_status_unchanged"] = pre == git_status(root)
    log(f"git status unchanged: {res['git_status_unchanged']}")
    return res


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--json", help="write the results as JSON here")
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("session")
    s.add_argument("root")
    s.add_argument("--hidden", action="store_true", help="filtering.includeHiddenFiles")
    s.add_argument("--churn-seconds", type=float, default=30.0)
    s.add_argument("--rescans", type=int, default=5)
    s.add_argument("--binary", help="instrumented server, e.g. target/release/examples/memprofile_lsp")
    s.add_argument("--memlog", default="target/bench/memlog.txt", help="its heap log")
    s.add_argument("--relief-ms", type=int, default=0,
                   help="have it call malloc_zone_pressure_relief this often")
    k = sub.add_parser("soak")
    k.add_argument("root")
    k.add_argument("--hidden", action="store_true")
    k.add_argument("--cycles", type=int, default=100)
    k.add_argument("--every", type=int, default=10, help="sample every N cycles")
    k.add_argument("--no-writes", action="store_true", help="rescans only; write nothing under ROOT")
    k.add_argument("--binary")
    k.add_argument("--memlog", default="target/bench/memlog.txt")
    k.add_argument("--relief-ms", type=int, default=0)
    k.add_argument("--heap-at", type=lambda v: {int(x) for x in v.split(",")}, default=set(),
                   help="cycles at which to save `heap PID` output, e.g. 10,300")
    k.add_argument("--heap-dir", default="target/bench")
    f = sub.add_parser("first-tree")
    f.add_argument("roots", nargs="+")
    f.add_argument("--hidden", action="store_true")
    f.add_argument("--runs", type=int, default=2)
    g = sub.add_parser("generate")
    g.add_argument("dir")
    g.add_argument("kind", choices=sorted(KINDS))
    c = sub.add_parser("cli")
    c.add_argument("root")
    c.add_argument("--no-writes", action="store_true", help="skip the watch churn")
    c.add_argument("--skip-scan", action="store_true", help="measure only watch")
    args = ap.parse_args()
    if not BINARY.exists():
        subprocess.run(["cargo", "build", "--release"], cwd=REPO, check=True)
    if args.cmd == "generate":
        generate(args)
        return 0
    result = {"session": session, "soak": soak, "first-tree": first_tree_only,
              "cli": cli}[args.cmd](args)
    if args.json:
        Path(args.json).write_text(json.dumps(result, indent=1))
    ok = result.get("git_status_unchanged", True) is not False
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
