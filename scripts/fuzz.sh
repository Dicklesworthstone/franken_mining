#!/usr/bin/env bash
# Coverage-guided fuzzing of every decoder and of whole `fmine` invocations (plan §32.4, PO-PORT-1, PO-ADIT-4;
# BD-017, BD-197, BD-112). The targets are in fuzz/.
#
# Usage: scripts/fuzz.sh [seconds-per-target] [target ...]   (default 60 seconds, every target)
#
# - Builds the targets through rch with cargo-fuzz (`cargo install cargo-fuzz`), with debug assertions on, so an
#   arithmetic overflow is a finding. No sanitizer: the product forbids unsafe code, so AddressSanitizer would check
#   only the standard library, at about twice the cost per run.
# - Writes the seeds (fuzz/seeds.py) and keeps each target's growing corpus, crash artifacts and log under
#   $FUZZ_DIR (default: the rch target base, beside the build), never in the repository.
# - Runs each target's binary directly for the given time, with a 2 GiB RSS limit and a 10-second per-input
#   timeout. An unbounded allocation therefore shows as an out-of-memory finding, and a hang as a timeout.
#
# Output: one JSON line per target {target, seconds, exit, runs, cov, ft, corpus, artifacts, log}, then
# {"targets": N, "failures": M}. Exits 1 if any target found a crash, a timeout or an out-of-memory input.
set -u
export LC_ALL=C
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
secs="${1:-60}"
shift $(($# > 0 ? 1 : 0))
targets=("$@")
[ "${#targets[@]}" -gt 0 ] || targets=(codec anchor journal xlsx numfmt registry record html cli cli_run pit cpit pcpsp)

base="${RCH_TARGET_BASE:-${TMPDIR:-/tmp}}"
target_dir="$base/rch_target_fmine_fuzz"
fuzz_dir="${FUZZ_DIR:-$target_dir/runs}"
host="$(rustc -vV | sed -n 's/^host: //p')"
bin_dir="$target_dir/$host/release"

cd "$root" || exit 2
mkdir -p "$fuzz_dir"
if ! rch exec -- env CARGO_TARGET_DIR="$target_dir" cargo fuzz build -s none -a >"$fuzz_dir.build.log" 2>&1; then
  tail -20 "$fuzz_dir.build.log"
  echo '{"build":false}'
  exit 2
fi
mkdir -p "$fuzz_dir/seeds"
python3 -I "$root/fuzz/seeds.py" "$root" "$fuzz_dir/seeds" >/dev/null || exit 2

failures=0
for t in "${targets[@]}"; do
  corpus="$fuzz_dir/corpus/$t"
  artifacts="$fuzz_dir/artifacts/$t"
  log="$fuzz_dir/logs/$t-$(date -u +%Y%m%dT%H%M%SZ).log"
  mkdir -p "$corpus" "$artifacts" "$(dirname "$log")"
  stamp="$fuzz_dir/logs/.start-$t"
  : >"$stamp"
  "$bin_dir/$t" "$corpus" "$fuzz_dir/seeds/$t" -max_total_time="$secs" -rss_limit_mb=2048 -timeout=10 \
    -artifact_prefix="$artifacts/" -print_final_stats=1 >"$log" 2>&1
  rc=$?
  runs="$(sed -n 's/^stat::number_of_executed_units: *//p' "$log" | tail -1)"
  done_line="$(grep -E '^#[0-9]+[[:space:]]+DONE' "$log" | tail -1)"
  cov="$(printf '%s' "$done_line" | sed -n 's/.*cov: \([0-9]*\).*/\1/p')"
  ft="$(printf '%s' "$done_line" | sed -n 's/.*ft: \([0-9]*\).*/\1/p')"
  n_corpus="$(find "$corpus" -type f | wc -l | tr -d ' ')"
  found="$(find "$artifacts" -type f -newer "$stamp" -exec basename {} \; | tr '\n' ' ' | sed 's/ $//')"
  [ "$rc" -eq 0 ] || failures=$((failures + 1))
  printf '{"target":"%s","seconds":%s,"exit":%s,"runs":%s,"cov":%s,"ft":%s,"corpus":%s,"artifacts":"%s","log":"%s"}\n' \
    "$t" "$secs" "$rc" "${runs:-0}" "${cov:-0}" "${ft:-0}" "$n_corpus" "$found" "$log"
done
echo "{\"targets\":${#targets[@]},\"failures\":$failures}"
[ "$failures" -eq 0 ]
