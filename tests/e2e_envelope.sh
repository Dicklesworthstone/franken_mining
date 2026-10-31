#!/usr/bin/env bash
# End-to-end check of the fmine envelope (BD-329.S3, PO-ADIT-1). It runs each verb through
# `cargo run -p fmine-cli --` and checks the one line it prints: one JSON envelope with the four INV-002 flags
# all false, `certificate` (a string for report verbs, null otherwise), `warnings` (an array) and `next_actions`
# (a non-empty array of {command, why}), and the expected verb, status, exit code and refusal code (`-` for none).
#
# Output: one JSON log line per run, then {"runs": N, "failures": M}. Exits 1 if any run fails.
# The real-corpus run needs corpus_of_input_docs/downloaded/mine_stats/ (never in git); without it, that run is
# logged as skipped: "corpus_absent".
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root" || exit 1
cargo build -q -p fmine-cli || exit 1

read -r -d '' CHECK <<'PY'
import json
import sys

verb, args, want_exit, want_status, want_cert, want_code, exit_code = sys.argv[1:8]
out = sys.stdin.read()
flags = {"reserve_disclosure_claim": False, "execution_authorized": False,
         "geotechnical_signoff": False, "regulatory_compliance_claim": False}
problems = []
if len(out.splitlines()) != 1:
    problems.append(f"{len(out.splitlines())} stdout lines")
try:
    env = json.loads(out)
except ValueError as e:
    env = {}
    problems.append(f"not JSON: {e}")
flags_ok = env.get("flags") == flags and out.count('"flags":{') == 1
has_certificate = isinstance(env.get("certificate"), str)
has_warnings = isinstance(env.get("warnings"), list)
actions = env.get("next_actions")
has_next_actions = (isinstance(actions, list) and len(actions) > 0
                    and all(isinstance(a, dict) and set(a) == {"command", "why"} for a in actions))
if not flags_ok:
    problems.append("flags")
if has_certificate != (want_cert == "1") or ("certificate" not in env):
    problems.append("certificate")
if not has_warnings:
    problems.append("warnings")
if not has_next_actions:
    problems.append("next_actions")
if int(exit_code) != int(want_exit) or env.get("exit_code") != int(want_exit):
    problems.append(f"exit {exit_code} (envelope exit_code {env.get('exit_code')}), expected {want_exit}")
if env.get("status") != want_status:
    problems.append(f"status {env.get('status')}")
if env.get("verb") != verb:
    problems.append(f"verb {env.get('verb')}")
refusal = env.get("refusal")
code = refusal.get("code") if isinstance(refusal, dict) else None
if code != (None if want_code == "-" else want_code):
    problems.append(f"refusal code {code}, expected {want_code}")
if isinstance(refusal, dict) and refusal.get("exit_code") != int(want_exit):
    problems.append(f"refusal exit_code {refusal.get('exit_code')}")
print(json.dumps({"verb": verb, "args": args, "exit": int(exit_code), "status": env.get("status"), "code": code,
                  "flags_ok": flags_ok, "has_certificate": has_certificate, "has_warnings": has_warnings,
                  "has_next_actions": has_next_actions, "ok": not problems, "problems": problems}))
sys.exit(1 if problems else 0)
PY

runs=0
failures=0

# run <verb> <exit> <status> <certificate 0|1> <refusal code|-> -- <fmine args...>
run() {
    local verb="$1" want_exit="$2" want_status="$3" want_cert="$4" want_code="$5"
    shift 6
    local out code
    out="$(cargo run -q -p fmine-cli -- "$@")"
    code=$?
    runs=$((runs + 1))
    if ! printf '%s\n' "$out" | python3 -I -c "$CHECK" "$verb" "$*" "$want_exit" "$want_status" "$want_cert" \
        "$want_code" "$code"; then
        failures=$((failures + 1))
    fi
}

run "assure public" 0 ok 1 - -- assure public --synthetic
run "doctor" 0 ok 0 - -- doctor
run "robot-docs" 0 ok 0 - -- robot-docs
run "claim show" 3 refused 0 FMINE-REF-201 -- claim show clm_0001
run "unknown" 2 usage_error 0 - -- frobnicate
run "assure public" 2 usage_error 0 - -- assure public --issuer newmont
run "doctor" 2 usage_error 0 - -- doctor --deep

# A workspace with the barrick registry and no corpus: FMINE-REF-506 (registered input missing), exit 5.
ws="$(mktemp -d)"
mkdir -p "$ws/registries/sources"
cp registries/sources/barrick.toml "$ws/registries/sources/barrick.toml"
run "assure public" 5 refused 0 FMINE-REF-506 -- assure public --issuer barrick --workspace "$ws"
rm -f "$ws/registries/sources/barrick.toml"
rmdir "$ws/registries/sources" "$ws/registries" "$ws"

if [ -d corpus_of_input_docs/downloaded/mine_stats ]; then
    run "assure public" 0 ok 1 - -- assure public --issuer barrick --through 2026Q2
else
    printf '{"verb": "assure public", "args": "assure public --issuer barrick --through 2026Q2", "skipped": "corpus_absent"}\n'
fi

printf '{"runs": %d, "failures": %d}\n' "$runs" "$failures"
[ "$failures" -eq 0 ]
