#!/usr/bin/env bash
# End-to-end test of the public forecast record (BD-334.S1, S2): a synthetic record in a scratch directory is
# hashed, stamped by a throwaway local RFC 3161 authority and verified, then each planted defect must be refused.
# The authority is `test_tsa`: it shows the commands work, never a record's time. No network, no corpus, no
# issuer data. The rules are the committed FY2026 rules with the record id renamed to the synthetic record.
#
# Steps:
# - rules: forecast-rules-hash writes rules.sha256, equal to the FMRUL1 frame hashed by the shell;
# - rules_stamp, rules_verify: the rules' digest stamped by the test TSA, and scripts/verify_rules_timestamp.sh
#   reports it verified against the TSA's root, and unverified against its leaf alone;
# - hash: forecast-record-hash writes record.sha256, equal to the FMREC1 frame hashed by the shell;
# - stamp: `openssl ts -query` and `-reply` with the test TSA write record.tsr;
# - verify: forecast-record-verify passes (after 31 Dec 2026 it must refuse instead: any FY2026 proof made then
#   is after the deadline);
# - planted defects, each refused with its own reason: an OPEN marker left in the rules; a forecast changed
#   after hashing; a live site × target with no forecast; a forecast for a not_published site; rules.toml
#   changed after hashing; record.sha256 re-hashed over a changed manifest; a manifest naming another record; a
#   chain that is not the TSA's; a registered file listed with another sha256; an information cut-off after the
#   proof.
#
# Output: one JSON line per step {step, ok, digest, detail}, then {"steps": N, "failures": M}. Exits 1 if any step
# fails. The scratch directory is left in place and named in the first line.
set -u
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/fmine-forecast-record.XXXXXX")"
echo "{\"scratch\":\"$scratch\"}"

steps=0
failures=0
log() { # step ok digest detail
  steps=$((steps + 1))
  [ "$2" = true ] || failures=$((failures + 1))
  printf '{"step":"%s","ok":%s,"digest":"%s","detail":"%s"}\n' "$1" "$2" "$3" \
    "$(printf '%s' "$4" | tr '\n"\\' '   ' | cut -c1-300)"
}
finish() {
  echo "{\"steps\":$steps,\"failures\":$failures}"
  [ "$failures" -eq 0 ] || exit 1
  exit 0
}
if command -v sha256sum >/dev/null 2>&1; then
  sha() { sha256sum | cut -d' ' -f1; }
else
  sha() { shasum -a 256 | cut -d' ' -f1; }
fi
size() { wc -c <"$1" | tr -d ' '; }
xt() { (cd "$root" && cargo run -q -p xtask -- "$@") 2>&1; }
# A refused step: nonzero exit and the expected reason in the output.
refused() { # step expected-substring command...
  local name="$1" want="$2" out rc
  shift 2
  out="$("$@")"
  rc=$?
  if [ "$rc" -ne 0 ] && printf '%s' "$out" | grep -qF -- "$want"; then
    log "$name" true "" "refused: $want"
  else
    log "$name" false "" "exit $rc, wanted a refusal naming '$want': $out"
  fi
}

if ! (cd "$root" && cargo build -q -p xtask); then
  log build false "" "cargo build -p xtask failed"
  finish
fi

# --- a throwaway test TSA: a root and a timeStamping leaf, each valid for two days -------------------------------
tsa="$scratch/test_tsa"
mkdir -p "$tsa"
cat >"$tsa/tsa.cnf" <<'EOF'
[ tsa_config ]
serial = ./serial
crypto_device = builtin
signer_digest = sha256
default_policy = 1.2.3.4.1
other_policies = 1.2.3.4.2
digests = sha256
accuracy = secs:1
ordering = no
tsa_name = no
ess_cert_id_chain = no
ess_cert_id_alg = sha256
[ req ]
distinguished_name = dn
prompt = no
[ dn ]
CN = test_tsa
[ tsa_ext ]
basicConstraints = CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, timeStamping
[ ca_ext ]
basicConstraints = critical, CA:TRUE
keyUsage = critical, keyCertSign, cRLSign
EOF
echo 01 >"$tsa/serial"
if ! (cd "$tsa" &&
  openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.pem -days 2 -subj /CN=test_tsa_root \
    -config tsa.cnf -extensions ca_ext &&
  openssl req -newkey rsa:2048 -nodes -keyout tsa.key -out tsa.csr -subj /CN=test_tsa -config tsa.cnf &&
  openssl x509 -req -in tsa.csr -CA ca.pem -CAkey ca.key -CAcreateserial -out tsa.pem -days 2 \
    -extfile tsa.cnf -extensions tsa_ext) >"$tsa/setup.log" 2>&1; then
  log test_tsa false "" "$(tail -3 "$tsa/setup.log")"
  finish
