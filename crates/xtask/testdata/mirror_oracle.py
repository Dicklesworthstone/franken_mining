"""Independent JSON/Fraction checks for named, invented BD-346 integration cases.

These expectations come from the printed fixture values and exact interval
arithmetic, never from regenerating expected results with the Rust candidate.
"""

import argparse
from dataclasses import dataclass, replace
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib
import unittest
from fractions import Fraction


def rat(value):
    return Fraction(int(value["num"]), int(value["den"]))


def bounds(value):
    assert value["lo_strict"] is False and value["hi_strict"] is False
    return rat(value["lo"]), rat(value["hi"])


def contains(envelope, expected):
    serialized = json.dumps(envelope)
    for token in expected:
        assert token in serialized, f"missing {token}: {serialized}"


def no_authority(envelope):
    for name in (
        "reserve_disclosure_claim", "execution_authorized",
        "geotechnical_signoff", "regulatory_compliance_claim",
    ):
        assert envelope["flags"][name] is False
    assert all(flag is False for flag in envelope["flags"].values())
    assert envelope["anchor_id"] is None and envelope["certificate"] is None


def positive(x):
    p = x["payload"]
    contains(x, (
        "alpha", "beta", "Left.pdf", "Right.pdf", "615", "385", "1000",
        "61.5", "38.5", "mine-a", "mine-b", "Scope", "Operations",
        "2025-01-01", "2025-12-31",
    ))
    assert p["verdict"]["status"] == "consistent"
    assert p["verdict"]["independently_checked"] is True
    assert p["gate_eligible"] is False and p["jv_sensitive"] is True
    assert len(p["sources"]) == 2 and len(p["inputs"]) == 3 and len(p["normalized"]) == 3
    assert bounds(p["common"]["interval_koz"]) == (Fraction(1999, 2), Fraction(2001, 2))
    shares = {s["issuer"]: rat(s["ownership_claim"]["fraction"]) for s in p["sources"]}
    assert shares == {"alpha": Fraction(123, 200), "beta": Fraction(77, 200)}
    for source in p["sources"]:
        ownership = source["ownership_claim"]
        assert any("ownership_percent" in a["locator"] for a in ownership["anchors"])
        assert any(":p2:Scope:Operations/2025" in a["locator"] for a in ownership["anchors"])
        assert source["document"]["published_at"] == "2026-01-15"
        assert source["document"]["published_at_basis"] == "FACT: invented fixture publication date"
        assert all(r["readers"] == ["reader_a", "reader_b"] for r in source["readings"])
    for source in p["inputs"]:
        normalized = next(n for n in p["normalized"] if n["source"] == source["source"])
        factor = rat(source["ownership"]["share"]) if source["ownership"]["kind"] == "attributable" else 1
        assert bounds(normalized["interval_koz"]) == tuple(v / factor for v in bounds(source["interval_koz"]))
        if source["ownership"]["kind"] == "attributable":
            equity = next(s["ownership_claim"] for s in p["sources"] if s["issuer"] == source["reporter"])
            assert equity["claim_id"] in source["parents"]
            assert all(a in source["anchors"] for a in equity["anchors"])
    witness = {v["cell"]: rat(v["value"]) for v in p["verdict"]["witness"]}
    for cell in p["proof"]["cells"]:
        assert cell["scope"] == "Pdf"
        assert abs(witness[cell["locator"]] - rat(cell["printed"])) <= rat(cell["step"]) / 2
    for identity in p["proof"]["identities"]:
        assert identity["kind"] == "scale"
        assert witness[identity["target"]] == rat(identity["factor"]) * witness[identity["source"]]
    no_authority(x)


def point(x):
    p = x["payload"]
    assert p["verdict"]["status"] == "consistent"
    assert bounds(p["common"]["interval_koz"]) == (Fraction(100), Fraction(100))
    assert p["verdict"]["independently_checked"] is True
    assert len(p["proof"]["cells"]) == 3
    assert all(rat(c["step"]) == 1 for c in p["proof"]["cells"])


