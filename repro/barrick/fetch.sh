#!/usr/bin/env bash
# Fetches the issuer workbooks registered in registries/sources/barrick.toml into the gitignored corpus directory
# and checks each against its registered size and sha256 (BD-339.S2). Needs bash, awk, curl and sha256sum or
# shasum; no build.
#
#   repro/barrick/fetch.sh          fetch what is missing, verify what is present
#   repro/barrick/fetch.sh --list   print "file<TAB>url<TAB>bytes<TAB>sha256" per registered row, fetch nothing
#
# A file already present is verified and never overwritten. A download is saved under its registered name only
# when its size and digest match; otherwise the download is discarded. Either mismatch exits 1 with "register a
# new vintage (do not edit the digest)": the digest, not the URL, binds a run to its input.
#
# Output: one JSON line per file, then {"stage":"fetch_summary",...}.
# Environment:
#   FMINE_WORKSPACE    workspace root (default: two levels above this script)
#   FMINE_FETCH_BASE   replaces the registry's url_prefix (offline test: file:///...)
#   FMINE_FETCH_UA     User-Agent header (default names the project and its URL)
#   FMINE_FETCH_SLEEP  seconds between downloads (default 2)
set -euo pipefail

ws="${FMINE_WORKSPACE:-$(cd "$(dirname "$0")/../.." && pwd)}"
registry="$ws/registries/sources/barrick.toml"
ua="${FMINE_FETCH_UA:-franken_mining reproduction kit (+https://github.com/Dicklesworthstone/franken_mining)}"
pause="${FMINE_FETCH_SLEEP:-2}"
changed="register a new vintage (do not edit the digest)"

die() {
  echo "fetch.sh: $*" >&2
  exit 2
}

[[ -f "$registry" ]] || die "no registry at $registry"

