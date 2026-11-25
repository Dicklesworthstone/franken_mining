#!/usr/bin/env bash
# The real-lane run (BD-339.S3), as .github/workflows/real-files.yml runs it and as an owner can run it by hand:
# Fetch and verify the registered Barrick workbooks, run the ignored real-file tests, print the Q2 and Q3 roots,
# and check that no workbook is in git.
#
# Output: {"stage","ok","elapsed_s"} per stage, plus fetch.sh's lines, {"stage":"root",...} and
# {"stage":"offline_root",...}. It stops at the first failed stage with exit 1. Under GitHub Actions the root is also
# written to $GITHUB_OUTPUT as root=<hex> for Q2 and root_q3=<hex> for Q3.
# BD-338.S3's live-dependency proof runs in two places:
# - its file checks and the real M1, inside test_public_integration;
# - its leaf sample (one planned perturbation per instance, about 2,600 reruns of the pipeline), as the release-mode stage
#   live_leaves. The reruns reuse the Mine Stats section unless a leaf is in a Mine Stats file, and run on
#   every core (bd-r7lp). Release, 2026-10-30: 55 s on 36 cores, 751 s on one thread.
set -u -o pipefail
cd "$(dirname "$0")/../.." || exit 1

stage() {
  local name="$1" t0=$SECONDS ok=true
  shift
  "$@" || ok=false
  printf '{"stage":"%s","ok":%s,"elapsed_s":%d}\n' "$name" "$ok" $((SECONDS - t0))
  [[ $ok == true ]] || exit 1
}

root() {
  local through="$1" output_key="$2"
  cargo run --locked -q -p fmine-cli -- assure public --issuer barrick --through "$through" --json | python3 -I -c '
import json, os, sys
env = json.load(sys.stdin)
root, anchor = env["certificate"], env["anchor_id"]
if not isinstance(root, str) or len(root) != 64 or any(c not in "0123456789abcdef" for c in root):
    raise SystemExit("assurance did not return a state root")
print(json.dumps({"stage": "root", "through": sys.argv[1], "root": root, "anchor_id": anchor}, separators=(",", ":")))
out = os.environ.get("GITHUB_OUTPUT")
if out:
    with open(out, "a") as f:
        f.write(f"{sys.argv[2]}={root}\n")
' "$through" "$output_key"
}

xlsx_guard() {
  test -z "$(git ls-files '*.xlsx' '*.xls')"
}

# bd-073: the release binary gives the same root with no network at all. A fresh network namespace (Linux, so CI
# only) holds just a down loopback; the run inside it drops back to this user. The control first shows that the
# namespace cannot open a connection, so an equal root is not a namespace that leaked.
offline() {
  local bin=target/release/fmine online isolated
  local cert='import json, sys; print(json.loads(sys.stdin.read())["certificate"])'
  cargo build -q --release --locked -p fmine-cli || return 1
  if sudo -n unshare --net -- python3 -I -c 'import socket; socket.create_connection(("github.com", 443), 5)'; then
    echo "offline: the namespace reached the network" >&2
    return 1
  fi
  online="$("$bin" assure public --issuer barrick --through 2026Q2 | python3 -I -c "$cert")" || return 1
  isolated="$(sudo -n unshare --net -- setpriv --reuid "$(id -u)" --regid "$(id -g)" --clear-groups -- \
    "$bin" assure public --issuer barrick --through 2026Q2 | python3 -I -c "$cert")" || return 1
  printf '{"stage":"offline_root","online":"%s","no_network":"%s"}\n' "$online" "$isolated"
  [[ -n $online && $online == "$isolated" ]]
}

stage fetch repro/barrick/fetch.sh
stage test_public cargo test -p fmine-public -- --ignored --skip barrick_live_leaves_one_per_instance
# Recovery and PDF-conflict checks need local two-reader transcriptions, which are never in git. This workbook
# lane excludes public_recovery and the CLI recovery/PDF-conflict checks; they run with the supplied local inputs.
stage test_public_integration cargo test -p xtask \
  --test public_corpus --test public_exact_feas_blame --test public_laap_2023 \
  --test public_keys_aisc --test public_keys_mine_stats --test public_mine_stats_inventory \
  --test public_mine_stats_vintages --test public_transcriptions \
  --test public_consistency --test public_waterfall --test public_manifest --test public_lint \
  --test public_report_contract --test public_workbook_reading \
  -- --ignored --skip barrick_live_leaves_one_per_instance
stage live_leaves cargo test --release -p xtask --test public_report_contract -- --ignored --exact \
  tests::live_dependency::barrick_live_leaves_one_per_instance --nocapture
stage test_cli cargo test -p fmine-cli -- --ignored \
  --skip test_real_recovery_is_embedded_in_assurance_with_separate_digest \
  --skip barrick_pdf_packets_cover_registered_conflicts
stage root root 2026Q2 root
stage root_q3 root 2026Q3 root_q3
stage xlsx_guard xlsx_guard
if [[ "$(uname -s)" == Linux ]]; then
  stage offline offline
fi
