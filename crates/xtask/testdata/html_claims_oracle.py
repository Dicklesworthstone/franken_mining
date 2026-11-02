"""Exact expectations for invented HTML filings read by the real fmine executable.

The fixture values and locators are written by hand. These scenarios establish
source binding and CLI behavior; they make no claim about any real issuer filing.
"""

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


ISSUER = "html_test"
MAPPING = "html-claims.toml"
BASE = ["html", "claims", "--issuer", ISSUER, "--definitions", MAPPING]
ORIGINAL = b"""<table>
<tr><th>Site A</th><th>100%</th></tr>
<tr><th>AISC</th><th>2025Q1</th></tr>
<tr><th>US$/oz sold</th><td>10.00</td></tr>
</table>"""
LATER = """<table>
<thead>
<tr><th rowspan="2">Site A</th><th colspan="2">100%</th><th rowspan="2">Unit</th></tr>
<tr><th>2025Q2</th><th>2025Q1</th></tr>
</thead><tbody>
<tr><th>AISC</th><td>9007199254740993.25</td><td>(12.50)</td><td rowspan="2">US$/oz sold</td></tr>
<tr><th>TCC</th><td>—</td><td>N/A</td></tr>
<tr><th>Recovery</th><td></td><td>0</td><td>%</td></tr>
</tbody></table>""".encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def fields(values):
    return "".join(f"{key} = {json.dumps(value, ensure_ascii=False)}\n"
                   for key, value in values.items())


class Fixture:
    def __init__(self, binary):
        self.binary = binary
        # Retain source fixtures for diagnosis; the suite deletes no files.
        self.root = Path(tempfile.mkdtemp(prefix="fmine-html-claims-"))
        (self.root / "registries/sources").mkdir(parents=True)
        (self.root / "corpus").mkdir()
        self.documents = {"Original.html": ORIGINAL, "Later.html": LATER}
        self.mapping = fields({"schema": "fmine.html-claims.v1", "issuer": ISSUER})
        self.add("original", "Original.html", "B3", "A1", "B2", "2025Q1",
                 "A2", "AISC", "A3", "US$/oz sold", "B1")
        for name, cell, period_cell, period, metric_cell, metric, unit_cell, unit in (
            ("large", "B3", "B2", "2025Q2", "A3", "AISC", "D4", "US$/oz sold"),
            ("comparative", "C3", "C2", "2025Q1", "A3", "AISC", "D3", "US$/oz sold"),
            ("nil", "B4", "B2", "2025Q2", "A4", "TCC", "D4", "US$/oz sold"),
            ("not_applicable", "C4", "C2", "2025Q1", "A4", "TCC", "D3", "US$/oz sold"),
            ("missing", "B5", "B2", "2025Q2", "A5", "Recovery", "D5", "%"),
            ("zero", "C5", "C2", "2025Q1", "A5", "Recovery", "D5", "%"),
        ):
            self.add(name, "Later.html", cell, "A2", period_cell, period,
                     metric_cell, metric, unit_cell, unit, "C1")

    def add(self, identity, file, cell, subject, period_cell, period,
            metric_cell, metric, unit_cell, unit, ownership):
        self.mapping += "\n[[claim]]\n" + fields({
            "id": identity, "file": file, "table": 1, "cell": cell,
            "subject_cell": subject, "subject_label": "Site A",
            "period_cell": period_cell, "period_label": period,
            "metric_cell": metric_cell, "metric_label": metric,
            "unit_cell": unit_cell, "unit_label": unit,
            "ownership_cell": ownership, "ownership_label": "100%",
        })

    def write(self, omit=()):
        registry = fields({"schema": "fmine.sources.v1", "issuer": ISSUER,
                           "corpus_dir": "corpus", "retrieved_utc": "2025-08-02T00:00:00Z",
                           "url_prefix": "https://example.invalid/"})
        registry += '\n[[subject]]\nname = "Site A"\n'
        for file, release, published in (("Original.html", "2025Q1", "2025-05-01"),
                                         ("Later.html", "2025Q2", "2025-08-01")):
            data = self.documents[file]
            registry += "\n[[html]]\n" + fields({
                "file": file, "release": release, "url": f"https://example.invalid/{file}",
                "url_basis": "fetched and verified 2025-08-02", "published_at": published,
                "published_at_basis": "FACT: invented fixture publication date",
                "bytes": len(data), "sha256": sha(data),
            })
            if file not in omit:
                (self.root / "corpus" / file).write_bytes(data)
        (self.root / "registries/sources" / f"{ISSUER}.toml").write_text(registry)
        (self.root / MAPPING).write_text(self.mapping)

    def run(self, extra=(), arguments=None):
        return subprocess.run([str(self.binary), *(BASE if arguments is None else arguments),
                               *extra, "--workspace", str(self.root)],
                              capture_output=True, text=True, encoding="utf-8", check=False)

    def invoke(self, extra=(), arguments=None):
        output = self.run(extra, arguments)
        try:
            report = json.loads(output.stdout)
        except ValueError as error:
            raise AssertionError((output.returncode, output.stdout, output.stderr)) from error
        assert report["exit_code"] == output.returncode, output
        return report


