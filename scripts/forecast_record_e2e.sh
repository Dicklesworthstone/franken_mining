#!/usr/bin/env bash
# End-to-end test of the public forecast record (BD-334.S1): a synthetic record in a scratch directory is
# hashed, stamped by a throwaway local RFC 3161 authority and verified, then each planted defect must be refused.
# The authority is `test_tsa`: it shows the commands work, never a record's time. No network, no corpus, no
# issuer data.
#
# Steps:
# - rules: forecast-rules-hash writes rules.sha256, equal to the FMRUL1 frame hashed by the shell;
# - hash: forecast-record-hash writes record.sha256, equal to the FMREC1 frame hashed by the shell;
# - stamp: `openssl ts -query` and `-reply` with the test TSA write record.tsr;
# - verify: forecast-record-verify passes (after 31 Dec 2026 it must refuse instead: any FY2026 proof made then
#   is after the deadline);
# - planted defects, each refused with its own reason: a byte added to forecasts.csv; rules.toml changed after
#   hashing; record.sha256 re-hashed over a changed manifest; a chain that is not the TSA's; a registered file
#   listed with another sha256; an information cut-off after the proof.
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
stamp() { # dir: stamp record.sha256 into record.tsr and install the test chain
  (cd "$tsa" &&
    openssl ts -query -digest "$(cat "$1/record.sha256")" -sha256 -cert -out "$1/request.tsq" &&
    openssl ts -reply -config tsa.cnf -section tsa_config -queryfile "$1/request.tsq" -inkey tsa.key \
      -signer tsa.pem -out "$1/record.tsr") >"$1/stamp.log" 2>&1 && cp "$tsa/ca.pem" "$1/tsa_chain.pem"
}

# --- a synthetic record --------------------------------------------------------------------------------------
rec="$scratch/rec"
mkdir -p "$rec"
cat >"$rec/rules.toml" <<'EOF'
# Synthetic rules for the end-to-end test of BD-334.S1; not the FY2026 rules.
[[scoring]]
rule = "interval score, alpha = 0.2"
EOF
out="$(xt forecast-rules-hash "$rec/rules.toml")"
rules_hex="$( { printf 'FMRUL1\n%s\n' "$(size "$rec/rules.toml")"; cat "$rec/rules.toml"; } | sha)"
if [ "$(cat "$rec/rules.sha256" 2>/dev/null)" = "$rules_hex" ]; then
  log rules true "$rules_hex" "rules.sha256 equals the shell's FMRUL1 digest"
else
  log rules false "" "$out"
fi

now="$(date -u +%s)"
label=forecast
[ "$now" -ge 1794222000 ] && label=nowcast # 2026-11-09T11:00:00Z, the Q3 2026 release
write_manifest() { # dir cutoff doc_id sha256
  cat >"$1/manifest.toml" <<EOF
record_id = "FM-SYNTHETIC-v1"
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
printf 'site,target,point,p10,p90,uncalibrated\nSynthetic A,production_koz,100,90,110,true\n' >"$rec/forecasts.csv"

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
  log stamp false "" "$(tail -3 "$rec/stamp.log")"
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
printf 'Synthetic B,production_koz,1,1,1,true\n' >>"$d/forecasts.csv"
refused flipped_csv "hash to" xt forecast-record-verify "$d"

d="$(copy changed_rules)"
printf '# changed after hashing\n' >>"$d/rules.toml"
refused changed_rules "rules_sha256 differs" xt forecast-record-verify "$d"

d="$(copy rehash)"
sed 's/FM-SYNTHETIC-v1/FM-SYNTHETIC-v2/' "$rec/manifest.toml" >"$d/manifest.toml"
refused rehash "already holds" xt forecast-record-hash "$d"

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
