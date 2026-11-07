"""HTML disclosure-lint cases over invented filings and independently stated decimal expectations."""

import argparse
import importlib.util
import json
from pathlib import Path
import unittest


SPEC = importlib.util.spec_from_file_location(
    "html_claims_fixture", Path(__file__).with_name("html_claims_oracle.py")
)
FIXTURES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(FIXTURES)
ISSUER, MAPPING = FIXTURES.ISSUER, FIXTURES.MAPPING
BASE = ["lint", "draft.md", "--issuer", ISSUER, "--html-definitions", MAPPING]


class Fixture(FIXTURES.Fixture):
    def __init__(self, binary):
        super().__init__(binary)
        later = self.documents["Later.html"].replace("<td>—</td>".encode(), b"<td>8.50</td>")
        later = later.replace(b"<th>Recovery</th><td></td>", b"<th>Recovery</th><td>91.5</td>")
        later = later.replace(b"</tbody>", b"""<tr><th>Head grade</th><td>2.50</td><td>2.40</td><td>g/t</td></tr>
<tr><th>Gold-equivalent production</th><td>100.0</td><td>99.0</td><td>koz</td></tr>
<tr><th>Gold production</th><td>120.50</td><td>119.00</td><td>koz</td></tr>
<tr><th>Gold sales</th><td>118.75</td><td>116.25</td><td>koz</td></tr>
<tr><th>Copper production</th><td>12.340</td><td>11.125</td><td>kt</td></tr>
<tr><th>Copper C1 cash costs</th><td>1.75</td><td>1.70</td><td>US$/lb sold</td></tr>
<tr><th>Copper AISC</th><td>2.25</td><td>2.20</td><td>US$/lb sold</td></tr>
<tr><th>Gold cost of sales</th><td>890.25</td><td>875.20</td><td>US$/oz sold</td></tr>
<tr><th>Copper cost of sales</th><td>2.05</td><td>1.95</td><td>US$/lb sold</td></tr></tbody>""")
        self.documents["Later.html"] = later
        self.mapping = self.mapping.replace('id = "nil"', 'id = "tcc"')
        self.add("grade", "Later.html", "B6", "A2", "B2", "2025Q2",
                 "A6", "Head grade", "D6", "g/t", "C1")
        self.add("geo", "Later.html", "B7", "A2", "B2", "2025Q2",
                 "A7", "Gold-equivalent production", "D7", "koz", "C1")
        for row, identity, metric, unit in (
            (8, "gold_production", "Gold production", "koz"),
            (9, "gold_sales", "Gold sales", "koz"),
            (10, "copper_production", "Copper production", "kt"),
            (11, "c1", "Copper C1 cash costs", "US$/lb sold"),
            (12, "copper_aisc", "Copper AISC", "US$/lb sold"),
            (13, "gold_cos", "Gold cost of sales", "US$/oz sold"),
            (14, "copper_cos", "Copper cost of sales", "US$/lb sold"),
        ):
            self.add(identity, "Later.html", f"B{row}", "A2", "B2", "2025Q2",
                     f"A{row}", metric, f"D{row}", unit, "C1")

    def projection(self, cut=None):
        report = self.invoke(extra=() if cut is None else ("--as-of", cut))
        return FIXTURES.success(report)

    def lint(self, draft, extra=()):
        (self.root / "draft.md").write_text(draft, encoding="utf-8")
        return self.invoke(extra=extra, arguments=BASE)


def by_id(projection):
    return {row["definition_id"]: row for row in projection["claims"]}


def claim_id(row):
    return row["claim"]["claim_id"]


def sentence(row, value, metric="AISC", period="Q2 2025", credit="", cited=True):
    amount = f"[{value}](fmine:{claim_id(row)})" if cited else value
    return f"Site A {period} 100% basis {credit}{metric} is {amount}.\n"


def success(envelope):
    payload = FIXTURES.success(envelope)
    assert payload["schema"] == "fmine.lint_html_snapshot.v1", payload
    assert payload["source_state_root"] is None
    assert payload["read_mode"] == "verified_html_source_projection"
    return payload


def findings(payload):
    return payload["lint"]["findings"]


