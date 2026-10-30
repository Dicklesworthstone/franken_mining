#!/usr/bin/env bash
# Registers a new quarterly vintage of the issuer workbooks as data (bd-s9j3.2). It never edits the registry and
# never commits workbook bytes.
#
#   scripts/register_vintage.sh [--dry-run] <url>...
#     Fetches each URL, sequentially, into a new, empty corpus_of_input_docs/incoming/<UTC stamp>/ (gitignored,
#     untrusted input). It runs the independent reader (scripts/oracle/fidelity.py and census2.py, unchanged) on
#     the files and seals its output, oracle.jsonl, with its sha256 and time before any fmine run. It then
#     prints candidate [[source]] rows for a human to paste into registries/sources/barrick.toml, also written
#     to <dir>/rows.toml. A file whose sha256 is already registered prints no row, except with --dry-run.
#   scripts/register_vintage.sh --run <YYYYQn> <incoming dir>
#     After the human's data-only commit of the rows, and repro/barrick/fetch.sh, which places the new files in
#     the corpus and checks their digests. It checks the oracle seal and that the fmine binary is the one the
#     blind record names. It runs the binary on all sources and on --through <YYYYQn>, and prints the diffs:
#     the two roots, the census per new file against the oracle, the ledger counts, new exceptions, table
#     refusals and unregistered headers. It never fixes a mismatch.
#
# Output: one JSON line per step. Environment:
#   FMINE_WORKSPACE       workspace root (default: one level above this script)
#   FMINE_FETCH_BASE      replaces the registry's url_prefix when fetching (offline test: file:///...)
#   FMINE_FETCH_UA        User-Agent header (default names the project and its URL)
#   FMINE_FETCH_SLEEP     seconds between downloads (default 2)
#   FMINE_INCOMING_STAMP  directory name under incoming/ (default: the UTC time)
#   FMINE_BIN             fmine binary for --run (default <workspace>/target/release/fmine)
#   FMINE_BLIND_RECORD    blind record for --run (default <workspace>/repro/barrick/blind_record.json), one JSON
#                         object with "binary_sha256" (bd-s9j3.1 writes it when the binary is pushed)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
ws="${FMINE_WORKSPACE:-$(cd "$here/.." && pwd)}"
helper=(python3 -I "$here/register_vintage.py")
ua="${FMINE_FETCH_UA:-franken_mining reproduction kit (+https://github.com/Dicklesworthstone/franken_mining)}"
pause="${FMINE_FETCH_SLEEP:-2}"

die() {
  echo "register_vintage.sh: $*" >&2
  exit 2
}
now() { python3 -I -c 'import datetime; print(datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ"))'; }
if command -v sha256sum >/dev/null 2>&1; then
  digest() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  digest() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  die "needs sha256sum or shasum"
fi
size() { wc -c <"$1" | tr -d ' '; }

registry="$ws/registries/sources/barrick.toml"
[[ -f "$registry" ]] || die "no registry at $registry"

if [[ "${1:-}" == "--run" ]]; then
  [[ $# -eq 3 ]] || die "usage: register_vintage.sh --run <YYYYQn> <incoming dir>"
  bound="$2"
  dir="$3"
  [[ "$bound" =~ ^20[0-9]{2}Q[1-4]$ ]] || die "bound $bound is not YYYYQn"
  [[ -f "$dir/oracle.jsonl" && -f "$dir/log.jsonl" ]] || die "$dir has no sealed oracle"
  "${helper[@]}" sealed "$dir"
  bin="${FMINE_BIN:-$ws/target/release/fmine}"
  record="${FMINE_BLIND_RECORD:-$ws/repro/barrick/blind_record.json}"
  [[ -x "$bin" ]] || die "no fmine binary at $bin"
  [[ -f "$record" ]] || die "no blind record at $record (bd-s9j3.1 writes it when the binary is pushed)"
  want="$(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["binary_sha256"])' "$record")"
  got="$(digest "$bin")"
  equal=false
  [[ "$got" == "$want" ]] && equal=true
  printf '{"stage":"binary","path":"%s","sha256":"%s","blind_sha256":"%s","equal":%s}\n' "$bin" "$got" "$want" "$equal"
  if [[ "$equal" != true ]]; then
    echo "register_vintage.sh: binary differs from the recorded blind binary" >&2
    exit 1
  fi
  for mode in through all; do
    args=(assure public --issuer barrick --workspace "$ws" --format json)
    [[ "$mode" == through ]] && args+=(--through "$bound")
    t="$(now)"
    printf '{"stage":"fmine","mode":"%s","utc":"%s"}\n' "$mode" "$t" >>"$dir/log.jsonl"
    code=0
    "$bin" "${args[@]}" >"$dir/run_$mode.json" || code=$?
    printf '{"stage":"fmine","mode":"%s","utc":"%s","exit":%d}\n' "$mode" "$t" "$code"
  done
  exec "${helper[@]}" diff "$ws" "$dir" "$bound"
fi

dry_run=()
if [[ "${1:-}" == "--dry-run" ]]; then
  dry_run=(--dry-run)
  shift
fi
[[ $# -ge 1 ]] || die "usage: register_vintage.sh [--dry-run] <url>... | --run <YYYYQn> <incoming dir>"
prefix="$(awk -F'"' '/^url_prefix[ \t]*=/ { print $2; exit }' "$registry")"
[[ -n "$prefix" ]] || die "registry has no url_prefix"
stamp="${FMINE_INCOMING_STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}"
[[ "$stamp" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || die "stamp $stamp is not a bare name"
dir="$ws/corpus_of_input_docs/incoming/$stamp"
if [[ -e "$dir" && -n "$(ls -A "$dir")" ]]; then
  die "$dir is not empty: an incoming directory is never reused"
fi
mkdir -p "$dir/fetched"
: >"$dir/fetch.jsonl"

n=0
for url in "$@"; do
  name="${url##*/}"
  [[ "$name" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || die "the URL's last segment $name is not a bare file name"
  out="$dir/fetched/$name"
  [[ ! -e "$out" ]] || die "two URLs end in $name"
  from="$url"
  if [[ -n "${FMINE_FETCH_BASE:-}" && "$url" == "$prefix"* ]]; then
    from="$FMINE_FETCH_BASE${url#"$prefix"}"
  fi
  if ((n > 0)); then
    sleep "$pause"
  fi
  n=$((n + 1))
  eff="$(curl -fsSL --retry 3 -A "$ua" -w '%{url_effective}' -o "$out" "$from")" || die "download failed: $from"
  t="$(now)"
  b="$(size "$out")"
  s="$(digest "$out")"
  printf '{"url":"%s","url_effective":"%s","path":"%s","bytes":%s,"sha256":"%s","utc":"%s"}\n' \
    "$url" "$eff" "$out" "$b" "$s" "$t" >>"$dir/fetch.jsonl"
  printf '{"stage":"fetch","url":"%s","url_effective":"%s","bytes":%s,"sha256":"%s","dir":"%s"}\n' \
    "$url" "$eff" "$b" "$s" "$dir"
done

"${helper[@]}" oracle "$ws" "$dir"
"${helper[@]}" rows "$ws" "$dir" "${dry_run[@]+"${dry_run[@]}"}"