def conflict(x):
    p = x["payload"]
    verdict = p["verdict"]
    contains(x, ("conflict", "alpha", "beta", "Left.pdf", "Right.pdf", "400"))
    assert verdict["status"] == "conflict" and verdict["independently_checked"] is True
    assert p["common"] is None
    assert verdict["certificate"]["kind"] in ("farkas", "residual")
    assert len(verdict["certificate_digest"]) == 64
    int(verdict["certificate_digest"], 16)
    assert verdict["blame"]["status"] == "checked"
    assert verdict["blame"]["minimal_sets_exhaustive"] is False
    sets = verdict["minimal_source_sets"]
    assert sets
    intervals = [bounds(c["interval_koz"]) for c in p["normalized"]]
    for source_set in sets:
        assert len(source_set) == 2 and len(set(source_set)) == 2
        assert {p["inputs"][i]["reporter"] for i in source_set} == {"alpha", "beta"}
        assert max(intervals[i][0] for i in source_set) > min(intervals[i][1] for i in source_set)
        for removed in source_set:
            remaining = [i for i in source_set if i != removed]
            assert max(intervals[i][0] for i in remaining) <= min(intervals[i][1] for i in remaining)


def zero(x):
    p = x["payload"]
    assert p["verdict"]["status"] == "consistent"
    assert bounds(p["common"]["interval_koz"]) == (Fraction(0), Fraction(1, 2))
    assert all(bounds(c["interval_koz"]) == (Fraction(-1, 2), Fraction(1, 2)) for c in p["inputs"])
    assert all(bounds(c["interval_koz"]) == (Fraction(-1, 2), Fraction(1, 2)) for c in p["normalized"])
    assert all(rat(c["printed"]) == 0 and rat(c["step"]) == 1 for c in p["proof"]["cells"])
    assert all(rat(w["value"]) >= 0 for w in p["verdict"]["witness"])
    assert p["proof"]["physical_nonnegative"] is True


def third_owner(x):
    p = x["payload"]
    assert p["verdict"]["status"] == "consistent"
    assert bounds(p["common"]["interval_koz"]) == (Fraction(1999, 2), Fraction(2001, 2))
    assert [rat(s["ownership_claim"]["fraction"]) for s in p["sources"]] == [Fraction(9, 20)] * 2


def metadata(x):
    a, b = (e["payload"] for e in x)
    assert a["payload_digest"] != b["payload_digest"]
    assert bounds(a["common"]["interval_koz"]) == bounds(b["common"]["interval_koz"])
    assert a["sources"][1]["document"]["published_at"] == "2026-01-15"
    assert b["sources"][1]["document"]["published_at"] == "2026-01-16"
    assert a["sources"][1]["document"]["sha256"] == b["sources"][1]["document"]["sha256"]


def synthetic(x):
    p = x["payload"]
    assert x["exit_code"] == 0 and p["verdict"]["status"] == "consistent"
    assert p["synthetic"] is True and p["gate_eligible"] is False
    assert all(c["evidence"] == "Assumed" for c in p["inputs"])
    assert all(c["evidence"] == "Assumed" for c in p["normalized"])
    assert p["common"]["evidence"] == "Assumed"
    no_authority(x)


def formats(x):
    assert x["exit_code"] == 0
    assert x["flags"]["regulatory_compliance_claim"] is False
    assert x["payload"]["verdict"]["status"] == "consistent"
    assert bounds(x["payload"]["common"]["interval_koz"]) == (Fraction(1999, 2), Fraction(2001, 2))


CASES = {
    "positive": positive, "point": point, "conflict": conflict, "zero": zero,
    "third_owner": third_owner, "metadata": metadata, "synthetic": synthetic,
    "formats": formats,
}


CONTEXT = (
    "perimeter=group;members=mine-a+mine-b;valid=2025-01-01/2025-12-31;"
    "scope_page=2;scope_table=Scope;scope_row=Operations;scope_col=2025"
)
FULL, ATTR, EQUITY = "gold_produced_100_koz", "gold_produced_attr_koz", "ownership_percent"


@dataclass
class Reading:
    quantity: str
    token: str
    unit: str
    basis: str
    period: str
    row: str
    reader: str


def pair(role, token, mode, unit):
    return [Reading(f"mirror_example_jv_2025fy_{role}", token, unit,
                    f"{mode};{CONTEXT}", "2025FY", role, reader)
            for reader in ("reader_a", "reader_b")]


CAPEX_DEFINITION = (
    ";timing=cash;leases=excluded;definition_page=2;definition_table=Capital policy;"
    "definition_row=Cash capital excluding leases;definition_col=2025"
)