class HtmlLint(unittest.TestCase):
    binary = None

    def fixture(self, omit=()):
        fixture = Fixture(self.binary)
        fixture.write(omit)
        return fixture

    def test_exact_cited_equalities_preserve_full_claims_labels_and_private_draft(self):
        fixture = self.fixture()
        projection = fixture.projection()
        rows = by_id(projection)
        draft = (sentence(rows["large"], "US$9007199254740993.25/oz", "AISC per ounce sold")
                 + sentence(rows["grade"], "2.50 g/t", "processed grade")
                 + sentence(rows["missing"], "91.5%", "recovery")
                 + "PRIVATE_DRAFT_SENTENCE_MUST_NOT_APPEAR.\n")
        envelope = fixture.lint(draft)
        payload = success(envelope)
        self.assertEqual([f["verdict"] for f in findings(payload)], ["verified"] * 3)
        self.assertEqual(payload["lint"]["rules_version"], "cited-equality/7")
        self.assertEqual(payload["html_projection"], projection)
        self.assertEqual(payload["lint"]["claims_digest"], projection["claims_digest"])
        self.assertEqual(findings(payload)[0]["source_claim"]["claim_id"], claim_id(rows["large"]))
        text = json.dumps(envelope)
        self.assertNotIn("PRIVATE_DRAFT_SENTENCE", text)
        self.assertIn("Table1!A1", text)
        self.assertIn("mapping_source", text)
        self.assertEqual(fixture.lint(draft), envelope)
        rendered = fixture.run(("--format", "markdown"), arguments=BASE)
        self.assertEqual(rendered.returncode, 0, rendered.stderr)
        self.assertIn("verified", rendered.stdout)
        self.assertIn("HTML source publication evidence", rendered.stdout)
        self.assertIn("Mapping row:", rendered.stdout)
        self.assertNotIn("PRIVATE_DRAFT_SENTENCE", rendered.stdout)
        self.assertEqual((fixture.root / "draft.md").read_text(), draft)

    def test_displayed_interval_mismatch_and_excess_precision_are_distinct(self):
        fixture = self.fixture()
        row = by_id(fixture.projection("2025-05-01"))["original"]
        draft = "".join(sentence(row, amount, "AISC per ounce sold", "Q1 2025")
                        for amount in ("US$10.10/oz", "US$10.000/oz", "US$10.0/oz"))
        payload = success(fixture.lint(draft, ("--as-of", "2025-05-01")))
        self.assertEqual([f["verdict"] for f in findings(payload)],
                         ["value_mismatch", "rounding_beyond_tolerance", "verified"])

    def test_gold_and_copper_production_sales_and_costs_use_exact_source_claims_in_both_modes(self):
        fixture = self.fixture()
        projection = fixture.projection()
        rows = by_id(projection)
        cases = (
            ("gold_production", "120.50 koz", "gold production", "html:gold_production_koz"),
            ("gold_sales", "118.75 koz", "gold sales", "html:gold_sales_koz"),
            ("copper_production", "12.340 kt", "copper production", "html:copper_production_kt"),
            ("c1", "US$1.75/lb", "C1 per pound sold", "html:c1_usd_per_lb_sold"),
            ("copper_aisc", "US$2.25/lb", "AISC per pound sold", "html:aisc_usd_per_lb_sold"),
            ("gold_cos", "US$890.25/oz", "cost of sales per ounce sold", "html:cos_usd_per_oz_sold"),
            ("copper_cos", "US$2.05/lb", "cost of sales per pound sold", "html:cos_usd_per_lb_sold"),
        )
        for cited in (True, False):
            draft = "".join(sentence(rows[identity], amount, metric, cited=cited)
                            for identity, amount, metric, _ in cases)
            payload = success(fixture.lint(draft, () if cited else ("--discover",)))
            self.assertEqual([f["verdict"] for f in findings(payload)], ["verified"] * len(cases))
            self.assertEqual(payload["html_projection"], projection)
            for finding, (identity, _, _, key) in zip(findings(payload), cases):
                expected = rows[identity]
                self.assertEqual(finding["mode"], "cited" if cited else "discovered")
                self.assertEqual(finding["source_claim"]["claim_id"], claim_id(expected))
                self.assertEqual(finding["source_claim"]["line"], key)
                self.assertIn(expected["value_source"]["anchor"], expected["claim"]["anchors"])
                self.assertIn(expected["mapping_source"], expected["claim"]["anchors"])

    def test_new_families_keep_metal_denominator_ownership_and_precision_errors_visible(self):
        fixture = self.fixture()
        rows = by_id(fixture.projection())
        cases = (
            ("gold_production", "120.50 koz", "copper production", "basis_mismatch"),
            ("copper_production", "12.340 kt", "gold production", "basis_mismatch"),
            ("c1", "US$1.75/lb", "C1 per ounce sold", "basis_mismatch"),
            ("copper_aisc", "US$2.25/oz", "AISC per ounce sold", "basis_mismatch"),
            ("gold_cos", "US$890.25/oz", "cost of sales per ounce produced", "basis_mismatch"),
            ("gold_sales", "118.750 koz", "gold sales", "rounding_beyond_tolerance"),
            ("copper_production", "12.3400 kt", "copper production", "rounding_beyond_tolerance"),
            ("copper_cos", "US$2.15/lb", "cost of sales per pound sold", "value_mismatch"),
        )
        draft = "".join(sentence(rows[identity], amount, metric) for identity, amount, metric, _ in cases)
        payload = success(fixture.lint(draft))
        self.assertEqual([f["verdict"] for f in findings(payload)], [case[3] for case in cases])
        draft = sentence(rows["gold_sales"], "118.75 koz", "gold sales").replace("100% basis", "attributable")
        self.assertEqual(findings(success(fixture.lint(draft)))[0]["verdict"], "basis_mismatch")

    def test_credit_treatment_is_never_inferred_in_cited_or_discovery_modes(self):
        fixture = self.fixture()
        rows = by_id(fixture.projection())
        for cited in (True, False):
            draft = "".join(
                sentence(rows[identity], amount, metric + " per ounce sold", credit=credit, cited=cited)
                for identity, amount, metric in (("large", "US$9007199254740993.25/oz", "AISC"),
                                                 ("tcc", "US$8.50/oz", "TCC"))
                for credit in ("by-product ", "co-product ")
            )
            payload = success(fixture.lint(draft, () if cited else ("--discover",)))
            self.assertEqual(len(findings(payload)), 4)
            for finding in findings(payload):
                self.assertEqual(finding["verdict"], "unchecked", finding)
                self.assertEqual(finding["reason"], "source_credit_basis_unstated", finding)
                self.assertIsNotNone(finding["source_claim"])
        no_credit = success(fixture.lint(sentence(rows["tcc"], "US$8.50/oz", "TCC per ounce sold")))
        self.assertEqual(findings(no_credit)[0]["verdict"], "ambiguous")
        for cited in (True, False):
            draft = "".join(
                sentence(rows[identity], amount, metric + " per pound sold", credit=credit, cited=cited)
                for identity, amount, metric in (("copper_aisc", "US$2.25/lb", "AISC"),
                                                 ("c1", "US$1.75/lb", "C1"))
                for credit in ("by-product ", "co-product ")
            )
            payload = success(fixture.lint(draft, () if cited else ("--discover",)))
            self.assertEqual(len(findings(payload)), 4)
            for finding in findings(payload):
                self.assertEqual((finding["verdict"], finding["reason"]),
                                 ("unchecked", "source_credit_basis_unstated"))
                self.assertIsNotNone(finding["source_claim"])

    def test_geo_remains_outside_the_supported_equality_population(self):
        fixture = self.fixture()
        row = by_id(fixture.projection())["geo"]
        payload = success(fixture.lint(sentence(row, "100.0 koz", "gold-equivalent production")))
        self.assertTrue(findings(payload))
        self.assertTrue(all(f["verdict"] != "verified" for f in findings(payload)))

    def test_future_missing_sources_are_filtered_before_reads_and_citations_stay_unresolved(self):
        reference = self.fixture()
        rows = by_id(reference.projection())
        fixture = self.fixture(omit=("Later.html",))
        draft = (sentence(rows["original"], "US$10.00/oz", "AISC per ounce sold", "Q1 2025")
                 + sentence(rows["large"], "US$9007199254740993.25/oz", "AISC per ounce sold")
                 + sentence(rows["copper_production"], "12.340 kt", "copper production")
                 + sentence(rows["c1"], "US$1.75/lb", "C1 per pound sold"))
        payload = success(fixture.lint(draft, ("--as-of", "2025-05-01")))
        self.assertEqual([f["verdict"] for f in findings(payload)], ["verified"] + ["ambiguous"] * 3)
        self.assertEqual(findings(payload)[1]["reason"], "claim_not_unique_in_snapshot")
        self.assertEqual([s["file"] for s in payload["html_projection"]["excluded_sources"]], ["Later.html"])
        dates = payload["source_publication"]
        self.assertFalse(dates["excluded_source_bytes_read"])
        self.assertEqual(dates["mapping_historical_availability"], "not_established")
        self.assertEqual(dates["registry_historical_availability"], "not_established")
        self.assertEqual(dates["findings"][0]["sources"][0]["published_at"], "2025-05-01")
        self.assertEqual(dates["findings"][1]["status"], "no_source_resolved_in_selected_population")
        self.assertFalse((fixture.root / "corpus/Later.html").exists())
        FIXTURES.refused(fixture.lint(draft), "FMINE-REF-506")
        FIXTURES.refused(fixture.lint(draft, ("--as-of", "2025-08-01")), "FMINE-REF-506")

    def test_an_entirely_excluded_selection_does_not_open_any_sources(self):
        reference = self.fixture()
        row = by_id(reference.projection())["original"]
        fixture = self.fixture(omit=("Original.html", "Later.html"))
        payload = success(fixture.lint(sentence(row, "US$10.00/oz", "AISC per ounce sold", "Q1 2025"),
                                       ("--as-of", "2025-04-30")))
        self.assertEqual(payload["html_projection"]["claims"], [])
        self.assertEqual(len(payload["html_projection"]["excluded_sources"]), 2)
        self.assertNotEqual(findings(payload)[0]["verdict"], "verified")

    def test_vintages_remain_stale_or_ambiguous_without_selecting_by_amount(self):
        fixture = self.fixture()
        rows = by_id(fixture.projection())
        draft = sentence(rows["original"], "US$10.00/oz", "AISC per ounce sold", "Q1 2025")
        payload = success(fixture.lint(draft))
        self.assertEqual(findings(payload)[0]["verdict"], "stale")
        self.assertEqual(findings(payload)[0]["later_claims"][0]["claim_id"], claim_id(rows["comparative"]))
        uncited = sentence(rows["original"], "US$10.00/oz", "AISC per ounce sold", "Q1 2025", cited=False)
        ambiguous = success(fixture.lint(uncited, ("--discover",)))
        self.assertEqual(findings(ambiguous)[0]["verdict"], "ambiguous")
        self.assertEqual({c["claim_id"] for c in findings(ambiguous)[0]["candidate_claims"]},
                         {claim_id(rows["original"]), claim_id(rows["comparative"])})
        earlier = success(fixture.lint(uncited, ("--discover", "--as-of", "2025-05-01")))
        self.assertEqual(findings(earlier)[0]["verdict"], "verified")

    def test_changed_source_bytes_refuse_and_changed_mapping_never_retargets_old_ids(self):
        fixture = self.fixture()
        row = by_id(fixture.projection())["grade"]
        draft = sentence(row, "2.50 g/t", "processed grade")
        before = success(fixture.lint(draft))
        path = fixture.root / MAPPING
        path.write_text("# changed mapping bytes\n" + fixture.mapping)
        after = success(fixture.lint(draft))
        self.assertNotEqual(before["html_projection"]["definitions"]["sha256"],
                            after["html_projection"]["definitions"]["sha256"])
        self.assertNotEqual(before["lint"]["claims_digest"], after["lint"]["claims_digest"])
        self.assertEqual(findings(after)[0]["reason"], "claim_not_unique_in_snapshot")
        path.write_text(fixture.mapping.replace('metric_label = "Head grade"', 'metric_label = "Processed grade"'))
        FIXTURES.refused(fixture.lint(draft), "FMINE-REF-208")
        fixture.write()
        source = fixture.root / "corpus/Later.html"
        source.write_bytes(source.read_bytes() + b" ")
        FIXTURES.refused(fixture.lint(draft), "FMINE-REF-505")

    def test_html_selection_does_not_load_unrelated_missing_workbooks(self):
        fixture = self.fixture()
        registry = fixture.root / "registries/sources" / f"{ISSUER}.toml"
        registry.write_text(registry.read_text() + '\n[[source]]\nfile = "Absent.xlsx"\nkind = "mine_stats"\n'
                            'quarter = "2025Q1"\npath = "Absent.xlsx"\nurl_basis = "W1 table"\nbytes = 10\n'
                            f'sha256 = "{"a" * 64}"\n')
        row = by_id(fixture.projection())["grade"]
        payload = success(fixture.lint(sentence(row, "2.50 g/t", "processed grade")))
        self.assertEqual(findings(payload)[0]["verdict"], "verified")
        self.assertFalse((fixture.root / "corpus/Absent.xlsx").exists())

    def test_identical_source_bytes_in_distinct_registered_files_are_refused(self):
        fixture = self.fixture()
        alias = "Alias.html"
        data = fixture.documents["Later.html"]
        (fixture.root / "corpus" / alias).write_bytes(data)
        registry = fixture.root / "registries/sources" / f"{ISSUER}.toml"
        registry.write_text(registry.read_text() + "\n[[html]]\n" + FIXTURES.fields({
            "file": alias, "release": "2025Q2", "url": f"https://example.invalid/{alias}",
            "url_basis": "fetched and verified 2025-08-02", "published_at": "2025-08-02",
            "published_at_basis": "FACT: distinct invented fixture publication date",
            "bytes": len(data), "sha256": FIXTURES.sha(data),
        }))
        (fixture.root / MAPPING).write_text(fixture.mapping.replace(
            'id = "grade"\nfile = "Later.html"', 'id = "grade"\nfile = "Alias.html"'))
        draft = "Site A Q2 2025 100% basis processed grade is 2.50 g/t.\n"
        for envelope in (fixture.invoke(), fixture.lint(draft, ("--discover",))):
            FIXTURES.refused(envelope, "FMINE-REF-506")
            self.assertIn(f"duplicate sha256 {FIXTURES.sha(data)}", envelope["refusal"]["what"])

    def test_in_workspace_renamed_symlink_keeps_registered_publication_evidence(self):
        fixture = self.fixture()
        registered = fixture.root / "corpus/Later.html"
        target = fixture.root / "corpus/actual.html"
        registered.rename(target)
        registered.symlink_to(target.name)
        projection = fixture.projection("2025-08-01")
        row = by_id(projection)["grade"]
        self.assertEqual(row["value_source"]["anchor"]["artifact_path"], "corpus/actual.html")
        for cited in (True, False):
            draft = sentence(row, "2.50 g/t", "processed grade", cited=cited)
            flags = ("--as-of", "2025-08-01") + (() if cited else ("--discover",))
            payload = success(fixture.lint(draft, flags))
            self.assertEqual(findings(payload)[0]["verdict"], "verified")
            self.assertEqual(payload["html_projection"], projection)
            evidence = payload["source_publication"]["findings"][0]["sources"][0]
            self.assertEqual(evidence["claim_id"], claim_id(row))
            self.assertEqual(evidence["file"], "Later.html")
            self.assertEqual(evidence["url"], "https://example.invalid/Later.html")
            self.assertEqual(evidence["sha256"], FIXTURES.sha(fixture.documents["Later.html"]))
            self.assertEqual(evidence["published_at"], "2025-08-01")
            self.assertEqual(evidence["published_at_basis"], "FACT: invented fixture publication date")
            self.assertEqual(evidence["availability"], "published_by_cut_under_current_registry")
        self.assertTrue(registered.is_symlink())
        self.assertEqual(target.read_bytes(), fixture.documents["Later.html"])

    def test_html_selection_requires_unique_compatible_flags(self):
        fixture = self.fixture()
        (fixture.root / "draft.md").write_text("No amount.")
        for arguments in (
            ("lint", "draft.md", "--html-definitions", MAPPING),
            (*BASE, "--through", "2025Q2"), (*BASE, "--synthetic"),
            (*BASE, "--html-definitions", MAPPING), (*BASE, "--as-of", "2025-02-30"),
            (*BASE, "--as-of", "2025-05-01", "--as-of", "2025-08-01"),
            (*BASE, "--discover", "--discover"),
            (*BASE, "--issuer", ISSUER), ("doctor", "--html-definitions", MAPPING),
            ("lint", "draft.md", "--issuer", ISSUER, "--html-definitions"),
        ):
            with self.subTest(arguments=arguments):
                envelope = fixture.invoke(arguments=arguments)
                self.assertEqual(envelope["exit_code"], 2, envelope)
                self.assertEqual(envelope["status"], "usage_error", envelope)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    options = parser.parse_args()
    HtmlLint.binary = options.binary.resolve()
    if options.fixture_only:
        fixture = Fixture(HtmlLint.binary)
        fixture.write()
        print(fixture.root)
    else:
        unittest.main(argv=[__file__])
