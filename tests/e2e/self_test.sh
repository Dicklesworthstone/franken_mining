#!/usr/bin/env bash
# BD-142 [WP-07] Harness self-test: proves tests/e2e/ runs against the real `fmine` binary (zero mocks),
# emits strict §33.5 NDJSON logs with anchor chaining and timings, renders summary.md strictly from the
# NDJSON log, captures a self-contained SHA-256-verified CrashPack on failure, replays that CrashPack
# deterministically after the original temp workspace is deleted, and refuses tampered CrashPacks.
set -euo pipefail

SELF_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=tests/e2e/lib.sh
source "$SELF_DIR/lib.sh"

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/fmine-bd142-selftest.XXXXXX")"
cleanup() {
    if [ "${E2E_KEEP_TMP:-0}" != "1" ]; then
        rm -rf "$SCRATCH"
    fi
}
trap cleanup EXIT

# ------------------------------------------------------------------------------
# 1. Passing multi-step suite against the real `fmine` binary
# ------------------------------------------------------------------------------
PASS_RUN="$SCRATCH/pass_run"
e2e_init "bd142_self_test_pass" "$PASS_RUN"

e2e_step "bare_triage" 0 ok - --
e2e_step "doctor" 0 ok - -- doctor
e2e_step "robot_docs" 0 ok - -- robot-docs
e2e_step "assure_synthetic_1" 0 ok - --want-cert 1 -- assure public --synthetic
SYNTH_DIGEST_1="$E2E_LAST_DIGEST"
e2e_step "assure_synthetic_2_rerun" 0 ok - --want-cert 1 --assert-stdout-sha256 "$SYNTH_DIGEST_1" -- assure public --synthetic
e2e_step "claim_missing_refusal" 3 refused FMINE-REF-201 --want-cert 0 -- claim show clm_0001

# Salted review workspace refusal (BD-357.S2) exercised through the real binary.
SALTED_WS="$SCRATCH/salted_ws"
mkdir -p "$SALTED_WS"
printf 'packet = "salted"\nround_id = "self-test"\nitem_ids = ["item-0001"]\n' >"$SALTED_WS/review-packet.toml"
e2e_step "salted_workspace_refusal" 7 refused FMINE-REF-301 --want-cert 0 \
    --file "review-packet.toml" --instance "self-test" --state "refused" \
    -- assure public --issuer barrick --workspace "$SALTED_WS"

# Custom bd-074-compatible structured event appended through the harness.
e2e_log_event '{"event":"file_check","file":"synthetic.xlsx","instance":"AISC-001","state":"(i)","digest":"'"$SYNTH_DIGEST_1"'","anchor":"anc_self_test"}'

SUMMARY_JSON="$(e2e_finish)"

# ------------------------------------------------------------------------------
# 2. Strict NDJSON schema, anchor-chaining, and summary.md verification
# ------------------------------------------------------------------------------
python3 -I - "$PASS_RUN/events.ndjson" "$PASS_RUN/summary.md" "$SUMMARY_JSON" <<'PY'
import json
import pathlib
import sys

log_path = pathlib.Path(sys.argv[1])
summary_path = pathlib.Path(sys.argv[2])
summary_stdout = json.loads(sys.argv[3])

def no_dup(pairs):
    d = {}
    for k, v in pairs:
        if k in d:
            raise AssertionError(f"duplicate key {k!r}")
        d[k] = v
    return d

rows = [json.loads(line, object_pairs_hook=no_dup) for line in log_path.read_text(encoding="utf-8").splitlines()]
step_rows = [r for r in rows if r.get("event") == "e2e_step"]
custom_rows = [r for r in rows if r.get("event") == "file_check"]
summary_rows = [r for r in rows if r.get("event") == "e2e_summary"]

assert len(step_rows) == 7, f"expected 7 step rows, got {len(step_rows)}"
assert len(custom_rows) == 1, f"expected 1 custom event row, got {len(custom_rows)}"
assert len(summary_rows) == 1, f"expected 1 summary row, got {len(summary_rows)}"
assert summary_rows[0] == summary_stdout
assert summary_rows[0]["ok"] is True
assert summary_rows[0]["steps"] == 7
assert summary_rows[0]["passed"] == 7
assert summary_rows[0]["failed"] == 0
assert summary_rows[0]["crashpacks"] == []

required_step_keys = {
    "event", "schema", "suite", "step", "ts_utc", "git_sha", "command",
    "anchor_before", "anchor_after", "anchor", "exit_code", "expected_exit_code",
    "status", "refusals", "refusal_code", "certificate_digests", "digest",
    "timing_ms", "file", "instance", "state", "ok", "problems", "crashpack",
}
for r in step_rows:
    missing = required_step_keys - set(r.keys())
    assert not missing, f"step {r.get('step')} missing keys {missing}"
    assert r["ok"] is True, f"step {r.get('step')} unexpectedly failed: {r}"
    assert isinstance(r["timing_ms"], (int, float)) and r["timing_ms"] >= 0.0
    assert len(r["digest"]) == 64

# Anchor chaining: synthetic_1 produces a real anchor, which becomes anchor_before of synthetic_2_rerun.
s1 = next(r for r in step_rows if r["step"] == "assure_synthetic_1")
s2 = next(r for r in step_rows if r["step"] == "assure_synthetic_2_rerun")
assert s1["anchor_after"] and s1["anchor_after"].startswith("anc_")
assert s2["anchor_before"] == s1["anchor_after"]
assert s2["anchor_after"] == s1["anchor_after"]
assert s2["certificate_digests"] == s1["certificate_digests"] and len(s1["certificate_digests"]) == 1
assert s2["digest"] == s1["digest"]

