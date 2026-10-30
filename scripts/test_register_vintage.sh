#!/usr/bin/env bash
# Offline test of scripts/register_vintage.sh (bd-s9j3.2). A scratch workspace registers one invented row; two
# small synthetic workbooks are served through file:// under issuer-style URL paths. No network, no corpus.
#
# Cases:
# - the happy path prints two rows spelled as the registry spells them, and seals the oracle;
# - a reused incoming directory refuses (exit 2);
# - an already-registered sha256 prints no row, and with --dry-run it does;
# - --run refuses a binary whose sha256 differs from the blind record (exit 1) and runs nothing;
# - --run refuses an oracle.jsonl changed after its seal (exit 2);
# - --run with the recorded binary runs it after the seal and refuses its output when it has no payload (exit 1).
#
# Output: one JSON line per case, then {"cases": N, "failures": M}. Exits 1 if any case fails. The scratch
# directory is left in place and named in the first line.
set -u
here="$(cd "$(dirname "$0")" && pwd)"
script="$here/register_vintage.sh"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/fmine-vintage-test.XXXXXX")"
srv="$scratch/served"
echo "{\"scratch\":\"$scratch\"}"

if command -v sha256sum >/dev/null 2>&1; then
  digest() { sha256sum "$1" | cut -d' ' -f1; }
else
  digest() { shasum -a 256 "$1" | cut -d' ' -f1; }
fi
size() { wc -c <"$1" | tr -d ' '; }
# Digit groups of three joined by "_", as the registry writes bytes (1524 -> 1_524).
group() {
  awk -v n="$1" 'BEGIN { s = ""; while (length(n) > 3) { s = "_" substr(n, length(n) - 2) s; n = substr(n, 1, length(n) - 3) } print n s }'
}
zeros=0000000000000000000000000000000000000000000000000000000000000000

# Two synthetic one-sheet workbooks, byte-stable (fixed zip times), holding different numbers.
mkdir -p "$srv/2026/q3"
stats="$srv/2026/q3/Barrick_Q3_2026_Mine_Stats.xlsx"
aisc="$srv/2026/q3/AISC_Q3_2026.xlsx"
python3 -I - "$stats" "$aisc" <<'PY'
import sys, zipfile
main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
pkg = "http://schemas.openxmlformats.org/package/2006/relationships"
for n, path in enumerate(sys.argv[1:], 1):
    parts = {
        "[Content_Types].xml": '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>',
        "_rels/.rels": '<Relationships xmlns="%s"><Relationship Id="rId1" Type="%s/officeDocument" '
                       'Target="xl/workbook.xml"/></Relationships>' % (pkg, rel),
        "xl/workbook.xml": '<workbook xmlns="%s" xmlns:r="%s"><sheets><sheet name="S" sheetId="1" r:id="rId1"/>'
                           '</sheets></workbook>' % (main, rel),
        "xl/_rels/workbook.xml.rels": '<Relationships xmlns="%s"><Relationship Id="rId1" Type="%s/worksheet" '
                                      'Target="worksheets/sheet1.xml"/></Relationships>' % (pkg, rel),
        "xl/worksheets/sheet1.xml": '<worksheet xmlns="%s"><sheetData><row r="1"><c r="A1"><v>%d</v></c></row>'
                                    '</sheetData></worksheet>' % (main, n),
    }
    with zipfile.ZipFile(path, "w") as z:
        for name, body in parts.items():
            z.writestr(zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0)), body)
PY
urls=(https://example.org/files/2026/q3/Barrick_Q3_2026_Mine_Stats.xlsx https://example.org/files/2026/q3/AISC_Q3_2026.xlsx)

# registry WS SHA256_OF_THE_ONE_REGISTERED_ROW
registry() {
  mkdir -p "$1/registries/sources"
  printf 'schema = "fmine.sources.v1"\nissuer = "barrick"\ncorpus_dir = "corpus"\nretrieved_utc = "invented"\n' \
    >"$1/registries/sources/barrick.toml"
  printf 'url_prefix = "https://example.org/files/"\n\n[[source]]\nfile = "Barrick_Q2_2026_AISC.xlsx"\n' \
    >>"$1/registries/sources/barrick.toml"
  printf 'kind = "aisc_reconciliation"\nquarter = "2026Q2"\npath = "2026/q2/AISC_Q2_2026.xlsx"\n' \
    >>"$1/registries/sources/barrick.toml"
  printf 'url_basis = "W1 naming rule"\n' \
    >>"$1/registries/sources/barrick.toml"
  printf 'bytes = 1\nsha256 = "%s"\n' "$2" >>"$1/registries/sources/barrick.toml"
}

cases=0
failures=0
# check NAME DETAIL CONDITION...
check() {
  local name="$1" detail="$2"
  shift 2
  cases=$((cases + 1))
  if "$@"; then
    printf '{"case":"%s","ok":true}\n' "$name"
  else
    failures=$((failures + 1))
    printf '{"case":"%s","ok":false,"detail":"%s"}\n' "$name" "$detail"
  fi
}
# vintage WS STAMP [ARGS...]: runs the script; sets out, code, err
vintage() {
  local w="$1" stamp="$2"
  shift 2
  code=0
  out="$(FMINE_WORKSPACE="$w" FMINE_FETCH_BASE="file://$srv/" FMINE_FETCH_SLEEP=0 FMINE_INCOMING_STAMP="$stamp" \
    bash "$script" "$@" 2>"$scratch/err")" || code=$?
  err="$(cat "$scratch/err")"
}
has() { grep -qF -- "$2" "$1"; }
count() { grep -cF -- "$2" <<<"$1"; }