def success(report):
    assert report["exit_code"] == 0 and report["refusal"] is None, report
    assert report["anchor_id"] is None and report["certificate"] is None
    assert all(flag is False for flag in report["flags"].values())
    return report["payload"]


def refused(report, code=None):
    assert report["exit_code"] != 0 and report["refusal"] is not None, report
    if code:
        assert report["refusal"]["code"] == code, report


def decimal(value):
    return Fraction(int(value["mantissa"]), 10 ** value["scale"])


class HtmlClaims(unittest.TestCase):
    binary = None

    def fixture(self, omit=()):
        fixture = Fixture(self.binary)
        fixture.write(omit)
        return fixture

    def test_exact_values_periods_comparative_placeholders_and_full_claims(self):
        fixture = self.fixture()
        envelope = fixture.invoke()
        payload = success(envelope)
        self.assertEqual(payload["schema"], "fmine.html_claims.v1")
        self.assertFalse(payload["gate_eligible"])
        self.assertEqual(len(payload["sources"]), 2)
        self.assertEqual(payload["excluded_sources"], [])
        rows = {row["definition_id"]: row for row in payload["claims"]}
        self.assertEqual(len(rows), 7)
        for name, expected in (("large", Fraction(36028797018963973, 4)),
                               ("comparative", Fraction(-25, 2)),
                               ("original", Fraction(10)), ("zero", Fraction(0))):
            row = rows[name]
            self.assertEqual(decimal(row["exact_number"]["value"]), expected)
            self.assertEqual(row["claim"]["value"]["value"]["kind"], "number")
            self.assertEqual(row["claim"]["knowledge"], "Known")
        self.assertEqual(decimal(rows["large"]["exact_number"]["step"]), Fraction(1, 100))
        self.assertEqual(rows["large"]["claim"]["value"]["value"]["stored"], "9007199254740993.25")
        for name, kind in (("nil", "nil"), ("not_applicable", "not_applicable"), ("missing", "missing")):
            self.assertIsNone(rows[name]["exact_number"])
            self.assertEqual(rows[name]["claim"]["value"]["value"]["kind"], kind)
            self.assertEqual(rows[name]["claim"]["knowledge"], "Unknown")
            self.assertIn("Placeholder", rows[name]["claim"]["representation"])
        original = rows["original"]["claim"]["value"]
        comparative = rows["comparative"]["claim"]["value"]
        self.assertEqual(original["period"], comparative["period"])
        self.assertEqual((original["vintage"], comparative["vintage"]), ("2025Q1", "2025Q2"))
        self.assertNotEqual(rows["original"]["claim"]["claim_id"], rows["comparative"]["claim"]["claim_id"])
        for row in rows.values():
            claim = row["claim"]
            self.assertTrue(bytes.fromhex(claim["canonical_body_hex"]))
            self.assertEqual(claim["claim_id"], claim["value"]["claim_id"])
            for field in ("value_tag", "evidence", "epistemic", "knowledge", "representation", "support",
                          "additivity", "confidentiality", "parents", "uu_terms", "supersedes", "stale", "calibration_failed"):
                self.assertIn(field, claim)
            self.assertIn(row["value_source"]["anchor"], claim["anchors"])
            self.assertIn(row["mapping_source"], claim["anchors"])
            self.assertEqual(row["mapping_source"]["sha256"], payload["definitions"]["sha256"])
            self.assertTrue(any(a["sha256"] == payload["registry_sha256"] for a in claim["anchors"]))
            self.assertEqual({meaning["role"] for meaning in row["interpretations"]},
                             {"subject", "period", "metric", "unit", "ownership", "scope"})
            for interpretation in row["interpretations"]:
                self.assertIn(interpretation["source"]["anchor"], claim["anchors"])
        self.assertEqual(envelope, fixture.invoke())
        markdown = fixture.run(["--format", "markdown"])
        self.assertEqual(markdown.returncode, 0, markdown.stderr)
        for row in rows.values():
            self.assertIn(row["claim"]["claim_id"], markdown.stdout)
        for text in ("9007199254740993.25", "Not reported", "Not applicable", "Missing", "Verified label"):
            self.assertIn(text, markdown.stdout)

    def test_merged_label_slots_resolve_to_origins_and_exact_source_byte_ranges(self):
        fixture = self.fixture()
        rows = {row["definition_id"]: row for row in success(fixture.invoke())["claims"]}
        row = rows["large"]
        labels = {meaning["role"]: meaning["source"] for meaning in row["interpretations"]}
        for role, requested, origin in (("subject", "A2", "A1"), ("ownership", "C1", "B1"), ("unit", "D4", "D3")):
            self.assertTrue(labels[role]["requested_locator"].endswith(f"!{requested}"))
            self.assertEqual(labels[role]["anchor"]["locator"], f"Table1!{origin}")
        self.assertEqual(labels["subject"]["row_span"], 2)
        self.assertEqual(labels["ownership"]["column_span"], 2)
        self.assertEqual(labels["unit"]["row_span"], 2)
        span = row["value_source"]["byte_range"]
        self.assertEqual(LATER[span["start"]:span["end_exclusive"]], b"9007199254740993.25")

    def test_publication_cut_precedes_source_reads_and_retains_exclusions(self):
        fixture = self.fixture(omit=("Later.html",))
        payload = success(fixture.invoke(["--as-of", "2025-05-01"]))
        self.assertEqual([row["definition_id"] for row in payload["claims"]], ["original"])
        self.assertEqual([row["file"] for row in payload["sources"]], ["Original.html"])
        self.assertEqual([row["file"] for row in payload["excluded_sources"]], ["Later.html"])
        refused(fixture.invoke(["--as-of", "2025-08-01"]), "FMINE-REF-506")

    def test_changed_bytes_and_changed_source_labels_refuse(self):
        fixture = self.fixture()
        (fixture.root / "corpus/Later.html").write_bytes(LATER.replace(b"AISC", b"TCC!"))
        refused(fixture.invoke(), "FMINE-REF-505")
        fixture.documents["Later.html"] = LATER.replace(b"AISC", b"Changed cost label")
        fixture.write()
        refused(fixture.invoke(), "FMINE-REF-208")

    def test_mapping_and_registry_bytes_bind_the_canonical_interpretation(self):
        fixture = self.fixture()
        before = success(fixture.invoke())
        (fixture.root / MAPPING).write_text(fixture.mapping + "\n# Identical interpretation, new mapping bytes.\n")
        after = success(fixture.invoke())
        self.assertNotEqual(before["definitions"]["sha256"], after["definitions"]["sha256"])
        self.assertNotEqual(before["claims_digest"], after["claims_digest"])
        before_rows = {r["definition_id"]: r for r in before["claims"]}
        for row in after["claims"]:
            previous = before_rows[row["definition_id"]]
            self.assertNotEqual(previous["claim"]["claim_id"], row["claim"]["claim_id"])
            self.assertEqual(previous["exact_number"], row["exact_number"])
        registry = fixture.root / "registries/sources" / f"{ISSUER}.toml"
        registry.write_text(registry.read_text() + "\n# Registered metadata snapshot changed.\n")
        latest = success(fixture.invoke())
        self.assertNotEqual(after["registry_sha256"], latest["registry_sha256"])
        self.assertNotEqual(after["claims_digest"], latest["claims_digest"])

    def test_missing_mapping_label_and_fragmented_numeric_text_refuse(self):
        fixture = self.fixture()
        (fixture.root / MAPPING).write_text(fixture.mapping.replace('ownership_label = "100%"\n', "", 1))
        refused(fixture.invoke())
        fixture.documents["Later.html"] = LATER.replace(b"9007199254740993.25", b"<span>9007199254740993</span><span>.25</span>")
        fixture.write()
        refused(fixture.invoke(), "FMINE-REF-208")

    def test_cli_requires_explicit_unique_selectors_and_valid_dates(self):
        fixture = self.fixture()
        for arguments in (("html", "claims"), ("html", "claims", "--issuer", ISSUER),
                          (*BASE, "--issuer", ISSUER), (*BASE, "--as-of", "2025-02-30"),
                          (*BASE, "--unknown", "x")):
            with self.subTest(arguments=arguments):
                report = fixture.invoke(arguments=arguments)
                self.assertEqual(report["exit_code"], 2)
                self.assertEqual(report["status"], "usage_error")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    options = parser.parse_args()
    HtmlClaims.binary = options.binary.resolve()
    if options.fixture_only:
        fixture = Fixture(HtmlClaims.binary)
        fixture.write()
        print(fixture.root)
    else:
        unittest.main(argv=[__file__])
