#!/usr/bin/env bash
# Offline test of repro/barrick/fetch.sh (BD-339.S2). A scratch workspace registers three small invented files,
# served through file:// under URL names that differ from the registered names. No network, no corpus.
#
# Cases: the happy path; a second run that only verifies; a served file with one byte changed (exit 1, "issuer
# file changed", nothing saved); a present file with one byte changed (exit 1, "present file does not match the
# registry", the file left as it was); --list; a registered file name that is not a bare name (exit 2).
#
# Output: one JSON line per case, then {"cases": N, "failures": M}. Exits 1 if any case fails. The scratch
# directory is left in place and named in the first line.
set -u
fetch="$(cd "$(dirname "$0")" && pwd)/fetch.sh"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/fmine-fetch-test.XXXXXX")"
srv="$scratch/served"
echo "{\"scratch\":\"$scratch\"}"

if command -v sha256sum >/dev/null 2>&1; then
  digest() { sha256sum "$1" | cut -d' ' -f1; }
else
  digest() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi
size() { wc -c <"$1" | tr -d ' '; }
# Replaces the first byte of a file in place; the size is unchanged.
flip() { printf 'X' | dd of="$1" bs=1 count=1 conv=notrunc 2>/dev/null; }

mkdir -p "$srv/q1" "$srv/q2"
printf 'invented workbook one\n' >"$srv/q1/AISC_Q1.xlsx"
printf 'invented workbook two, a little longer\n' >"$srv/q1/Stats_Q1.xlsx"
printf 'invented workbook three\n' >"$srv/q2/AISC_Q2.xlsx"

# registry WS [FILE_OF_FIRST_ROW]
registry() {
  local ws="$1" first="${2:-Invented_Q1_AISC.xlsx}"
  mkdir -p "$ws/registries/sources"
  {
    printf 'schema = "fmine.sources.v1"\nissuer = "barrick"\ncorpus_dir = "corpus"\n'
    printf 'url_prefix = "https://example.org/files/" # replaced by FMINE_FETCH_BASE\n'
    local row
    for row in "$first q1/AISC_Q1.xlsx" "Invented_Q1_Stats.xlsx q1/Stats_Q1.xlsx" \
      "Invented_Q2_AISC.xlsx q2/AISC_Q2.xlsx"; do
      set -- $row
      printf '\n[[source]]\nfile = "%s"\nkind = "aisc"\nquarter = "2024Q1"\npath = "%s"\n' "$1" "$2"
      printf 'bytes = %s\nsha256 = "%s"\n' "$(size "$srv/$2")" "$(digest "$srv/$2")"
    done
  } >"$ws/registries/sources/barrick.toml"
}

# run WS [ARGS] -> sets status, out, err
run() {
  local ws="$1"
  shift
  out="$(FMINE_WORKSPACE="$ws" FMINE_FETCH_BASE="file://$srv/" FMINE_FETCH_SLEEP=0 bash "$fetch" "$@" 2>"$scratch/err")"
  status=$?
  err="$(cat "$scratch/err")"
}

cases=0
failures=0
# check CASE CONDITION_STATUS DETAIL
check() {
  cases=$((cases + 1))
  local ok=true
  if [[ "$2" != 0 ]]; then
    ok=false
    failures=$((failures + 1))
  fi
  printf '{"case":"%s","ok":%s,"exit":%s,"detail":"%s"}\n' "$1" "$ok" "$status" "$3"
}
no_temp_left() { ! ls -A "$1/corpus" | grep -q '^\.fetch\.'; }

ws="$scratch/ws"
registry "$ws"
run "$ws"
[[ $status == 0 && "$out" == *'{"stage":"fetch_summary","files":3,"ok":3,"failed":[]}'* &&
  "$(digest "$ws/corpus/Invented_Q1_AISC.xlsx")" == "$(digest "$srv/q1/AISC_Q1.xlsx")" &&
  -f "$ws/corpus/Invented_Q2_AISC.xlsx" && ! -e "$ws/corpus/AISC_Q1.xlsx" ]] && no_temp_left "$ws"
check happy_path $? "three files saved under their registered names"

run "$ws"
[[ $status == 0 && "$(grep -c '"ok":true,"cached":true' <<<"$out")" == 3 ]]
check second_run_verifies $? "three present files verified, none downloaded"

ws2="$scratch/ws_changed"
registry "$ws2"
flip "$srv/q2/AISC_Q2.xlsx"
run "$ws2"
[[ $status == 1 && "$err" == *"Invented_Q2_AISC.xlsx: issuer file changed: register a new vintage (do not edit the digest)"* &&
  "$out" == *'"failed":["Invented_Q2_AISC.xlsx"]'* && ! -e "$ws2/corpus/Invented_Q2_AISC.xlsx" &&
  -f "$ws2/corpus/Invented_Q1_AISC.xlsx" ]] && no_temp_left "$ws2"
check issuer_file_changed $? "one byte changed at the source: exit 1, nothing saved for that file"
printf 'invented workbook three\n' >"$srv/q2/AISC_Q2.xlsx"

flip "$ws/corpus/Invented_Q1_Stats.xlsx"
before="$(digest "$ws/corpus/Invented_Q1_Stats.xlsx")"
run "$ws"
[[ $status == 1 && "$err" == *"Invented_Q1_Stats.xlsx: present file does not match the registry: register a new vintage (do not edit the digest)"* &&
  "$(digest "$ws/corpus/Invented_Q1_Stats.xlsx")" == "$before" ]]
check present_file_mismatch $? "one byte changed in a present file: exit 1, the file left as it was"

run "$ws" --list
[[ $status == 0 && "$(wc -l <<<"$out" | tr -d ' ')" == 3 &&
  "$out" == "Invented_Q1_AISC.xlsx	https://example.org/files/q1/AISC_Q1.xlsx	$(size "$srv/q1/AISC_Q1.xlsx")	$(digest "$srv/q1/AISC_Q1.xlsx")"* ]]
check list $? "three rows: file, registry URL, bytes, sha256"

ws3="$scratch/ws_escape"
registry "$ws3" "../escape.xlsx"
run "$ws3"
[[ $status == 2 && "$err" == *"is not a bare name"* && ! -e "$ws3/escape.xlsx" && ! -e "$ws3/corpus" ]]
check file_name_not_bare $? "a registered name with a path refuses before any download"

echo "{\"cases\": $cases, \"failures\": $failures}"
((failures == 0))