ws1="$scratch/ws1"
registry "$ws1" "$zeros"
vintage "$ws1" t1 "${urls[@]}"
dir1="$ws1/corpus_of_input_docs/incoming/t1"
rows="$dir1/rows.toml"
happy() {
  [[ $code -eq 0 ]] && (($(count "$out" '"stage":"fetch"') == 2)) && (($(count "$out" '"stage":"oracle"') == 2)) &&
    has "$rows" 'file = "Barrick_Q3_2026_Mine_Stats.xlsx"' && has "$rows" 'file = "Barrick_Q3_2026_AISC.xlsx"' &&
    has "$rows" 'kind = "aisc_reconciliation"' && has "$rows" 'kind = "mine_stats"' &&
    has "$rows" 'path = "2026/q3/AISC_Q3_2026.xlsx"' && (($(grep -c 'quarter = "2026Q3"' "$rows") == 2)) &&
    has "$rows" "bytes = $(group "$(size "$aisc")")" &&
    has "$rows" "sha256 = \"$(digest "$aisc")\"" && has "$rows" 'url_basis = "fetched and verified ' &&
    has "$dir1/log.jsonl" '"stage": "oracle_sealed"' && [[ "$out" == *'"stage":"rows"'*'"count":2'* ]]
}
check happy_path "rows, oracle seal and log lines" happy

vintage "$ws1" t1 "${urls[@]}"
reuse() { [[ $code -eq 2 && "$err" == *"is not empty"* ]]; }
check reused_directory_refused "exit $code: $err" reuse

ws2="$scratch/ws2"
registry "$ws2" "$(digest "$aisc")"
vintage "$ws2" t2 "${urls[@]}"
dir2="$ws2/corpus_of_input_docs/incoming/t2"
duplicate() {
  [[ $code -eq 0 && "$out" == *'"count":1'* && "$err" == *"already registered: Barrick_Q2_2026_AISC.xlsx"* ]] &&
    [[ "$out" == *'"file_name":"Barrick_Q3_2026_AISC.xlsx","quarter":"2026Q3","already_registered":true'* ]] &&
    ! has "$dir2/rows.toml" 'file = "Barrick_Q3_2026_AISC.xlsx"'
}
check registered_sha256_prints_no_row "exit $code" duplicate

vintage "$ws2" t3 --dry-run "${urls[@]}"
dry() { [[ $code -eq 0 && "$out" == *'"count":2'* ]]; }
check dry_run_prints_registered_row "exit $code" dry

fake="$scratch/fake-fmine"
printf '#!/bin/sh\necho "{}"\n' >"$fake"
chmod +x "$fake"
printf '{"binary_sha256":"%s"}\n' "$zeros" >"$scratch/record-other.json"
printf '{"binary_sha256":"%s"}\n' "$(digest "$fake")" >"$scratch/record-fake.json"
# run WS DIR RECORD: --run 2026Q2 with the fake binary; sets out, code, err
run() {
  code=0
  out="$(FMINE_WORKSPACE="$1" FMINE_BIN="$fake" FMINE_BLIND_RECORD="$3" bash "$script" --run 2026Q2 "$2" \
    2>"$scratch/err")" || code=$?
  err="$(cat "$scratch/err")"
}
run "$ws1" "$dir1" "$scratch/record-other.json"
other_binary() {
  [[ $code -eq 1 && "$err" == *"binary differs from the recorded blind binary"* ]] &&
    [[ ! -e "$dir1/run_all.json" && ! -e "$dir1/run_through.json" ]]
}
check other_binary_refused "exit $code: $err" other_binary

run "$ws2" "$dir2" "$scratch/record-fake.json"
no_payload() {
  [[ $code -eq 1 && "$err" == *"has no payload"* && "$out" == *'"stage":"oracle_check"'* ]] &&
    [[ "$out" == *'"stage":"binary"'*'"equal":true'* ]] && (($(grep -c '"stage":"fmine"' "$dir2/log.jsonl") == 2))
}
check recorded_binary_runs_after_seal "exit $code: $err" no_payload

printf ' ' >>"$dir1/oracle.jsonl"
run "$ws1" "$dir1" "$scratch/record-fake.json"
tampered() { [[ $code -eq 2 && "$err" == *"changed after it was sealed"* ]]; }
check changed_oracle_refused "exit $code: $err" tampered

printf '{"cases":%d,"failures":%d}\n' "$cases" "$failures"
((failures == 0))