def capex_pair(scope, ownership, token):
    rows = pair(f"capex_{scope}_{ownership}_usd_m", token,
                "100%" if ownership == "100" else "attributable", "USD_m")
    for reading in rows:
        reading.basis += CAPEX_DEFINITION
    return rows


class Document:
    def __init__(self, issuer, file, attributable, share, full):
        self.file = file
        self.bytes = f"%PDF-1.7 invented {issuer} reporting fixture; never parsed as a PDF".encode()
        self.published_at = "2026-01-15"
        self.publication_basis = "FACT: invented fixture publication date"
        self.kind = "partner_filing"
        self.readings = pair(ATTR, attributable, "attributable", "koz")
        self.readings += pair(EQUITY, share, "equity", "percent")
        if full:
            self.readings += pair(FULL, "1000", "100%", "koz")
        self.write_bytes = True

    def token(self, role, value):
        selected = [r for r in self.readings if r.quantity.endswith(role)]
        assert len(selected) == 2, f"change both synthetic readers for {role}"
        for reading in selected:
            reading.token = value

    def change_basis(self, old, new):
        for reading in self.readings:
            assert old in reading.basis, f"fixture basis contains {old}"
            reading.basis = reading.basis.replace(old, new)


class Report(dict):
    def __init__(self, raw):
        super().__init__(json.loads(raw))
        self.raw = raw


def successful(report):
    assert report["exit_code"] == 0, report.raw
    assert report["refusal"] is None, report.raw
    return report


def refused(report, code=None):
    assert report["refusal"] is not None, report.raw
    if code is not None:
        assert report["refusal"]["code"] == code, report.raw
    assert report["exit_code"] != 0, report.raw
    assert '"status": "consistent"' not in json.dumps(report["payload"])


def same(first, second):
    assert first == second and first.raw == second.raw, (first.raw, second.raw)


def replace_argument(arguments, flag, value):
    arguments[arguments.index(flag) + 1] = value


def toml_fields(fields):
    # JSON string escaping is valid for these ASCII TOML fixture values.
    return "".join(f"{key} = {json.dumps(value)}\n" for key, value in fields.items())


class Fixture:
    def __init__(self, binary, tag):
        self.binary = binary
        # Keep evidence files available for diagnosis; the suite deletes no files.
        self.root = Path(tempfile.mkdtemp(prefix=f"fmine-mirror-{tag}-"))
        for directory in ("registries/sources", "registries/transcriptions", "corpus/alpha", "corpus/beta"):
            (self.root / directory).mkdir(parents=True)
        self.left = [Document("alpha", "Left.pdf", "615", "61.5", True)]
        self.right = [Document("beta", "Right.pdf", "385", "38.5", False)]

    def write(self):
        for issuer, documents in (("alpha", self.left), ("beta", self.right)):
            source = toml_fields({"schema": "fmine.sources.v1", "issuer": issuer,
                                  "corpus_dir": f"corpus/{issuer}", "retrieved_utc": "2026-02-01T00:00:00Z",
                                  "url_prefix": f"https://example.invalid/{issuer}/"})
            source += '\n[[subject]]\nname = "Example JV"\n'
            readings = toml_fields({"schema": "fmine.transcriptions.v1", "issuer": issuer})
            for document in documents:
                digest = hashlib.sha256(document.bytes).hexdigest()
                source += "\n[[document]]\n" + toml_fields({
                    "file": document.file, "kind": document.kind,
                    "url": f"https://example.invalid/{issuer}/{document.file}",
                    "url_basis": "fetched and verified 2026-02-01", "published_at": document.published_at,
                    "published_at_basis": document.publication_basis, "pages": 2,
                    "bytes": len(document.bytes), "sha256": digest,
                })
                if document.write_bytes:
                    (self.root / "corpus" / issuer / document.file).write_bytes(document.bytes)
                for reading in document.readings:
                    readings += "\n[[entry]]\n" + toml_fields({
                        "doc": document.file, "doc_sha256": digest, "page": 1,
                        "table": "Invented reported quantities", "row": reading.row, "col": "2025",
                        "token": reading.token, "quantity": reading.quantity, "unit": reading.unit,
                        "basis": reading.basis, "period": reading.period, "reader": reading.reader,
                    })
            (self.root / "registries/sources" / f"{issuer}.toml").write_text(source, encoding="utf-8")
            (self.root / "registries/transcriptions" / f"{issuer}.toml").write_text(readings, encoding="utf-8")

    def arguments(self):
        return ["mirror", "--asset", "example-jv", "--period", "2025", "--as-of", "2026-02-01",
                "--left-issuer", "alpha", "--left-document", self.left[0].file,
                "--right-issuer", "beta", "--right-document", self.right[0].file]

    def run(self, arguments=None):
        arguments = self.arguments() if arguments is None else list(arguments)
        if "--workspace" not in arguments:
            arguments += ["--workspace", str(self.root)]
        return subprocess.run([str(self.binary), *arguments], cwd=self.root.parent,
                              capture_output=True, text=True, encoding="utf-8", check=False)

    def invoke(self, arguments=None):
        result = self.run(arguments)
        try:
            report = Report(result.stdout)
        except (ValueError, TypeError) as error:
            raise AssertionError((result.returncode, result.stdout, result.stderr)) from error
        assert result.returncode == report["exit_code"], (result.stdout, result.stderr)
        return report

    def markdown(self, arguments=None):
        arguments = self.arguments() if arguments is None else list(arguments)
        result = self.run(arguments + ["--format", "markdown"])
        assert result.returncode == 0, (result.stdout, result.stderr)
        return result.stdout