fi
log test_tsa true "" "throwaway root and timeStamping leaf, labelled test_tsa"
stamp_digest() { # hex out.tsr: an RFC 3161 reply from the test TSA over the digest hex
  (cd "$tsa" &&
    openssl ts -query -digest "$1" -sha256 -cert -out "$2.tsq" &&
    openssl ts -reply -config tsa.cnf -section tsa_config -queryfile "$2.tsq" -inkey tsa.key \
      -signer tsa.pem -out "$2") >"$2.log" 2>&1
}
stamp() { # dir: stamp record.sha256 into record.tsr and install the test chain
  stamp_digest "$(cat "$1/record.sha256")" "$1/record.tsr" && cp "$tsa/ca.pem" "$1/tsa_chain.pem"
}

# --- the rules -----------------------------------------------------------------------------------------------
rec="$scratch/rec"
mkdir -p "$rec"
sed 's/^record_id = "FM-FY2026-v1"$/record_id = "FM-SYNTHETIC-v1"/' "$root/forecasts/fy2026/rules.toml" \
  >"$rec/rules.toml"
out="$(xt forecast-rules-hash "$rec/rules.toml")"
rules_hex="$( { printf 'FMRUL1\n%s\n' "$(size "$rec/rules.toml")"; cat "$rec/rules.toml"; } | sha)"
if [ "$(cat "$rec/rules.sha256" 2>/dev/null)" = "$rules_hex" ]; then
  log rules true "$rules_hex" "rules.sha256 equals the shell's FMRUL1 digest"
else
  log rules false "" "$out"
fi

mkdir -p "$scratch/tsa_root" "$scratch/tsa_leaf"
cp "$tsa/ca.pem" "$scratch/tsa_root/test_tsa.pem"
cp "$tsa/tsa.pem" "$scratch/tsa_leaf/test_tsa_leaf.pem"
if stamp_digest "$rules_hex" "$rec/rules.tsr"; then
  log rules_stamp true "$rules_hex" "rules.tsr from test_tsa"
else
  log rules_stamp false "" "$(tail -3 "$rec/rules.tsr.log")"
fi
out="$(bash "$here/verify_rules_timestamp.sh" "$rec" "$scratch/tsa_root" 2>&1)"
rc=$?
if [ "$rc" -eq 0 ] && printf '%s' "$out" | grep -qF '"verified":true' &&
  printf '%s' "$out" | grep -qF '{"proofs":1,"verified":1}'; then
  log rules_verify true "$rules_hex" "$out"
else
  log rules_verify false "" "exit $rc: $out"
fi
refused rules_verify_wrong_chain '"verified":false' bash "$here/verify_rules_timestamp.sh" "$rec" "$scratch/tsa_leaf"

d="$scratch/open_marker"
mkdir -p "$d"
{ echo '# OPEN: the quantile estimator'; cat "$rec/rules.toml"; } >"$d/rules.toml"
refused open_marker "OPEN point must be decided" xt forecast-rules-hash "$d/rules.toml"

# --- a synthetic record --------------------------------------------------------------------------------------
now="$(date -u +%s)"
label=forecast
[ "$now" -ge 1794222000 ] && label=nowcast # 2026-11-09T11:00:00Z, the Q3 2026 release
write_manifest() { # dir cutoff doc_id sha256 [record_id]
  cat >"$1/manifest.toml" <<EOF
record_id = "${5:-FM-SYNTHETIC-v1}"
author_role = "public-lane-author"
information_cutoff_utc = "$2"
label = "$label"
contact = "pre-contact"
attestation = "No non-public issuer information was used to make these forecasts."
rules_sha256 = "$rules_hex"

[[input]]
doc_id = "$3"
url = "https://example.invalid/synthetic.xlsx"
publication_date = "2026-08-10"
retrieved_utc = "2026-09-01T12:00:00Z"
bytes = 1_524
sha256 = "$4"
EOF
}
ab=abababababababababababababababababababababababababababababababab
write_manifest "$rec" 2026-10-01T00:00:00Z Synthetic_Workbook.xlsx "$ab"
# Invented numbers, one row per live site of the committed rules × target (the record refuses any other set).
{
  printf 'site,target,point,p10,p90,uncalibrated\n'
  awk -F'"' '/^\[\[/{n=""} /^name = /{n=$2} /^status = "live"$/{if(n!="")print n}' "$rec/rules.toml" |
    while IFS= read -r site; do
      printf '%s,production,100,90,110,true\n%s,unit_cost,1500,1350.5,1700,true\n' "$site" "$site"
    done
} >"$rec/forecasts.csv"

