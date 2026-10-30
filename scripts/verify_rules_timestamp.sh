#!/usr/bin/env bash
# Checks the time proofs of the FY2026 forecast rules (BD-334.S2) with openssl, and ots when installed, so that
# anyone can check them without this repository's Rust toolchain.
#
# Usage: scripts/verify_rules_timestamp.sh [dir] [tsa_dir]   (defaults: forecasts/fy2026 and forecasts/tsa)
#
# The FMRUL1 digest is recomputed from dir/rules.toml ("FMRUL1" LF, the byte length in decimal, LF, the bytes) and
# must equal dir/rules.sha256. Then each proof present is checked:
# - dir/rules.tsr, an RFC 3161 reply over that digest: `openssl ts -verify` must accept it against one of
#   tsa_dir/*.pem. gen_time_utc is the reply's time stamp to the second, before its stated accuracy;
# - dir/rules.ots, an OpenTimestamps proof of the FMRUL1 frame's bytes: `ots verify` must name a Bitcoin block.
#   Without ots installed it is reported unverified, not checked.
#
# Output: one JSON line per proof {file, sha256, proof_kind, verified, gen_time_utc, tsa_or_block}, then
# {"proofs": N, "verified": M}. Exits 0 only when at least one proof exists and every proof verifies.
set -u
here="$(cd "$(dirname "$0")" && pwd)"
dir="${1:-$here/../forecasts/fy2026}"
tsa_dir="${2:-$here/../forecasts/tsa}"
proofs=0
verified=0
clean() { printf '%s' "$1" | tr '\n"\\' '   ' | cut -c1-300; }
emit() { # file proof_kind verified gen_time tsa_or_block
  proofs=$((proofs + 1))
  [ "$3" = true ] && verified=$((verified + 1))
  printf '{"file":"%s","sha256":"%s","proof_kind":"%s","verified":%s,"gen_time_utc":"%s","tsa_or_block":"%s"}\n' \
    "$(clean "$1")" "$hex" "$2" "$3" "$(clean "$4")" "$(clean "$5")"
}
finish() {
  echo "{\"proofs\":$proofs,\"verified\":$verified}"
  if [ "$proofs" -gt 0 ] && [ "$verified" -eq "$proofs" ]; then
    exit 0
  fi
  exit 1
}
if command -v sha256sum >/dev/null 2>&1; then
  sha() { sha256sum | cut -d' ' -f1; }
else
  sha() { shasum -a 256 | cut -d' ' -f1; }
fi
frame() {
  printf 'FMRUL1\n%s\n' "$(wc -c <"$dir/rules.toml" | tr -d ' ')"
  cat "$dir/rules.toml"
}
# "Oct 30 14:04:42 2026 GMT", a fraction of a second dropped, to 2026-10-30T14:04:42Z (GNU date, then BSD date).
utc() {
  local t
  t="$(printf '%s' "$1" | sed 's/\.[0-9]*//')"
  date -u -d "$t" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null ||
    date -u -j -f '%b %d %T %Y %Z' "$t" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null ||
    printf '%s' "$1"
}

if [ ! -f "$dir/rules.toml" ]; then
  hex=""
  echo "{\"error\":\"no rules.toml in $(clean "$dir")\"}"
  finish
fi
hex="$(frame | sha)"
stored="$(tr -d '\n' <"$dir/rules.sha256" 2>/dev/null)"
if [ "$hex" != "$stored" ]; then
  echo "{\"error\":\"rules.sha256 holds '$(clean "$stored")', but rules.toml's FMRUL1 digest is $hex\"}"
  finish
fi

if [ -f "$dir/rules.tsr" ]; then
  ok=false
  by=""
  why="no *.pem in $tsa_dir"
  for chain in "$tsa_dir"/*.pem; do
    [ -f "$chain" ] || continue
    if out="$(openssl ts -verify -digest "$hex" -in "$dir/rules.tsr" -CAfile "$chain" 2>&1)"; then
      ok=true
      by="$(basename "$chain")"
      break
    fi
    why="$(basename "$chain"): $(printf '%s' "$out" | tail -1)"
  done
  stamp="$(openssl ts -reply -in "$dir/rules.tsr" -text 2>&1 | sed -n 's/^Time stamp: //p')"
  if [ "$ok" = true ]; then
    emit rules.tsr rfc3161 true "$(utc "$stamp")" "$by"
  else
    emit rules.tsr rfc3161 false "$(utc "$stamp")" "$why"
  fi
fi

if [ -f "$dir/rules.ots" ]; then
  if command -v ots >/dev/null 2>&1; then
    framefile="$(mktemp "${TMPDIR:-/tmp}/fmine-rules-frame.XXXXXX")"
    frame >"$framefile"
    out="$(ots verify -f "$framefile" "$dir/rules.ots" 2>&1)"
    block="$(printf '%s' "$out" | grep -o 'Bitcoin block [0-9]*' | head -1)"
    when="$(printf '%s' "$out" | sed -n 's/.*attests existence as of //p' | head -1)"
    if [ -n "$block" ]; then
      emit rules.ots opentimestamps true "$when" "$block"
    else
      emit rules.ots opentimestamps false "" "$(printf '%s' "$out" | tail -1)"
    fi
  else
    emit rules.ots opentimestamps false "" "ots is not installed: not checked"
  fi
fi

finish