# Salted step carried explicit file/instance/state fields and structured refusal object.
salted = next(r for r in step_rows if r["step"] == "salted_workspace_refusal")
assert salted["file"] == "review-packet.toml"
assert salted["instance"] == "self-test"
assert salted["state"] == "refused"
assert salted["refusal_code"] == "FMINE-REF-301"
assert len(salted["refusals"]) == 1 and salted["refusals"][0]["code"] == "FMINE-REF-301"

# Summary rendered strictly from NDJSON log carries §33.5 disclaimer and all step names.
md = summary_path.read_text(encoding="utf-8")
assert "the NDJSON log, not this summary, is the evidence" in md
for r in step_rows:
    assert f"`{r['step']}`" in md
PY

# ------------------------------------------------------------------------------
# 3. Planted failure -> CrashPack capture -> workspace deletion -> Replay -> Tamper refusal
# ------------------------------------------------------------------------------
FAIL_RUN="$SCRATCH/fail_run"
e2e_init "bd142_self_test_crashpack" "$FAIL_RUN"

MISSING_CORPUS_WS="$SCRATCH/ephemeral_missing_corpus_ws"
mkdir -p "$MISSING_CORPUS_WS/registries/sources"
cp "$E2E_REPO_ROOT/registries/sources/barrick.toml" "$MISSING_CORPUS_WS/registries/sources/barrick.toml"

# Deliberately assert exit 0 / ok on a workspace missing its workbooks (real binary exits 5 / FMINE-REF-506).
if e2e_step "planted_missing_corpus_failure" 0 ok - -- assure public --issuer barrick --workspace "$MISSING_CORPUS_WS"; then
    echo "FAIL: planted failing step unexpectedly succeeded" >&2
    exit 1
fi

if e2e_finish >/dev/null; then
    echo "FAIL: e2e_finish unexpectedly returned 0 on failed run" >&2
    exit 1
fi

# Extract CrashPack path from the NDJSON log and delete the original temp workspace completely.
CRASHPACK_DIR="$(python3 -I - "$FAIL_RUN/events.ndjson" <<'PY'
import json, pathlib, sys
rows = [json.loads(l) for l in pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()]
step = next(r for r in rows if r.get("event") == "e2e_step")
assert step["ok"] is False
assert step["exit_code"] == 5
assert step["refusal_code"] == "FMINE-REF-506"
assert step["crashpack"], "CrashPack path missing on failed step"
print(step["crashpack"])
PY
)"
rm -rf "$MISSING_CORPUS_WS"

# Verify self-contained replay via both harness CLI and CrashPack's executable replay.sh.
REPLAY_JSON="$(e2e_replay_crashpack "$CRASHPACK_DIR")"
REPLAY_SH_JSON="$(bash "$CRASHPACK_DIR/replay.sh" --bin "$E2E_FMINE_BIN")"
python3 -I - "$REPLAY_JSON" "$REPLAY_SH_JSON" "$CRASHPACK_DIR" <<'PY'
import json, pathlib, sys
r1 = json.loads(sys.argv[1])
r2 = json.loads(sys.argv[2])
cp = pathlib.Path(sys.argv[3])
for r in (r1, r2):
    assert r["event"] == "crashpack_replay"
    assert r["reproduced"] is True, f"CrashPack failed to reproduce: {r}"
    assert r["exit_code"] == 5
    assert r["status"] == "refused"
    assert r["refusal_code"] == "FMINE-REF-506"
    assert r["mismatches"] == []
assert r1["normalized_stdout_sha256"] == r2["normalized_stdout_sha256"]
assert (cp / "inputs/arg0_workspace/registries/sources/barrick.toml").is_file()
PY

# Negative tamper controls: 1-byte mutation in stdout.txt and in manifest.json MUST fail replay.
TAMPER_STDOUT="$SCRATCH/tamper_stdout"
cp -R "$CRASHPACK_DIR" "$TAMPER_STDOUT"
printf 'X' >>"$TAMPER_STDOUT/stdout.txt"
if e2e_replay_crashpack "$TAMPER_STDOUT" >/dev/null 2>"$SCRATCH/tamper_stdout.err"; then
    echo "FAIL: tampered stdout.txt was not rejected by CrashPack replay" >&2
    exit 1
fi
grep -q "CrashPack tamper detected" "$SCRATCH/tamper_stdout.err"

TAMPER_MANIFEST="$SCRATCH/tamper_manifest"
cp -R "$CRASHPACK_DIR" "$TAMPER_MANIFEST"
python3 -I - "$TAMPER_MANIFEST/manifest.json" <<'PY'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
m = json.loads(p.read_text(encoding="utf-8"))
m["step"] = "forged_step"
p.write_text(json.dumps(m, indent=2), encoding="utf-8")
PY
if e2e_replay_crashpack "$TAMPER_MANIFEST" >/dev/null 2>"$SCRATCH/tamper_manifest.err"; then
    echo "FAIL: tampered manifest.json was not rejected by CrashPack replay" >&2
    exit 1
fi
grep -q "CrashPack tamper detected" "$SCRATCH/tamper_manifest.err"

TAMPER_INJECT="$SCRATCH/tamper_inject"
cp -R "$CRASHPACK_DIR" "$TAMPER_INJECT"
printf 'packet = "salted"\n' >"$TAMPER_INJECT/inputs/arg0_workspace/review-packet.toml"
if e2e_replay_crashpack "$TAMPER_INJECT" >/dev/null 2>"$SCRATCH/tamper_inject.err"; then
    echo "FAIL: injected untracked file in CrashPack inputs/ was not rejected by replay" >&2
    exit 1
fi
grep -q "CrashPack tamper detected" "$SCRATCH/tamper_inject.err"

printf '{"event":"bd142_self_test","ok":true,"pass_steps":7,"crashpack_reproduced":true,"tamper_checks":3}\n'