out="$(xt forecast-record-hash "$rec")"
record_hex="$( {
  printf 'FMREC1\n%s\n' "$(size "$rec/manifest.toml")"
  cat "$rec/manifest.toml"
  printf '%s\n' "$(size "$rec/forecasts.csv")"
  cat "$rec/forecasts.csv"
} | sha)"
if [ "$(cat "$rec/record.sha256" 2>/dev/null)" = "$record_hex" ]; then
  log hash true "$record_hex" "record.sha256 equals the shell's FMREC1 digest"
else
  log hash false "" "$out"
fi

if stamp "$rec"; then
  log stamp true "$record_hex" "record.tsr from test_tsa"
else
  log stamp false "" "$(tail -3 "$rec/record.tsr.log")"
fi

if [ "$now" -le 1798761599 ]; then # 2026-12-31T23:59:59Z
  out="$(xt forecast-record-verify "$rec")"
  rc=$?
  if [ "$rc" -eq 0 ]; then
    log verify true "$record_hex" "$out"
  else
    log verify false "" "exit $rc: $out"
  fi
else
  refused verify_after_deadline "after the deadline" xt forecast-record-verify "$rec"
fi

# --- planted defects, each in its own copy of the record ------------------------------------------------------
copy() { cp -R "$rec" "$scratch/$1" && printf '%s' "$scratch/$1"; }

d="$(copy flipped_csv)"
sed 's/^Carlin,production,100,/Carlin,production,101,/' "$rec/forecasts.csv" >"$d/forecasts.csv"
refused flipped_csv "hash to" xt forecast-record-verify "$d"

d="$(copy missing_site)"
grep -v '^Kibali,unit_cost,' "$rec/forecasts.csv" >"$d/forecasts.csv"
refused missing_site "no unit_cost forecast for Kibali" xt forecast-record-hash "$d"

d="$(copy sold_site)"
printf 'Hemlo,production,1,1,1,true\n' >>"$d/forecasts.csv"
refused sold_site "not_published" xt forecast-record-hash "$d"

d="$(copy changed_rules)"
printf '# changed after hashing\n' >>"$d/rules.toml"
refused changed_rules "rules_sha256 differs" xt forecast-record-verify "$d"

d="$(copy rehash)"
sed 's/^author_role = .*/author_role = "public-lane-reviewer"/' "$rec/manifest.toml" >"$d/manifest.toml"
refused rehash "already holds" xt forecast-record-hash "$d"

d="$scratch/other_record"
mkdir -p "$d"
cp "$rec/rules.toml" "$rec/forecasts.csv" "$d/"
write_manifest "$d" 2026-10-01T00:00:00Z Synthetic_Workbook.xlsx "$ab" FM-OTHER-v1
refused other_record "is not the rules' record" xt forecast-record-hash "$d"

d="$(copy wrong_chain)"
cp "$tsa/tsa.pem" "$d/tsa_chain.pem"
refused wrong_chain "Verification: FAILED" xt forecast-record-verify "$d"

registered="$(grep -m1 '^file = ' "$root/registries/sources/barrick.toml" | cut -d'"' -f2)"
d="$scratch/registered_sha"
mkdir -p "$d"
cp "$rec/rules.toml" "$rec/forecasts.csv" "$d/"
write_manifest "$d" 2026-10-01T00:00:00Z "$registered" "$ab"
refused registered_sha "differs from the source registry" xt forecast-record-hash "$d"

d="$scratch/cutoff_after_proof"
mkdir -p "$d"
cp "$rec/rules.toml" "$rec/forecasts.csv" "$d/"
write_manifest "$d" "$(date -u -r $((now + 86400)) +%Y-%m-%dT%H:%M:%SZ 2>/dev/null ||
  date -u -d "@$((now + 86400))" +%Y-%m-%dT%H:%M:%SZ)" Synthetic_Workbook.xlsx "$ab"
if (xt forecast-record-hash "$d" >/dev/null) && stamp "$d"; then
  refused cutoff_after_proof "cut-off is not before" xt forecast-record-verify "$d"
else
  log cutoff_after_proof false "" "could not hash and stamp the record"
fi

finish
