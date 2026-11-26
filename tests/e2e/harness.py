#!/usr/bin/env python3
"""FrankenMining tests/e2e NDJSON logging harness and CrashPack engine (BD-142, plan §33.5).

Stdlib-only (run with `python3 -I`). Never mocks FrankenMining components: every step executes
a real binary (e.g. `fmine`), validates the JSON envelope and INV-002 flags, emits one strict
NDJSON event per step, captures a self-contained SHA-256-verified CrashPack on failure, replays
CrashPacks deterministically, and renders a human-readable Markdown summary strictly from the
NDJSON log (the log, not the summary, is the evidence).
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
import shutil
import stat
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

LOG_SCHEMA = "fmine.e2e.log.v1"
CRASHPACK_SCHEMA = "fmine.e2e.crashpack.v1"
MAX_CAPTURED_FILE_BYTES = 4 * 1024 * 1024
SKIP_DIR_NAMES = {".git", "target", "corpus_of_input_docs", ".rch-tmp"}
PATH_FLAGS = {
    "--workspace",
    "--input",
    "--model",
    "--manifest",
    "--docs",
    "--definitions",
    "--bindings",
    "--rules",
    "--record",
}
REQUIRED_INV002_FLAGS = {
    "reserve_disclosure_claim": False,
    "execution_authorized": False,
    "geotechnical_signoff": False,
    "regulatory_compliance_claim": False,
}


def utc_now() -> str:
    return (
        datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        while True:
            chunk = fh.read(65536)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def load_json_no_dup(text: str) -> Any:
    def hook(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        obj: dict[str, Any] = {}
        for k, v in pairs:
            if k in obj:
                raise ValueError(f"duplicate JSON key {k!r}")
            obj[k] = v
        return obj

    return json.loads(text, object_pairs_hook=hook)


def canonical_json(obj: Any) -> str:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def append_ndjson(log_path: Path, record: dict[str, Any]) -> None:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    line = json.dumps(record, separators=(",", ":"), ensure_ascii=False)
    with log_path.open("a", encoding="utf-8") as fh:
        fh.write(line + "\n")


def resolve_git_sha(cwd: Path) -> str:
    try:
        out = subprocess.check_output(
            ["git", "rev-parse", "--short=12", "HEAD"],
            cwd=str(cwd),
            stderr=subprocess.DEVNULL,
            text=True,
        ).strip()
        if out:
            return out
    except Exception:
        pass
    return "unknown"


def read_last_anchor(run_dir: Path) -> str | None:
    anchor_file = run_dir / ".last_anchor"
    if not anchor_file.is_file():
        return None
    raw = anchor_file.read_text(encoding="utf-8").strip()
    return raw if raw else None


def write_last_anchor(run_dir: Path, anchor: str | None) -> None:
    anchor_file = run_dir / ".last_anchor"
    if anchor:
        anchor_file.write_text(anchor + "\n", encoding="utf-8")


def parse_and_validate_envelope(
    stdout_text: str,
    actual_rc: int,
    want_exit: int,
    want_status: str,
    want_refusal: str | None,
    want_cert: str | None,
) -> tuple[dict[str, Any], list[str]]:
    problems: list[str] = []
    lines = stdout_text.splitlines()
    if len(lines) != 1:
        problems.append(f"expected 1 stdout JSON line, got {len(lines)}")
    env: dict[str, Any] = {}
    try:
        parsed = load_json_no_dup(stdout_text)
        if not isinstance(parsed, dict):
            problems.append("stdout JSON root is not an object")
        else:
            env = parsed
    except ValueError as exc:
        problems.append(f"invalid JSON envelope: {exc}")

    if env.get("flags") != REQUIRED_INV002_FLAGS:
        problems.append("INV-002 flags mismatch")
    if not isinstance(env.get("warnings"), list):
        problems.append("warnings missing or not a list")
    actions = env.get("next_actions")
    if not (
        isinstance(actions, list)
        and all(isinstance(a, dict) and set(a.keys()) == {"command", "why"} for a in actions)
    ):
        problems.append("next_actions malformed")

    if actual_rc != want_exit:
        problems.append(f"process exit_code {actual_rc} != expected {want_exit}")
    if env.get("exit_code") != actual_rc:
        problems.append(
            f"envelope exit_code {env.get('exit_code')} != process exit_code {actual_rc}"
        )
    if env.get("status") != want_status:
        problems.append(f"status {env.get('status')!r} != expected {want_status!r}")

    refusal = env.get("refusal")
    actual_refusal_code = refusal.get("code") if isinstance(refusal, dict) else None
    if actual_refusal_code != want_refusal:
        problems.append(
            f"refusal code {actual_refusal_code!r} != expected {want_refusal!r}"
        )
    if isinstance(refusal, dict) and refusal.get("exit_code") != actual_rc:
        problems.append(
            f"refusal.exit_code {refusal.get('exit_code')} != process exit_code {actual_rc}"
        )

    has_cert = isinstance(env.get("certificate"), str) and bool(env.get("certificate"))
    if want_cert == "1" and not has_cert:
        problems.append("expected non-empty certificate digest, got none")
    elif want_cert == "0" and has_cert:
        problems.append("expected no certificate digest, got one")

    return env, problems


def snapshot_path_arg(src: Path, dst_root: Path, slot_name: str) -> tuple[str, list[dict[str, Any]]]:
    captured: list[dict[str, Any]] = []
    if src.is_file():
        if src.stat().st_size <= MAX_CAPTURED_FILE_BYTES:
            rel = f"inputs/{slot_name}/{src.name}"
            dst = dst_root / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)
            captured.append(
                {
                    "rel_path": rel,
                    "bytes": dst.stat().st_size,
                    "sha256": sha256_file(dst),
                }
            )
            return rel, captured
        return "", captured

    if src.is_dir():
        base_rel = f"inputs/{slot_name}"
        base_dst = dst_root / base_rel
        base_dst.mkdir(parents=True, exist_ok=True)
        for cur_root, dirnames, filenames in os.walk(src):
            dirnames[:] = sorted(d for d in dirnames if d not in SKIP_DIR_NAMES)
            for fname in sorted(filenames):
                fpath = Path(cur_root) / fname
                if fpath.is_symlink() or not fpath.is_file():
                    continue
                if fpath.stat().st_size > MAX_CAPTURED_FILE_BYTES:
                    continue
                sub_rel = fpath.relative_to(src).as_posix()
                rel = f"{base_rel}/{sub_rel}"
                dst = dst_root / rel
                dst.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(fpath, dst)
                captured.append(
                    {
                        "rel_path": rel,
                        "bytes": dst.stat().st_size,
                        "sha256": sha256_file(dst),
                    }
                )
        return base_rel, captured
    return "", captured


def normalize_output(data: bytes, path_rewrites: list[dict[str, str]]) -> bytes:
    text = data.decode("utf-8", errors="replace")
    for rw in sorted(path_rewrites, key=lambda r: len(r["original"]), reverse=True):
        orig = rw["original"]
        if orig:
            text = text.replace(orig, f"<CRASHPACK:{rw['slot']}>")
    return text.encode("utf-8")


def compute_pack_digest(manifest_without_digest: dict[str, Any]) -> str:
    return sha256_bytes(canonical_json(manifest_without_digest).encode("utf-8"))


def write_crashpack(
    run_dir: Path,
    log_path: Path,
    suite: str,
    step: str,
    git_sha: str,
    binary: Path,
    fmine_args: list[str],
    expected: dict[str, Any],
    actual: dict[str, Any],
    problems: list[str],
    stdout_bytes: bytes,
    stderr_bytes: bytes,
) -> Path:
    pack_id = sha256_bytes(
        f"{suite}\0{step}\0{git_sha}\0{actual['exit_code']}".encode("utf-8") + stdout_bytes
    )[:12]
    safe_suite = "".join(c if c.isalnum() or c in "-_" else "_" for c in suite)
    safe_step = "".join(c if c.isalnum() or c in "-_" else "_" for c in step)
    pack_dir = run_dir / "crashpacks" / f"{safe_suite}-{safe_step}-{pack_id}"
    if pack_dir.exists():
        shutil.rmtree(pack_dir)
    pack_dir.mkdir(parents=True, exist_ok=True)

    (pack_dir / "stdout.txt").write_bytes(stdout_bytes)
    (pack_dir / "stderr.txt").write_bytes(stderr_bytes)
    prior_events = log_path.read_bytes() if log_path.is_file() else b""
    (pack_dir / "events_before_failure.ndjson").write_bytes(prior_events)

    files_inventory: list[dict[str, Any]] = [
        {
            "rel_path": "stdout.txt",
            "bytes": len(stdout_bytes),
            "sha256": sha256_bytes(stdout_bytes),
        },
        {
            "rel_path": "stderr.txt",
            "bytes": len(stderr_bytes),
            "sha256": sha256_bytes(stderr_bytes),
        },
        {
            "rel_path": "events_before_failure.ndjson",
            "bytes": len(prior_events),
            "sha256": sha256_bytes(prior_events),
        },
    ]

    # Snapshot any referenced workspace/input path flags (both `--flag val` and `--flag=val`)
    # so the CrashPack is completely self-contained.
    path_rewrites: list[dict[str, str]] = []
    replay_args = list(fmine_args)
    slot_idx = 0
    i = 0
    while i < len(fmine_args):
        tok = fmine_args[i]
        eq_flag = None
        raw_val = None
        val_arg_idx = None
        if tok in PATH_FLAGS and i + 1 < len(fmine_args):
            eq_flag = None
            raw_val = fmine_args[i + 1]
            val_arg_idx = i + 1
            step_inc = 2
            flag_name = tok
        elif "=" in tok and tok.split("=", 1)[0] in PATH_FLAGS:
            flag_name, raw_val = tok.split("=", 1)
            eq_flag = flag_name
            val_arg_idx = i
            step_inc = 1
        else:
            step_inc = 1
            flag_name = ""

        if raw_val is not None and val_arg_idx is not None:
            candidate = Path(raw_val)
            if candidate.exists():
                slot_name = f"arg{slot_idx}_{flag_name.lstrip('-').replace('-', '_')}"
                slot_idx += 1
                rel_target, captured = snapshot_path_arg(candidate, pack_dir, slot_name)
                if rel_target:
                    files_inventory.extend(captured)
                    path_rewrites.append(
                        {
                            "flag": flag_name,
                            "arg_index": str(val_arg_idx),
                            "slot": slot_name,
                            "original": raw_val,
                            "captured_rel": rel_target,
                        }
                    )
                    if eq_flag is not None:
                        replay_args[val_arg_idx] = f"{eq_flag}=@CRASHPACK/{rel_target}"
                    else:
                        replay_args[val_arg_idx] = f"@CRASHPACK/{rel_target}"
        i += step_inc

    files_inventory.sort(key=lambda item: item["rel_path"])
    norm_stdout_sha = sha256_bytes(normalize_output(stdout_bytes, path_rewrites))

    manifest_core: dict[str, Any] = {
        "schema": CRASHPACK_SCHEMA,
        "suite": suite,
        "step": step,
        "created_utc": utc_now(),
        "git_sha": git_sha,
        "binary_path": str(binary),
        "binary_sha256": sha256_file(binary) if binary.is_file() else None,
        "command": [str(binary), *fmine_args],
        "replay_args": replay_args,
        "path_rewrites": path_rewrites,
        "expected": expected,
        "actual": {
            **actual,
            "stdout_sha256": sha256_bytes(stdout_bytes),
            "normalized_stdout_sha256": norm_stdout_sha,
            "stderr_sha256": sha256_bytes(stderr_bytes),
        },
        "problems": problems,
        "files": files_inventory,
    }
    pack_digest = compute_pack_digest(manifest_core)
    manifest = {**manifest_core, "pack_digest": pack_digest}
    (pack_dir / "manifest.json").write_text(
        json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    harness_path = Path(__file__).resolve()
    replay_sh = pack_dir / "replay.sh"
    replay_sh.write_text(
        "#!/usr/bin/env bash\n"
        "set -euo pipefail\n"
        'pack_dir="$(cd "$(dirname "$0")" && pwd)"\n'
        'harness="${E2E_HARNESS_PY:-}"\n'
        f'if [ -z "$harness" ] && [ -f "{harness_path}" ]; then harness="{harness_path}"; fi\n'
        'if [ -z "$harness" ] && [ -f "./tests/e2e/harness.py" ]; then harness="./tests/e2e/harness.py"; fi\n'
        'if [ -z "$harness" ]; then\n'
        '  repo_root="$(git rev-parse --show-toplevel 2>/dev/null || true)"\n'
        '  if [ -n "$repo_root" ] && [ -f "$repo_root/tests/e2e/harness.py" ]; then harness="$repo_root/tests/e2e/harness.py"; fi\n'
        'fi\n'
        'if [ -z "$harness" ]; then echo "CrashPack replay error: cannot locate tests/e2e/harness.py (set E2E_HARNESS_PY)" >&2; exit 2; fi\n'
        'exec python3 -I "$harness" replay "$pack_dir" "$@"\n',
        encoding="utf-8",
    )
    replay_sh.chmod(replay_sh.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return pack_dir


def cmd_run_step(ns: argparse.Namespace) -> int:
    run_dir = Path(ns.run_dir)
    run_dir.mkdir(parents=True, exist_ok=True)
    log_path = run_dir / "events.ndjson"
    binary = Path(ns.bin)
    if not binary.is_file():
        print(f"e2e harness error: binary not found: {binary}", file=sys.stderr)
        return 2

    fmine_args = list(ns.cmd)
    if fmine_args and fmine_args[0] == "--":
        fmine_args = fmine_args[1:]

    want_refusal = None if ns.want_refusal in (None, "", "-") else ns.want_refusal
    git_sha = ns.git_sha or resolve_git_sha(Path.cwd())
    anchor_before = read_last_anchor(run_dir)

    t0 = time.monotonic()
    proc = subprocess.run(
        [str(binary), *fmine_args],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    timing_ms = round((time.monotonic() - t0) * 1000.0, 3)

    stdout_bytes = proc.stdout
    stderr_bytes = proc.stderr
    stdout_text = stdout_bytes.decode("utf-8", errors="replace")
    stdout_digest = sha256_bytes(stdout_bytes)

    env, problems = parse_and_validate_envelope(
        stdout_text=stdout_text,
        actual_rc=proc.returncode,
        want_exit=ns.want_exit,
        want_status=ns.want_status,
        want_refusal=want_refusal,
        want_cert=ns.want_cert,
    )
    if ns.assert_stdout_sha256 and stdout_digest != ns.assert_stdout_sha256:
        problems.append(
            f"stdout sha256 {stdout_digest} != expected {ns.assert_stdout_sha256}"
        )

    anchor_after = env.get("anchor_id") if isinstance(env.get("anchor_id"), str) else None
    if anchor_after:
        write_last_anchor(run_dir, anchor_after)

    refusal_obj = env.get("refusal")
    refusals = [refusal_obj] if isinstance(refusal_obj, dict) else []
    refusal_code = refusal_obj.get("code") if isinstance(refusal_obj, dict) else None
    cert_val = env.get("certificate")
    cert_digests = [cert_val] if isinstance(cert_val, str) and cert_val else []

    ok = len(problems) == 0
    crashpack_path: str | None = None
    if not ok:
        expected = {
            "exit_code": ns.want_exit,
            "status": ns.want_status,
            "refusal_code": want_refusal,
            "want_cert": ns.want_cert,
            "assert_stdout_sha256": ns.assert_stdout_sha256,
        }
        actual = {
            "exit_code": proc.returncode,
            "status": env.get("status"),
            "refusal_code": refusal_code,
            "anchor_before": anchor_before,
            "anchor_after": anchor_after,
            "certificate_digests": cert_digests,
        }
        pack_dir = write_crashpack(
            run_dir=run_dir,
            log_path=log_path,
            suite=ns.suite,
            step=ns.step,
            git_sha=git_sha,
            binary=binary,
            fmine_args=fmine_args,
            expected=expected,
            actual=actual,
            problems=problems,
            stdout_bytes=stdout_bytes,
            stderr_bytes=stderr_bytes,
        )
        crashpack_path = str(pack_dir)

    record = {
        "event": "e2e_step",
        "schema": LOG_SCHEMA,
        "suite": ns.suite,
        "step": ns.step,
        "ts_utc": utc_now(),
        "git_sha": git_sha,
        "command": [str(binary), *fmine_args],
        "anchor_before": anchor_before,
        "anchor_after": anchor_after,
        "anchor": anchor_after,
        "exit_code": proc.returncode,
        "expected_exit_code": ns.want_exit,
        "status": env.get("status"),
        "refusals": refusals,
        "refusal_code": refusal_code,
        "certificate_digests": cert_digests,
        "digest": stdout_digest,
        "timing_ms": timing_ms,
        "file": ns.file,
        "instance": ns.instance,
        "state": ns.state,
        "ok": ok,
        "problems": problems,
        "crashpack": crashpack_path,
    }
    append_ndjson(log_path, record)

    if ns.digest_out:
        Path(ns.digest_out).write_text(stdout_digest + "\n", encoding="utf-8")

    return 0 if ok else 1


def cmd_log_event(ns: argparse.Namespace) -> int:
    run_dir = Path(ns.run_dir)
    run_dir.mkdir(parents=True, exist_ok=True)
    log_path = run_dir / "events.ndjson"
    try:
        obj = load_json_no_dup(ns.json_line)
    except ValueError as exc:
        print(f"e2e log-event invalid JSON: {exc}", file=sys.stderr)
        return 2
    if not isinstance(obj, dict):
        print("e2e log-event requires a JSON object", file=sys.stderr)
        return 2
    obj.setdefault("schema", LOG_SCHEMA)
    obj.setdefault("suite", ns.suite)
    obj.setdefault("git_sha", ns.git_sha or resolve_git_sha(Path.cwd()))
    obj.setdefault("ts_utc", utc_now())
    append_ndjson(log_path, obj)
    return 0


def render_summary_markdown(log_path: Path) -> tuple[str, dict[str, Any]]:
    if not log_path.is_file():
        raise ValueError(f"NDJSON log not found: {log_path}")
    events: list[dict[str, Any]] = []
    for lineno, raw in enumerate(log_path.read_text(encoding="utf-8").splitlines(), start=1):
        if not raw.strip():
            continue
        parsed = load_json_no_dup(raw)
        if not isinstance(parsed, dict):
            raise ValueError(f"{log_path}:{lineno}: NDJSON line is not a JSON object")
        events.append(parsed)

    step_events = [e for e in events if e.get("event") == "e2e_step"]
    suite = (
        step_events[0].get("suite")
        if step_events
        else (events[0].get("suite") if events else "unknown")
    )
    git_sha = (
        step_events[0].get("git_sha")
        if step_events
        else (events[0].get("git_sha") if events else "unknown")
    )
    passed = sum(1 for e in step_events if e.get("ok") is True)
    failed = sum(1 for e in step_events if e.get("ok") is not True)
    crashpacks = [e["crashpack"] for e in step_events if e.get("crashpack")]
    anchors = [e["anchor_after"] for e in step_events if e.get("anchor_after")]
    anchor_final = anchors[-1] if anchors else None
    total_ms = round(sum(float(e.get("timing_ms", 0.0)) for e in step_events), 3)

    lines = [
        f"# E2E Run Summary — `{suite}`",
        "",
        "> Rendered from NDJSON log; the NDJSON log, not this summary, is the evidence (plan §33.5).",
        "",
        f"- **Log file:** `{log_path}`",
        f"- **Git SHA:** `{git_sha}`",
        f"- **Steps:** `{len(step_events)}` (`{passed}` passed, `{failed}` failed)",
        f"- **Final anchor:** `{anchor_final or 'none'}`",
        f"- **Total step wall time:** `{total_ms:.3f} ms`",
        f"- **CrashPacks captured:** `{len(crashpacks)}`",
        "",
        "| Step | Status | Exit | Refusal | Anchor | Certs | Timing (ms) | OK |",
        "|---|---|---:|---|---|---:|---:|---|",
    ]
    for e in step_events:
        lines.append(
            f"| `{e.get('step')}` | `{e.get('status')}` | {e.get('exit_code')} "
            f"| `{e.get('refusal_code') or '-'}` | `{e.get('anchor_after') or '-'}` "
            f"| {len(e.get('certificate_digests') or [])} | {float(e.get('timing_ms', 0.0)):.3f} "
            f"| `{e.get('ok')}` |"
        )
    if crashpacks:
        lines.extend(["", "## CrashPacks", ""])
        for cp in crashpacks:
            lines.append(f"- `{cp}`")

    meta = {
        "suite": suite,
        "git_sha": git_sha,
        "steps": len(step_events),
        "passed": passed,
        "failed": failed,
        "anchor_final": anchor_final,
        "crashpacks": crashpacks,
        "total_timing_ms": total_ms,
        "ok": failed == 0,
    }
    return "\n".join(lines) + "\n", meta


def cmd_summarize(ns: argparse.Namespace) -> int:
    log_path = Path(ns.log_path)
    try:
        md, _ = render_summary_markdown(log_path)
    except ValueError as exc:
        print(f"e2e summarize error: {exc}", file=sys.stderr)
        return 2
    if ns.out:
        Path(ns.out).write_text(md, encoding="utf-8")
    else:
        sys.stdout.write(md)
    return 0


def cmd_finish(ns: argparse.Namespace) -> int:
    run_dir = Path(ns.run_dir)
    log_path = run_dir / "events.ndjson"
    summary_path = run_dir / "summary.md"
    try:
        md, meta = render_summary_markdown(log_path)
    except ValueError as exc:
        print(f"e2e finish error: {exc}", file=sys.stderr)
        return 2
    summary_event = {
        "event": "e2e_summary",
        "schema": LOG_SCHEMA,
        "ts_utc": utc_now(),
        **meta,
    }
    append_ndjson(log_path, summary_event)
    summary_path.write_text(md, encoding="utf-8")
    print(json.dumps(summary_event, separators=(",", ":"), ensure_ascii=False))
    return 0 if meta["ok"] else 1


def cmd_replay(ns: argparse.Namespace) -> int:
    pack_dir = Path(ns.crashpack_dir).resolve()
    manifest_path = pack_dir / "manifest.json"
    if not manifest_path.is_file():
        print(f"CrashPack corrupt: missing {manifest_path}", file=sys.stderr)
        return 2
    try:
        manifest = load_json_no_dup(manifest_path.read_text(encoding="utf-8"))
    except ValueError as exc:
        print(f"CrashPack corrupt: malformed manifest.json: {exc}", file=sys.stderr)
        return 2
    if not isinstance(manifest, dict) or manifest.get("schema") != CRASHPACK_SCHEMA:
        print("CrashPack corrupt: invalid schema", file=sys.stderr)
        return 2

    recorded_digest = manifest.get("pack_digest")
    manifest_core = {k: v for k, v in manifest.items() if k != "pack_digest"}
    computed_digest = compute_pack_digest(manifest_core)
    if recorded_digest != computed_digest:
        print(
            f"CrashPack tamper detected: manifest pack_digest {recorded_digest} != computed {computed_digest}",
            file=sys.stderr,
        )
        return 2

    # Verify every recorded file's safe relative path, exact byte size, and SHA-256,
    # and refuse any unexpected or symlinked files injected into the CrashPack directory.
    expected_rels: set[str] = set()
    for entry in manifest.get("files", []):
        rel = entry.get("rel_path")
        if not isinstance(rel, str) or not rel:
            print("CrashPack corrupt: invalid file inventory entry", file=sys.stderr)
            return 2
        rel_obj = Path(rel)
        if rel_obj.is_absolute() or ".." in rel_obj.parts or rel in expected_rels:
            print(f"CrashPack tamper detected: invalid or duplicate rel_path {rel!r}", file=sys.stderr)
            return 2
        expected_rels.add(rel)
        fpath = pack_dir / rel_obj
        if fpath.is_symlink() or not fpath.is_file():
            print(f"CrashPack corrupt: missing or non-regular file {rel}", file=sys.stderr)
            return 2
        actual_sha = sha256_file(fpath)
        if actual_sha != entry["sha256"] or fpath.stat().st_size != entry["bytes"]:
            print(
                f"CrashPack tamper detected on {rel}: expected sha256 {entry['sha256']} ({entry['bytes']}B), "
                f"got {actual_sha} ({fpath.stat().st_size}B)",
                file=sys.stderr,
            )
            return 2

    allowed_root_files = {"manifest.json", "replay.sh"}
    for cur_root, dirnames, filenames in os.walk(pack_dir):
        for dname in list(dirnames):
            dpath = Path(cur_root) / dname
            if dpath.is_symlink():
                print(
                    f"CrashPack tamper detected: unexpected symlink directory {dpath.relative_to(pack_dir).as_posix()}",
                    file=sys.stderr,
                )
                return 2
        for fname in filenames:
            fpath = Path(cur_root) / fname
            rel_disk = fpath.relative_to(pack_dir).as_posix()
            if fpath.is_symlink():
                print(f"CrashPack tamper detected: unexpected symlink {rel_disk}", file=sys.stderr)
                return 2
            if rel_disk in allowed_root_files:
                continue
            if rel_disk not in expected_rels:
                print(
                    f"CrashPack tamper detected: unexpected file {rel_disk} not in manifest inventory",
                    file=sys.stderr,
                )
                return 2

    if ns.bin:
        binary = Path(ns.bin)
    elif Path(manifest["binary_path"]).is_file():
        binary = Path(manifest["binary_path"])
    elif os.environ.get("FMINE_BIN"):
        binary = Path(os.environ["FMINE_BIN"])
    else:
        binary = Path(manifest["binary_path"])
    if not binary.is_file():
        print(f"CrashPack replay error: binary not found: {binary}", file=sys.stderr)
        return 2

    # Materialize replay args pointing to the CrashPack's self-contained captured inputs.
    replay_args: list[str] = []
    replay_rewrites: list[dict[str, str]] = []
    rw_by_rel = {rw["captured_rel"]: rw for rw in manifest.get("path_rewrites", [])}
    for tok in manifest.get("replay_args", []):
        if isinstance(tok, str) and tok.startswith("@CRASHPACK/"):
            rel = tok[len("@CRASHPACK/") :]
            target_path = str(pack_dir / rel)
            replay_args.append(target_path)
            if rel in rw_by_rel:
                replay_rewrites.append(
                    {"slot": rw_by_rel[rel]["slot"], "original": target_path}
                )
        elif isinstance(tok, str) and "=@CRASHPACK/" in tok:
            prefix, rel = tok.split("=@CRASHPACK/", 1)
            target_path = str(pack_dir / rel)
            replay_args.append(f"{prefix}={target_path}")
            if rel in rw_by_rel:
                replay_rewrites.append(
                    {"slot": rw_by_rel[rel]["slot"], "original": target_path}
                )
        else:
            replay_args.append(str(tok))

    proc = subprocess.run(
        [str(binary), *replay_args],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    actual_rec = manifest["actual"]
    replayed_stdout_text = proc.stdout.decode("utf-8", errors="replace")
    try:
        replayed_env = load_json_no_dup(replayed_stdout_text)
    except ValueError:
        replayed_env = {}
    replayed_status = replayed_env.get("status") if isinstance(replayed_env, dict) else None
    replayed_ref = (
        replayed_env.get("refusal", {}).get("code")
        if isinstance(replayed_env, dict) and isinstance(replayed_env.get("refusal"), dict)
        else None
    )
    replayed_norm_sha = sha256_bytes(normalize_output(proc.stdout, replay_rewrites))

    mismatches: list[str] = []
    if proc.returncode != actual_rec["exit_code"]:
        mismatches.append(
            f"exit_code {proc.returncode} != recorded {actual_rec['exit_code']}"
        )
    if replayed_status != actual_rec.get("status"):
        mismatches.append(
            f"status {replayed_status!r} != recorded {actual_rec.get('status')!r}"
        )
    if replayed_ref != actual_rec.get("refusal_code"):
        mismatches.append(
            f"refusal_code {replayed_ref!r} != recorded {actual_rec.get('refusal_code')!r}"
        )
    if replayed_norm_sha != actual_rec["normalized_stdout_sha256"]:
        mismatches.append(
            f"normalized_stdout_sha256 {replayed_norm_sha} != recorded {actual_rec['normalized_stdout_sha256']}"
        )
    if not manifest.get("path_rewrites") and sha256_bytes(proc.stdout) != actual_rec["stdout_sha256"]:
        mismatches.append(
            f"raw stdout_sha256 {sha256_bytes(proc.stdout)} != recorded {actual_rec['stdout_sha256']}"
        )

    report = {
        "event": "crashpack_replay",
        "schema": CRASHPACK_SCHEMA,
        "pack_dir": str(pack_dir),
        "pack_digest": computed_digest,
        "reproduced": len(mismatches) == 0,
        "exit_code": proc.returncode,
        "status": replayed_status,
        "refusal_code": replayed_ref,
        "normalized_stdout_sha256": replayed_norm_sha,
        "mismatches": mismatches,
    }
    print(json.dumps(report, separators=(",", ":"), ensure_ascii=False))
    return 0 if not mismatches else 1


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(description="FrankenMining E2E NDJSON & CrashPack harness")
    sub = p.add_subparsers(dest="subcmd", required=True)

    r = sub.add_parser("run-step")
    r.add_argument("--run-dir", required=True)
    r.add_argument("--suite", required=True)
    r.add_argument("--step", required=True)
    r.add_argument("--bin", required=True)
    r.add_argument("--want-exit", type=int, required=True)
    r.add_argument("--want-status", required=True)
    r.add_argument("--want-refusal", default="-")
    r.add_argument("--want-cert", default=None)
    r.add_argument("--assert-stdout-sha256", default=None)
    r.add_argument("--digest-out", default=None)
    r.add_argument("--git-sha", default=None)
    r.add_argument("--file", default=None)
    r.add_argument("--instance", default=None)
    r.add_argument("--state", default=None)
    r.add_argument("cmd", nargs=argparse.REMAINDER)

    le = sub.add_parser("log-event")
    le.add_argument("--run-dir", required=True)
    le.add_argument("--suite", required=True)
    le.add_argument("--git-sha", default=None)
    le.add_argument("--json", dest="json_line", required=True)

    f = sub.add_parser("finish")
    f.add_argument("--run-dir", required=True)

    s = sub.add_parser("summarize")
    s.add_argument("log_path")
    s.add_argument("--out", default=None)

    rp = sub.add_parser("replay")
    rp.add_argument("crashpack_dir")
    rp.add_argument("--bin", default=None)

    return p


def main(argv: list[str]) -> int:
    ns = build_parser().parse_args(argv)
    if ns.subcmd == "run-step":
        return cmd_run_step(ns)
    if ns.subcmd == "log-event":
        return cmd_log_event(ns)
    if ns.subcmd == "finish":
        return cmd_finish(ns)
    if ns.subcmd == "summarize":
        return cmd_summarize(ns)
    if ns.subcmd == "replay":
        return cmd_replay(ns)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