# The registry is the TOML subset fmine-registry reads: one `key = value` per line, strings in double quotes,
# integers with optional `_`, `[[table]]` rows. Prints "top<TAB>key<TAB>value" for the keys before the first
# row, then "source<TAB>file<TAB>path<TAB>bytes<TAB>sha256" per [[source]] row.
rows() {
  awk -v reg="$registry" '
    function fail(msg) { printf "fetch.sh: %s line %d: %s\n", reg, NR, msg > "/dev/stderr"; bad = 1; exit 1 }
    function flush() {
      if (table == "source") print "source\t" f["file"] "\t" f["path"] "\t" f["bytes"] "\t" f["sha256"]
      split("", f)
    }
    /^[ \t]*(#.*)?$/ { next }
    /^[ \t]*\[\[/ {
      flush()
      table = $0
      sub(/^[ \t]*\[\[[ \t]*/, "", table)
      sub(/[ \t]*\]\].*$/, "", table)
      next
    }
    {
      eq = index($0, "=")
      if (eq == 0) fail("expected key = value")
      key = substr($0, 1, eq - 1)
      gsub(/[ \t]/, "", key)
      v = substr($0, eq + 1)
      sub(/^[ \t]+/, "", v)
      if (substr(v, 1, 1) == "\"") {
        rest = substr(v, 2)
        q = index(rest, "\"")
        if (q == 0) fail("unterminated string")
        v = substr(rest, 1, q - 1)
        if (index(v, "\\") > 0) fail("escape sequences are outside what fetch.sh reads")
      } else {
        sub(/[ \t]*(#.*)?$/, "", v)
        gsub(/_/, "", v)
      }
      if (table == "") print "top\t" key "\t" v
      else f[key] = v
    }
    END { if (!bad) flush() }
  ' "$registry"
}

table="$(rows)" || exit 2
top() { awk -F'\t' -v k="$1" '$1 == "top" && $2 == k { print $3 }' <<<"$table"; }
corpus_dir="$(top corpus_dir)"
prefix="$(top url_prefix)"
[[ -n "$corpus_dir" && "$corpus_dir" != /* && "/$corpus_dir/" != */../* ]] || die "corpus_dir must be relative"
[[ -n "$prefix" ]] || die "registry has no url_prefix"
sources="$(awk -F'\t' '$1 == "source"' <<<"$table")"
[[ -n "$sources" ]] || die "registry has no [[source]] rows"
# A file name is a bare name, so nothing can be written outside the corpus directory.
while IFS=$'\t' read -r _ file path bytes sha; do
  [[ "$file" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || die "file name $file is not a bare name"
  [[ -n "$path" ]] || die "$file has no path"
  [[ "$bytes" =~ ^[0-9]+$ ]] || die "$file: bytes $bytes is not an integer"
  [[ "$sha" =~ ^[0-9a-f]{64}$ ]] || die "$file: sha256 is not 64 lowercase hex digits"
done <<<"$sources"

if [[ "${1:-}" == "--list" ]]; then
  while IFS=$'\t' read -r _ file path bytes sha; do
    printf '%s\t%s\t%s\t%s\n' "$file" "$prefix$path" "$bytes" "$sha"
  done <<<"$sources"
  exit 0
fi
[[ $# -eq 0 ]] || die "usage: fetch.sh [--list]"

if command -v sha256sum >/dev/null 2>&1; then
  digest() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  digest() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  die "needs sha256sum or shasum"
fi
size() { wc -c <"$1" | tr -d ' '; }

base="${FMINE_FETCH_BASE:-$prefix}"
corpus="$ws/$corpus_dir"
mkdir -p "$corpus"
files=0
ok=0
failed=()
downloads=0

# log FILE URL URL_EFFECTIVE BYTES SHA OK CACHED [ERROR]
log() {
  local eff="null" err=""
  [[ -n "$3" ]] && eff="\"$3\""
  [[ -n "${8:-}" ]] && err=",\"error\":\"$8\""
  printf '{"stage":"fetch","file":"%s","url":"%s","url_effective":%s,"bytes":%s,"sha256":"%s","ok":%s,"cached":%s%s}\n' \
    "$1" "$2" "$eff" "${4:-0}" "$5" "$6" "$7" "$err"
}

while IFS=$'\t' read -r _ file path bytes sha; do
  files=$((files + 1))
  url="$base$path"
  target="$corpus/$file"
  if [[ -e "$target" ]]; then
    got_bytes="$(size "$target")"
    got_sha="$(digest "$target")"
    if [[ "$got_bytes" == "$bytes" && "$got_sha" == "$sha" ]]; then
      ok=$((ok + 1))
      log "$file" "$url" "" "$got_bytes" "$got_sha" true true
    else
      failed+=("$file")
      log "$file" "$url" "" "$got_bytes" "$got_sha" false true "present file does not match the registry: $changed"
      echo "fetch.sh: $file: present file does not match the registry: $changed" >&2
    fi
    continue
  fi
  if ((downloads > 0)); then
    sleep "$pause"
  fi
  downloads=$((downloads + 1))
  tmp="$(mktemp "$corpus/.fetch.XXXXXX")"
  if ! eff="$(curl -fsSL --retry 3 -A "$ua" -w '%{url_effective}' -o "$tmp" "$url")"; then
    rm -f "$tmp"
    failed+=("$file")
    log "$file" "$url" "" 0 "" false false "download failed"
    echo "fetch.sh: $file: download failed from $url" >&2
    continue
  fi
  got_bytes="$(size "$tmp")"
  got_sha=""
  if [[ "$got_bytes" == "$bytes" ]]; then
    got_sha="$(digest "$tmp")"
  fi
  if [[ "$got_bytes" == "$bytes" && "$got_sha" == "$sha" ]]; then
    mv "$tmp" "$target"
    ok=$((ok + 1))
    log "$file" "$url" "$eff" "$got_bytes" "$got_sha" true false
  else
    [[ -n "$got_sha" ]] || got_sha="$(digest "$tmp")"
    rm -f "$tmp"
    failed+=("$file")
    log "$file" "$url" "$eff" "$got_bytes" "$got_sha" false false "issuer file changed: $changed"
    echo "fetch.sh: $file: issuer file changed: $changed" >&2
  fi
done <<<"$sources"

list=""
for f in "${failed[@]+"${failed[@]}"}"; do
  list+="${list:+,}\"$f\""
done
printf '{"stage":"fetch_summary","files":%d,"ok":%d,"failed":[%s]}\n' "$files" "$ok" "$list"
((${#failed[@]} == 0)) || exit 1
