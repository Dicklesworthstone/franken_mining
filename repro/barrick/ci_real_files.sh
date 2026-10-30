#!/usr/bin/env bash
# The real-lane run (BD-339.S3), as .github/workflows/real-files.yml runs it and as an owner can run it by hand:
# fetch and verify the registered Barrick workbooks, run the ignored real-file tests (which check the root and
# anchor of repro/barrick/expected.toml's last row), print the root, and check that no workbook is in git.
#
# Output: {"stage","ok","elapsed_s"} per stage, plus fetch.sh's lines and {"stage":"root",...}. It stops at the
# first failed stage with exit 1. Under GitHub Actions the root is also written to $GITHUB_OUTPUT as root=<hex>.
# BD-338.S3 adds its live-dependency proof as a stage here.
set -u
cd "$(dirname "$0")/../.." || exit 1

stage() {
  local name="$1" t0=$SECONDS ok=true
  shift
  "$@" || ok=false
  printf '{"stage":"%s","ok":%s,"elapsed_s":%d}\n' "$name" "$ok" $((SECONDS - t0))
  [[ $ok == true ]] || exit 1
}

root() {
  local env
  env="$(cargo run -q -p fmine-cli -- assure public --issuer barrick --through 2026Q2)" || return 1
  python3 -I -c '
import json, os, sys
env = json.loads(sys.stdin.read())
root, anchor = env["certificate"], env["anchor_id"]
print(json.dumps({"stage": "root", "root": root, "anchor_id": anchor}, separators=(",", ":")))
out = os.environ.get("GITHUB_OUTPUT")
if out:
    with open(out, "a") as f:
        f.write(f"root={root}\n")
' <<<"$env"
}

xlsx_guard() {
  test -z "$(git ls-files '*.xlsx' '*.xls')"
}

stage fetch repro/barrick/fetch.sh
stage test_public cargo test -p fmine-public -- --ignored
stage test_cli cargo test -p fmine-cli -- --ignored
stage root root
stage xlsx_guard xlsx_guard