class MirrorIntegration(unittest.TestCase):
    binary = None

    def fixture(self, tag):
        return Fixture(self.binary, tag)

    def capex_fixture(self, tag):
        fixture = self.fixture(tag)
        for document, share, values in (
            (fixture.left[0], "61.5", ("369", "246", "615")),
            (fixture.right[0], "38.5", ("231", "154", "385")),
        ):
            document.readings = pair(EQUITY, share, "equity", "percent")
            for scope, token in zip(("sustaining", "project", "total"), values):
                document.readings += capex_pair(scope, "attr", token)
        for scope, token in (("sustaining", "600"), ("project", "400"), ("total", "1000")):
            fixture.left[0].readings += capex_pair(scope, "100", token)
        return fixture

    def test_capex_scopes_bind_exact_money_and_separate_definition_citations(self):
        fixture = self.capex_fixture("capex-scopes")
        fixture.write()
        for scope, full in (("sustaining", 600), ("project", 400), ("total", 1000)):
            with self.subTest(scope=scope):
                arguments = fixture.arguments() + ["--metric", f"capex-{scope}"]
                report = successful(fixture.invoke(arguments))
                payload = report["payload"]
                self.assertEqual(payload["verdict"]["status"], "consistent")
                self.assertEqual(payload["metric"], f"capex_{scope}_usd_m")
                self.assertEqual(bounds(payload["common"]["interval_usd_m"]),
                                 (Fraction(2 * full - 1, 2), Fraction(2 * full + 1, 2)))
                self.assertNotIn("interval_koz", report.raw)
                self.assertFalse(payload["proof"]["physical_nonnegative"])
                for reading in payload["inputs"]:
                    self.assertEqual(reading["capex_basis"], {
                        "scope": scope, "timing": "cash", "leases": "excluded",
                    })
                    self.assertIn(":p2:Capital policy:Cash capital excluding leases/2025",
                                  reading["basis_source"]["locator"])
                    self.assertIn(reading["basis_source"], reading["anchors"])
                no_authority(report)
                markdown = fixture.markdown(arguments)
                self.assertIn("USD_m", markdown)
                self.assertIn("Cash capital excluding leases", markdown)
                same(report, successful(fixture.invoke(arguments)))

    def test_capex_joint_witness_satisfies_ownership_and_component_sum_in_exact_fractions(self):
        fixture = self.capex_fixture("capex-joint")
        fixture.write()
        arguments = fixture.arguments() + ["--metric", "capex"]
        report = successful(fixture.invoke(arguments))
        payload = report["payload"]
        self.assertEqual(payload["verdict"]["status"], "consistent")
        self.assertEqual(payload["construction"], "component_sum")
        self.assertEqual(len(payload["scope_checks"]), 3)
        self.assertTrue(all(s["construction"] == "independent_half_last_place"
                            for s in payload["scope_checks"]))
        joint = payload["joint"]
        self.assertIsNone(payload["joint_unavailable_reason"])
        witness = {v["cell"]: rat(v["value"]) for v in joint["verdict"]["witness"]}
        sums = [i for i in joint["proof"]["identities"] if i["kind"] == "sum"]
        self.assertEqual(len(sums), 3)
        total_cells = {s["total"] for s in sums}
        for cell in joint["proof"]["cells"]:
            if cell["locator"] not in total_cells:
                self.assertLessEqual(abs(witness[cell["locator"]] - rat(cell["printed"])),
                                     rat(cell["step"]) / 2)
        for identity in joint["proof"]["identities"]:
            if identity["kind"] == "sum":
                self.assertEqual(witness[identity["total"]],
                                 sum(sign * witness[line] for sign, line in
                                     zip(identity["signs"], identity["lines"])))
            else:
                self.assertEqual(identity["kind"], "scale")
                self.assertEqual(witness[identity["target"]],
                                 rat(identity["factor"]) * witness[identity["source"]])
        self.assertIn("Joint component-sum check", fixture.markdown(arguments))
        swapped = list(arguments)
        for flag, value in (("--left-issuer", "beta"), ("--left-document", "Right.pdf"),
                            ("--right-issuer", "alpha"), ("--right-document", "Left.pdf")):
            replace_argument(swapped, flag, value)
        same(report, successful(fixture.invoke(swapped)))
        no_authority(report)

    def test_capex_joint_footing_finds_a_conflict_that_each_scope_alone_accepts(self):
        fixture = self.capex_fixture("capex-footing")
        fixture.left[0].token("capex_total_100_usd_m", "1000.1")
        fixture.write()
        report = successful(fixture.invoke(fixture.arguments() + ["--metric", "capex"]))
        self.assertTrue(all(s["verdict"]["status"] == "consistent"
                            for s in report["payload"]["scope_checks"]))
        verdict = report["payload"]["joint"]["verdict"]
        self.assertEqual(verdict["status"], "conflict")
        self.assertTrue(verdict["independently_checked"])
        self.assertEqual(verdict["certificate"]["kind"], "residual")
        self.assertEqual(abs(rat(verdict["certificate"]["residual"])), Fraction(1, 10))
        self.assertEqual(report["payload"]["verdict"], verdict)

    def test_capex_missing_unstated_or_incompatible_basis_never_becomes_consistent(self):
        for old, new in (
            (";timing=cash", ""), ("timing=cash", "timing=unstated"),
            ("timing=cash", "timing=accrual"), ("leases=excluded", "leases=unstated"),
            ("leases=excluded", "leases=included"),
            (";definition_row=Cash capital excluding leases", ""),
            ("definition_page=2", "definition_page=3"),
        ):
            with self.subTest(new=new):
                fixture = self.capex_fixture("capex-basis")
                for reading in fixture.right[0].readings:
                    if "_capex_" in reading.quantity:
                        self.assertIn(old, reading.basis)
                        reading.basis = reading.basis.replace(old, new)
                fixture.write()
                refused(fixture.invoke(fixture.arguments() + ["--metric", "capex-total"]),
                        "FMINE-REF-104")

    def test_capex_signed_corrections_are_not_clamped_and_source_tampering_refuses(self):
        fixture = self.capex_fixture("capex-signed")
        fixture.left[0].readings = capex_pair("total", "100", "-10")
        fixture.right[0].readings = capex_pair("total", "100", "-10")
        fixture.write()
        arguments = fixture.arguments() + ["--metric", "capex-total"]
        report = successful(fixture.invoke(arguments))
        self.assertEqual(bounds(report["payload"]["common"]["interval_usd_m"]),
                         (Fraction(-21, 2), Fraction(-19, 2)))
        self.assertFalse(report["payload"]["proof"]["physical_nonnegative"])
        (fixture.root / "corpus/beta/Right.pdf").write_bytes(b"changed source")
        refused(fixture.invoke(arguments), "FMINE-REF-505")

    def test_capex_all_scopes_never_silently_omits_a_missing_or_disputed_line(self):
        for scenario in ("missing", "pending", "disputed", "wrong-currency"):
            with self.subTest(scenario=scenario):
                fixture = self.capex_fixture(scenario)
                rows = fixture.right[0].readings
                selected = [r for r in rows if r.quantity.endswith("capex_project_attr_usd_m")]
                if scenario == "missing":
                    fixture.right[0].readings = [r for r in rows if r not in selected]
                elif scenario == "pending":
                    fixture.right[0].readings = [r for r in rows if r is not selected[1]]
                elif scenario == "disputed":
                    selected[1].token = "155"
                else:
                    for r in selected:
                        r.unit = "CAD_m"
                fixture.write()
                refused(fixture.invoke(fixture.arguments() + ["--metric", "capex"]))

    def test_two_document_only_reporters_bind_values_shares_perimeters_and_source_bytes(self):
        fixture = self.fixture("positive")
        fixture.write()
        for issuer in ("alpha", "beta"):
            profile = tomllib.loads((fixture.root / "registries/sources" / f"{issuer}.toml").read_text())
            self.assertFalse(profile.get("source", []))
            self.assertEqual(len(profile["document"]), 1)
        report = successful(fixture.invoke())
        for document in (fixture.left[0], fixture.right[0]):
            self.assertIn(hashlib.sha256(document.bytes).hexdigest(), report.raw)
        positive(report)
        markdown = fixture.markdown()
        same(report, successful(fixture.invoke()))
        self.assertEqual(markdown, fixture.markdown())

    def test_exact_half_step_contact_is_a_consistent_point_not_a_fixed_tolerance(self):
        fixture = self.fixture("rounding-point")
        fixture.left[0].token(FULL, "100")
        fixture.left[0].token(ATTR, "62")
        fixture.right[0].token(ATTR, "38")
        fixture.write()
        point(successful(fixture.invoke()))

    def test_a_conflict_retains_both_reporters_and_a_minimal_source_certificate(self):
        fixture = self.fixture("conflict")
        fixture.right[0].token(ATTR, "400")
        fixture.write()
        conflict(successful(fixture.invoke()))

    def test_zero_keeps_printed_rounding_but_the_common_physical_quantity_is_nonnegative(self):
        fixture = self.fixture("zero")
        fixture.left[0].readings = pair(FULL, "0", "100%", "koz")
        fixture.right[0].readings = pair(FULL, "0", "100%", "koz")
        fixture.write()
        zero(successful(fixture.invoke()))
        fixture.right[0].token(FULL, "-1")
        fixture.write()
        refused(fixture.invoke())

    def test_reporter_shares_need_not_sum_to_one_and_missing_ownership_is_never_inferred(self):
        fixture = self.fixture("third-owner")
        for document in (fixture.left[0], fixture.right[0]):
            document.token(EQUITY, "45")
            document.token(ATTR, "450")
        fixture.write()
        third_owner(successful(fixture.invoke()))
        fixture.right[0].readings = [r for r in fixture.right[0].readings if not r.quantity.endswith(EQUITY)]
        fixture.write()
        refused(fixture.invoke(), "FMINE-REF-104")

    def test_full_mine_assertions_need_no_fabricated_investor_share(self):
        fixture = self.fixture("full-mine")
        fixture.left[0].readings = [r for r in fixture.left[0].readings if r.quantity.endswith(FULL)]
        fixture.right[0].readings = pair(FULL, "1000", "100%", "koz")
        fixture.write()
        report = successful(fixture.invoke())
        self.assertEqual(report["payload"]["verdict"]["status"], "consistent")
        self.assertNotIn(EQUITY, json.dumps(report["payload"]))

    def test_selected_documents_are_hash_checked_and_unselected_documents_are_isolated(self):
        missing = self.fixture("missing-source")
        missing.right[0].write_bytes = False
        missing.write()
        refused(missing.invoke(), "FMINE-REF-506")
        fixture = self.fixture("optional-sources")
        fixture.write()
        original = successful(fixture.invoke())
        markdown = fixture.markdown()
        optional = Document("optional", "Other.pdf", "500", "50", False)
        optional.write_bytes = False
        optional.published_at = "2030-01-15"
        optional.readings[0].token = "501"
        fixture.right.append(optional)
        fixture.write()
        same(original, successful(fixture.invoke()))
        self.assertEqual(markdown, fixture.markdown())
        (fixture.root / "corpus/beta/Other.pdf").write_bytes(b"altered optional document")
        same(original, successful(fixture.invoke()))
        self.assertEqual(markdown, fixture.markdown())
        (fixture.root / "corpus/beta/Right.pdf").write_bytes(b"altered selected document")
        refused(fixture.invoke(), "FMINE-REF-505")

    def test_selected_locator_keeps_every_reader_even_when_the_quantity_key_differs(self):
        fixture = self.fixture("reader-key")
        fixture.right[0].readings.append(replace(fixture.right[0].readings[0], reader="reader_c",
                                                quantity="another_interpretation_of_this_printed_value"))
        fixture.write()
        refused(fixture.invoke(), "FMINE-REF-202")

    def test_one_pending_selected_assertion_cannot_be_dropped_to_obtain_agreement(self):
        fixture = self.fixture("pending-selected")
        fixture.left[0].readings = [r for r in fixture.left[0].readings
                                    if not (r.quantity.endswith(FULL) and r.reader == "reader_b")]
        fixture.write()
        refused(fixture.invoke(), "FMINE-REF-202")

    def test_perimeter_members_and_dates_are_not_coerced_to_a_common_subset(self):
        for tag, old, new in (
            ("different-members", "members=mine-a+mine-b", "members=mine-a+mine-c"),
            ("partial-year", "valid=2025-01-01/2025-12-31", "valid=2025-07-01/2025-12-31"),
            ("expired-scope", "valid=2025-01-01/2025-12-31", "valid=2024-01-01/2024-12-31"),
        ):
            with self.subTest(tag=tag):
                fixture = self.fixture(tag)
                fixture.right[0].change_basis(old, new)
                fixture.write()
                refused(fixture.invoke())

    def test_ownership_units_and_scope_citations_cannot_be_silently_reinterpreted(self):
        for scenario in ("quantity-unit", "share-unit", "share-range", "scope-page"):
            with self.subTest(scenario=scenario):
                fixture = self.fixture(scenario)
                for reading in fixture.right[0].readings:
                    if scenario == "quantity-unit" and reading.quantity.endswith(ATTR):
                        reading.unit = "oz"
                    elif scenario == "share-unit" and reading.quantity.endswith(EQUITY):
                        reading.unit = "fraction"
                    elif scenario == "share-range" and reading.quantity.endswith(EQUITY):
                        reading.token = "38 - 39"
                    elif scenario == "scope-page":
                        reading.basis = reading.basis.replace("scope_page=2", "scope_page=3")
                fixture.write()
                refused(fixture.invoke())

    def test_publication_cut_rejects_future_preperiod_and_unestablished_availability(self):
        for tag, date, basis in (
            ("future", "2026-02-02", "FACT: invented later publication"),
            ("not-realized", "2025-01-01", "FACT: invented date before annual period ends"),
            ("inferred-date", "2026-01-15", "INFERENCE: cover date does not establish availability"),
        ):
            with self.subTest(tag=tag):
                fixture = self.fixture(tag)
                fixture.right[0].published_at = date
                fixture.right[0].publication_basis = basis
                fixture.write()
                refused(fixture.invoke())
        for kind in ("investor_deck", "technical_report"):
            with self.subTest(kind=kind):
                fixture = self.fixture(kind)
                fixture.right[0].kind = kind
                fixture.write()
                refused(fixture.invoke(), "FMINE-REF-104")

    def test_identical_document_names_preserve_both_reporter_sources(self):
        fixture = self.fixture("same-basename")
        fixture.left[0].file = fixture.right[0].file = "AnnualReport.pdf"
        fixture.write()
        report = successful(fixture.invoke())
        for document in (fixture.left[0], fixture.right[0]):
            self.assertIn(hashlib.sha256(document.bytes).hexdigest(), report.raw)
        self.assertIn("alpha", report.raw)
        self.assertIn("beta", report.raw)
        self.assertEqual(report["payload"]["verdict"]["status"], "consistent")

    def test_reporter_and_reader_order_are_canonical_but_source_metadata_changes_the_digest(self):
        fixture = self.fixture("canonical-evidence")
        fixture.write()
        original = successful(fixture.invoke())
        markdown = fixture.markdown()
        swapped = fixture.arguments()
        for flag, value in (("--left-issuer", "beta"), ("--left-document", "Right.pdf"),
                            ("--right-issuer", "alpha"), ("--right-document", "Left.pdf")):
            replace_argument(swapped, flag, value)
        same(original, successful(fixture.invoke(swapped)))
        self.assertEqual(markdown, fixture.markdown(swapped))
        fixture.left[0].readings.reverse()
        fixture.right[0].readings.reverse()
        fixture.write()
        same(original, successful(fixture.invoke()))
        self.assertEqual(markdown, fixture.markdown())
        fixture.right[0].published_at = "2026-01-16"
        fixture.write()
        metadata([original, successful(fixture.invoke())])

    def test_sold_and_half_year_inputs_use_the_same_source_bound_path(self):
        for period, key in (("2025Q1", "2025q1"), ("2025H1", "2025h1"), ("2025H2", "2025h2")):
            with self.subTest(period=period):
                fixture = self.fixture(key)
                for document in (fixture.left[0], fixture.right[0]):
                    for reading in document.readings:
                        reading.period = period
                        reading.quantity = reading.quantity.replace("2025fy", key).replace("gold_produced", "gold_sold")
                        reading.row = reading.row.replace("gold_produced", "gold_sold")
                fixture.write()
                arguments = fixture.arguments()
                replace_argument(arguments, "--period", period)
                arguments += ["--metric", "sold"]
                report = successful(fixture.invoke(arguments))
                self.assertEqual(report["payload"]["verdict"]["status"], "consistent")
                self.assertIn("gold_sold", json.dumps(report["payload"]))
                refused(fixture.invoke(arguments[:-2]))

    def test_explicit_selection_and_synthetic_lane_have_strict_usage_boundaries(self):
        fixture = self.fixture("selectors")
        fixture.write()
        for flag, value in (("--asset", ""), ("--period", "2025Q5"), ("--period", "2025H3"),
                            ("--as-of", "2026-02-30"), ("--left-issuer", "missing_issuer"),
                            ("--right-issuer", "alpha")):
            with self.subTest(flag=flag, value=value):
                arguments = fixture.arguments()
                replace_argument(arguments, flag, value)
                self.assertNotEqual(fixture.invoke(arguments)["exit_code"], 0)
        for extra in (["--synthetic"], ["--metric", "aisc"], ["--through", "2025Q4"]):
            self.assertEqual(fixture.invoke(fixture.arguments() + extra)["exit_code"], 2)
        report = successful(fixture.invoke(["mirror", "--synthetic", "--json"]))
        synthetic(report)
        self.assertIsNone(report["certificate"])
        capex = successful(fixture.invoke(["mirror", "--synthetic", "--metric", "capex"]))
        self.assertTrue(capex["payload"]["synthetic"])
        self.assertEqual(capex["payload"]["verdict"]["status"], "consistent")
        self.assertTrue(all(c["evidence"] == "Assumed" for c in capex["payload"]["joint"]["inputs"]))
        self.assertIn("Joint component-sum check", fixture.markdown(["mirror", "--synthetic", "--metric", "capex"]))
        no_authority(capex)

    def test_json_and_markdown_execute_as_a_real_read_only_command(self):
        fixture = self.fixture("binary")
        fixture.write()
        arguments = fixture.arguments() + ["--workspace", str(fixture.root)]
        formats(successful(fixture.invoke(arguments)))
        markdown = fixture.run(arguments + ["--format", "markdown"])
        self.assertEqual(markdown.returncode, 0, markdown.stderr)
        for expected in ("Left.pdf", "Right.pdf", "615", "385", "mine-a", "mine-b"):
            self.assertIn(expected, markdown.stdout)
        for issuer, document in (("alpha", fixture.left[0]), ("beta", fixture.right[0])):
            self.assertEqual((fixture.root / "corpus" / issuer / document.file).read_bytes(), document.bytes)


if __name__ == "__main__":
    if not __debug__:
        raise RuntimeError("Mirror oracle requires enabled assertions; do not run with -O")
    if len(sys.argv) == 2 and sys.argv[1] in CASES:
        CASES[sys.argv[1]](json.load(sys.stdin))
    else:
        parser = argparse.ArgumentParser(description=__doc__)
        parser.add_argument("--binary", type=Path, required=True)
        MirrorIntegration.binary = parser.parse_args().binary.resolve(strict=True)
        unittest.main(argv=[sys.argv[0]], verbosity=2)
