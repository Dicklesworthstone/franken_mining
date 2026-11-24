"""HTML disclosure-lint cases over invented filings and independently stated decimal expectations."""

import argparse
from fractions import Fraction
import importlib.util
from itertools import product
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
        for identity, amount, metric in (
            ("copper_cos", "US$2.05/lb", "gold cost of sales per pound sold"),
            ("gold_cos", "US$890.25/oz", "copper cost of sales per ounce sold"),
        ):
            draft = sentence(rows[identity], amount, metric)
            self.assertNotEqual(findings(success(fixture.lint(draft)))[0]["verdict"], "verified")
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


class ArithmeticFixture(Fixture):
    """Invented period assertions; expected arithmetic is computed separately with Fraction."""

    def __init__(self, binary, observations):
        super().__init__(binary)
        self.observations = observations
        self.documents = {}
        self.mapping = FIXTURES.fields({"schema": "fmine.html-claims.v1", "issuer": ISSUER})
        for index, row in enumerate(observations):
            file = f"Period{index}.html"
            subject, basis = row.get("subject", "Site A"), row.get("basis", "100%")
            metric, unit = row.get("metric", "Gold production"), row.get("unit", "koz")
            self.documents[file] = (
                f"<table><tr><th>{subject}</th><th>{basis}</th></tr>"
                f"<tr><th>{metric}</th><th>{row['period']}</th></tr>"
                f"<tr><th>{unit}</th><td>{row['value']}</td></tr></table><!--{index}-->"
            ).encode()
            self.mapping += "\n[[claim]]\n" + FIXTURES.fields({
                "id": f"period_{index}", "file": file, "table": 1, "cell": "B3",
                "subject_cell": "A1", "subject_label": subject,
                "period_cell": "B2", "period_label": row["period"],
                "metric_cell": "A2", "metric_label": metric,
                "unit_cell": "A3", "unit_label": unit,
                "ownership_cell": "B1", "ownership_label": basis,
            })

    def write(self, omit=()):
        registry = FIXTURES.fields({
            "schema": "fmine.sources.v1", "issuer": ISSUER, "corpus_dir": "corpus",
            "retrieved_utc": "2031-03-01T00:00:00Z", "url_prefix": "https://example.invalid/",
        })
        for subject in sorted({row.get("subject", "Site A") for row in self.observations}):
            registry += "\n[[subject]]\n" + FIXTURES.fields({"name": subject})
        for index, row in enumerate(self.observations):
            file = f"Period{index}.html"
            year = int(row["period"][:4])
            quarter = {"Q1": 1, "Q2": 2, "Q3": 3, "Q4": 4, "H1": 2, "9M": 3, "FY": 4}[row["period"][4:]]
            date = f"{year + 1}-02-01" if quarter == 4 else f"{year}-{3 * quarter + 2:02d}-01"
            data = self.documents[file]
            registry += "\n[[html]]\n" + FIXTURES.fields({
                "file": file, "release": row.get("release", f"{year}Q{quarter}"),
                "url": f"https://example.invalid/{file}", "url_basis": "fetched and verified 2031-03-01",
                "published_at": row.get("published", date),
                "published_at_basis": "FACT: invented fixture publication date",
                "bytes": len(data), "sha256": FIXTURES.sha(data),
            })
            if file not in omit:
                (self.root / "corpus" / file).write_bytes(data)
        (self.root / "registries/sources" / f"{ISSUER}.toml").write_text(registry)
        (self.root / MAPPING).write_text(self.mapping)


def observation(period, value, **options):
    return {"period": period, "value": value, **options}


def bounds(claim):
    interval = claim["value"]["interval"]
    return tuple(Fraction(int(interval[side]["num"]), int(interval[side]["den"]))
                 for side in ("lo", "hi"))


class HtmlArithmetic(unittest.TestCase):
    binary = None

    def fixture(self, observations, omit=()):
        fixture = ArithmeticFixture(self.binary, observations)
        fixture.write(omit)
        return fixture

    def assessment(self, fixture, target="Q4 2030", value="40.0", metric="gold production",
                   unit="koz", basis="100% basis", flags=()):
        draft = f"Site A {target} {basis} {metric} is {value} {unit}.\n"
        payload = success(fixture.lint(draft, ("--discover", *flags)))
        arithmetic = payload["source_arithmetic"]
        self.assertEqual(arithmetic["schema"], "fmine.html_period_arithmetic.v1")
        self.assertEqual(arithmetic["claims_digest"], payload["html_projection"]["claims_digest"])
        self.assertEqual(arithmetic["draft_sha256"], FIXTURES.sha(draft.encode()))
        return payload, arithmetic["findings"]

    def test_cumulative_differences_and_quarter_sums_match_independent_exact_answers(self):
        for metric, unit in (("Gold production", "koz"), ("Gold sales", "koz"), ("Copper production", "kt")):
            for prior, current, target, rule in (
                ("2030Q1", "2030H1", "Q2 2030", "H1-Q1"),
                ("2030H1", "20309M", "Q3 2030", "9M-H1"),
                ("20309M", "2030FY", "Q4 2030", "FY-9M"),
            ):
                fixture = self.fixture([observation(prior, "100.20", metric=metric, unit=unit),
                                        observation(current, "225.40", metric=metric, unit=unit)])
                payload, rows = self.assessment(fixture, target, "125.2", metric.lower(), unit)
                self.assertEqual(len(rows), 1, payload)
                row = rows[0]
                self.assertEqual((row["status"], row["rule"]), ("derived", rule))
                self.assertEqual(bounds(row["derived_claim"]), (Fraction("125.19"), Fraction("125.21")))
                self.assertEqual(row["derived_claim"]["value"]["unit"], unit)
                self.assertEqual(set(row["derived_claim"]["parents"]), {c["claim_id"] for c in row["inputs"]})
                self.assertEqual({c["parents"][0] for c in row["inputs"]},
                                 {e["claim"]["claim_id"] for e in row["source_evidence"]})
                self.assertEqual(len(row["source_evidence"]), 2)
                for evidence in row["source_evidence"]:
                    self.assertEqual(evidence["claim"]["claim_id"], evidence["publication"]["claim_id"])
                    self.assertTrue(evidence["publication"]["sha256"])
                    self.assertTrue(evidence["publication"]["published_at"])
        for n, target, value in ((2, "H1 2030", "30.0"), (3, "9M 2030", "60.0"), (4, "FY2030", "100.0")):
            fixture = self.fixture([observation(f"2030Q{q}", f"{q * 10}.00") for q in range(1, n + 1)])
            payload, rows = self.assessment(fixture, target, value)
            self.assertEqual(rows[0]["status"], "derived", payload)
            total, radius = Fraction(value), Fraction(n, 200)
            self.assertEqual(bounds(rows[0]["derived_claim"]), (total - radius, total + radius))
            self.assertEqual(rows[0]["rule"], "quarter_sum")

    def test_precision_signed_differences_and_large_decimals_never_round_through_float(self):
        fixture = self.fixture([observation("20309M", "100.00"), observation("2030FY", "140.00")])
        for value, status in (("40.0", "derived"), ("40.00", "precision_not_established"), ("41.0", "contradicted")):
            _, rows = self.assessment(fixture, value=value)
            self.assertEqual(rows[0]["status"], status)
            self.assertEqual(bounds(rows[0]["derived_claim"]), (Fraction("39.99"), Fraction("40.01")))
        for prior, current, expected in (("100.00", "90.00", "-10.0"),
                                          ("9007199254740893.25", "9007199254740993.25", "100.0")):
            fixture = self.fixture([observation("20309M", prior), observation("2030FY", current)])
            _, rows = self.assessment(fixture, value=expected)
            self.assertEqual(rows[0]["status"], "derived")
            value = Fraction(current) - Fraction(prior)
            self.assertEqual(bounds(rows[0]["derived_claim"]), (value - Fraction(1, 100), value + Fraction(1, 100)))

    def test_missing_placeholder_status_and_competing_vintages_remain_unresolved(self):
        base = [observation("20309M", "100.00"), observation("2030FY", "140.00")]
        for rows, expected in ((base[:1], "not_available"),
                               ([base[0], observation("2030FY", "—")], "refused"),
                               ([base[0], observation("2030FY", "")], "refused"),
                               ([*base, observation("20309M", "100.00", release="2030Q4")], "ambiguous"),
                               ([*base, observation("20309M", "90.00", release="2030Q4")], "ambiguous")):
            fixture = self.fixture(rows)
            _, results = self.assessment(fixture)
            self.assertEqual(results[0]["status"], expected)
            self.assertIsNone(results[0]["derived_claim"])
            self.assertEqual(len(results[0]["source_evidence"]), len(rows))
        fixture = self.fixture(base)
        registry = fixture.root / "registries/sources" / f"{ISSUER}.toml"
        registry.write_text(registry.read_text() + '\n[[subject_status]]\nsubject = "Site A"\n'
                            'status = "withheld"\nfirst = "2030Q1"\nlast = "2030Q4"\nevidence = "synthetic status"\n')
        _, rows = self.assessment(fixture)
        self.assertEqual(rows[0]["status"], "refused")
        self.assertEqual(rows[0]["refusal"]["code"], "FMINE-REF-208")

    def test_cutoff_filters_missing_future_files_before_arithmetic_and_retains_ancestry(self):
        fixture = self.fixture([observation("20309M", "100.00"), observation("2030FY", "140.00")],
                               omit=("Period1.html",))
        payload, rows = self.assessment(fixture, flags=("--as-of", "2030-12-01"))
        self.assertEqual(rows[0]["status"], "not_available")
        self.assertEqual(len(rows[0]["source_evidence"]), 1)
        self.assertNotIn("140.00", json.dumps(payload))
        self.assertFalse((fixture.root / "corpus/Period1.html").exists())
        self.assertEqual([s["file"] for s in payload["html_projection"]["excluded_sources"]], ["Period1.html"])
        FIXTURES.refused(fixture.lint("Site A Q4 2030 100% basis gold production is 40.0 koz.",
                                    ("--discover", "--as-of", "2031-02-01")), "FMINE-REF-506")

    def test_source_values_hashes_and_mapping_bytes_bind_results_and_no_draft_prose_leaks(self):
        fixture = self.fixture([observation("20309M", "100.00"), observation("2030FY", "140.00")])
        first, rows = self.assessment(fixture)
        self.assertEqual(first, self.assessment(fixture)[0])
        original_id = rows[0]["derived_claim"]["claim_id"]
        mapping = fixture.root / MAPPING
        mapping.write_text("# new interpretation bytes\n" + fixture.mapping)
        changed, rows = self.assessment(fixture)
        self.assertNotEqual(original_id, rows[0]["derived_claim"]["claim_id"])
        self.assertNotEqual(first["source_arithmetic"]["digest"], changed["source_arithmetic"]["digest"])
        fixture.documents["Period1.html"] = fixture.documents["Period1.html"].replace(b"140.00", b"141.00")
        fixture.write()
        changed, rows = self.assessment(fixture)
        self.assertEqual(rows[0]["status"], "contradicted")
        self.assertEqual(bounds(rows[0]["derived_claim"]), (Fraction("40.99"), Fraction("41.01")))
        self.assertNotEqual(original_id, rows[0]["derived_claim"]["claim_id"])
        source = fixture.root / "corpus/Period1.html"
        source.write_bytes(source.read_bytes() + b" ")
        FIXTURES.refused(fixture.lint("Site A Q4 2030 100% basis gold production is 40.0 koz."), "FMINE-REF-505")
        fixture.write()
        draft = "PRIVATE_UNRELATED_DRAFT_TEXT.\nSite A Q4 2030 100% basis gold production is 41.0 koz.\n"
        envelope = fixture.lint(draft, ("--discover",))
        self.assertNotIn("PRIVATE_UNRELATED_DRAFT_TEXT", json.dumps(envelope))
        rendered = fixture.run(("--discover", "--format", "markdown"), arguments=BASE)
        self.assertEqual(rendered.returncode, 0, rendered.stderr)
        self.assertIn("HTML period arithmetic", rendered.stdout)
        self.assertIn("Derived Claim", rendered.stdout)
        self.assertNotIn("PRIVATE_UNRELATED_DRAFT_TEXT", rendered.stdout)

    def test_incompatible_bases_units_subjects_and_nonadditive_rates_cannot_supply_operands(self):
        for prior, current in ((observation("20309M", "100.00", basis="Attributable (60%)"),
                                observation("2030FY", "140.00", basis="Attributable (80%)")),
                               (observation("2030Q1", "100.00", basis="Attributable (60%)"),
                                observation("2030Q2", "140.00", basis="Attributable (80%)"))):
            fixture = self.fixture([prior, current])
            target = "Q4 2030" if current["period"] == "2030FY" else "H1 2030"
            _, rows = self.assessment(fixture, target=target, basis="attributable")
            self.assertEqual(rows[0]["status"], "refused")
            self.assertEqual(rows[0]["refusal"]["code"], "FMINE-REF-105")
        for changed in (observation("2030FY", "140.00", subject="Site B"),
                        observation("2030FY", "140.00", metric="Copper production", unit="kt"),
                        observation("2030FY", "140.00", metric="Gold sales"),
                        observation("2029FY", "140.00")):
            fixture = self.fixture([observation("20309M", "100.00"), changed])
            _, rows = self.assessment(fixture)
            self.assertEqual(rows[0]["status"], "not_available")
        fixture = self.fixture([observation("2030Q2", "100.00"), observation("20309M", "140.00")])
        _, rows = self.assessment(fixture, target="Q3 2030")
        self.assertEqual(rows[0]["status"], "not_available")
        fixture = self.fixture([observation("20309M", "100.00", metric="AISC", unit="US$/oz sold"),
                                observation("2030FY", "140.00", metric="AISC", unit="US$/oz sold")])
        payload = success(fixture.lint("Site A Q4 2030 100% basis AISC per ounce sold is $40.0/oz.", ("--discover",)))
        self.assertEqual(payload["source_arithmetic"]["findings"][0]["status"], "not_available")
        self.assertIsNone(payload["source_arithmetic"]["findings"][0]["derived_claim"])

    def test_direct_disclosures_and_explicit_bad_citations_are_not_replaced_by_arithmetic(self):
        fixture = self.fixture([observation("20309M", "100.00"), observation("2030FY", "140.00"),
                                observation("2030Q4", "99.00")])
        payload, rows = self.assessment(fixture)
        self.assertEqual(findings(payload)[0]["verdict"], "value_mismatch")
        self.assertEqual(rows, [])
        plain = success(fixture.lint("Site A Q4 2030 100% basis gold production is 40.0 koz."))
        self.assertEqual(plain["source_arithmetic"]["findings"][0]["status"], "direct_assertion_available")
        self.assertIsNone(plain["source_arithmetic"]["findings"][0]["derived_claim"])
        original = by_id(fixture.projection())["period_0"]
        draft = f"Site A Q4 2030 100% basis gold production is [40.0 koz](fmine:{claim_id(original)})."
        bad = success(fixture.lint(draft, ("--discover",)))
        self.assertNotEqual(findings(bad)[0]["verdict"], "verified")
        self.assertEqual(bad["source_arithmetic"]["findings"], [])


class RateFixture(Fixture):
    """One invented filing carries all cumulative operands, with optional later-vintage competition."""

    def __init__(self, binary, observations, copies=1):
        super().__init__(binary)
        self.observations = observations
        self.documents = {}
        self.mapping = FIXTURES.fields({"schema": "fmine.html-claims.v1", "issuer": ISSUER})
        for copy in range(copies):
            file = f"Rates{copy}.html"
            tables = []
            for index, row in enumerate(observations):
                metric, unit = row["metric"], row["unit"]
                basis = row.get("basis", "100%")
                tables.append(
                    f"<table><tr><th>Site A</th><th>{basis}</th></tr>"
                    f"<tr><th>{metric}</th><th>{row['period']}</th></tr>"
                    f"<tr><th>{unit}</th><td>{row['value']}</td></tr></table>"
                )
                self.mapping += "\n[[claim]]\n" + FIXTURES.fields({
                    "id": f"rate_{copy}_{index}", "file": file, "table": index + 1, "cell": "B3",
                    "subject_cell": "A1", "subject_label": "Site A",
                    "period_cell": "B2", "period_label": row["period"],
                    "metric_cell": "A2", "metric_label": metric,
                    "unit_cell": "A3", "unit_label": unit,
                    "ownership_cell": "B1", "ownership_label": basis,
                })
            self.documents[file] = ("".join(tables) + f"<!--{copy}-->").encode()

    def write(self, omit=()):
        registry = FIXTURES.fields({
            "schema": "fmine.sources.v1", "issuer": ISSUER, "corpus_dir": "corpus",
            "retrieved_utc": "2031-03-01T00:00:00Z", "url_prefix": "https://example.invalid/",
        }) + '\n[[subject]]\nname = "Site A"\n'
        for index, (file, data) in enumerate(self.documents.items()):
            registry += "\n[[html]]\n" + FIXTURES.fields({
                "file": file, "release": "2030Q4" if index == 0 else "2031Q1",
                "url": f"https://example.invalid/{file}", "url_basis": "fetched and verified 2031-03-01",
                "published_at": "2031-02-01" if index == 0 else "2031-03-01",
                "published_at_basis": "FACT: invented fixture publication date",
                "bytes": len(data), "sha256": FIXTURES.sha(data),
            })
            if file not in omit:
                (self.root / "corpus" / file).write_bytes(data)
        (self.root / "registries/sources" / f"{ISSUER}.toml").write_text(registry)
        (self.root / MAPPING).write_text(self.mapping)


def rate_rows(prior="20309M", current="2030FY", metric="AISC", values=None, copper=False):
    values = values or ("100.00", "100.000", "120.00", "200.000")
    sales = "Copper sales" if copper else "Gold sales"
    rate_unit, sales_unit = ("US$/lb sold", "Mlb") if copper else ("US$/oz sold", "koz")
    return [observation(period, value, metric=label, unit=unit)
            for period, label, unit, value in zip(
                (prior, prior, current, current),
                (metric, sales, metric, sales),
                (rate_unit, sales_unit, rate_unit, sales_unit), values)]


def rate_bounds(values):
    boxes = []
    for value in values:
        decimals = len(value.split(".")[1]) if "." in value else 0
        center, radius = Fraction(value), Fraction(1, 2 * 10 ** decimals)
        boxes.append((center - radius, center + radius))
    # Independent exact expression preserves the denominator's repeated sales variables.
    answers = [c + (c - a) * b / (d - b) for a, b, c, d in product(*boxes)]
    return min(answers), max(answers)


class HtmlQuarterRate(unittest.TestCase):
    binary = None

    def fixture(self, rows=None, copies=1, omit=()):
        fixture = RateFixture(self.binary, rows or rate_rows(), copies)
        fixture.write(omit)
        return fixture

    def assess(self, fixture, value="140.0", target="Q4 2030", metric="AISC", flags=(), discover=True,
               copper=False):
        denominator, unit = ("pound", "lb") if copper else ("ounce", "oz")
        draft = f"Site A {target} 100% basis {metric} per {denominator} sold is US${value}/{unit}.\n"
        payload = success(fixture.lint(draft, (("--discover",) if discover else ()) + flags))
        return payload, payload["source_arithmetic"]["findings"]

    def test_all_quarters_and_cost_families_match_independent_exact_enclosures(self):
        for metric in ("AISC", "Gold cost of sales"):
            for prior, current, target in (("2030Q1", "2030H1", "Q2 2030"),
                                            ("2030H1", "20309M", "Q3 2030"),
                                            ("20309M", "2030FY", "Q4 2030")):
                observations = rate_rows(prior, current, metric)
                fixture = self.fixture(observations)
                payload, rows = self.assess(fixture, target=target, metric=metric)
                self.assertEqual(len(rows), 1, payload)
                row = rows[0]
                self.assertEqual(row["status"], "derived", row)
                self.assertEqual(bounds(row["derived_claim"]), rate_bounds([r["value"] for r in observations]))
                self.assertEqual(row["rule"], "public-disclosure.sales-weighted-quarter-rate/1")
                self.assertEqual(row["derived_claim"]["value"]["ownership"], "100%")
                self.assertIn("not independently established", row["assumption"])
                self.assertEqual(set(row["derived_claim"]["parents"]),
                                 {e["claim"]["claim_id"] for e in row["source_evidence"]})
                self.assertEqual(len(row["source_evidence"]), 4)
                self.assertEqual({e["publication"]["file"] for e in row["source_evidence"]}, {"Rates0.html"})
                self.assertEqual(payload, self.assess(fixture, target=target, metric=metric)[0])

    def test_precision_signed_costs_and_large_decimal_values_keep_exact_answers(self):
        fixture = self.fixture()
        for value, status in (("140.0", "derived"), ("140.000", "precision_not_established"), ("141.0", "contradicted")):
            _, rows = self.assess(fixture, value=value)
            self.assertEqual(rows[0]["status"], status)
        for values, answer in ((("-100.00", "100.000", "-120.00", "200.000"), "-140.0"),
                               (("9007199254740993.00", "100.000", "9007199254741013.00", "200.000"), "9007199254741033.0")):
            fixture = self.fixture(rate_rows(values=values))
            _, rows = self.assess(fixture, value=answer)
            self.assertEqual(rows[0]["status"], "derived", rows)
            self.assertEqual(bounds(rows[0]["derived_claim"]), rate_bounds(values))

    def test_copper_quarters_reuse_exact_shared_sales_arithmetic_with_copper_sources(self):
        values = ("1.0000", "100.000", "1.2000", "200.000")
        for source_metric, metric in (("Copper AISC", "AISC"), ("Copper C1 cash costs", "C1"),
                                       ("Copper cost of sales", "cost of sales")):
            for prior, current, target in (("2030Q1", "2030H1", "Q2 2030"),
                                            ("2030H1", "20309M", "Q3 2030"),
                                            ("20309M", "2030FY", "Q4 2030")):
                observations = rate_rows(prior, current, source_metric, values, copper=True)
                fixture = self.fixture(observations)
                payload, rows = self.assess(fixture, "1.40", target, metric, copper=True)
                self.assertEqual(len(rows), 1, payload)
                row = rows[0]
                self.assertEqual(row["status"], "derived", row)
                self.assertEqual(row["rule"], "public-disclosure.copper-sales-weighted-quarter-rate/1")
                self.assertEqual(row["unit"], "usd_per_lb")
                self.assertEqual(row["derived_claim"]["value"]["unit"], "usd_per_lb")
                self.assertEqual(bounds(row["derived_claim"]), rate_bounds(values))
                self.assertIn("sold-copper denominator in millions of pounds", row["assumption"])
                self.assertEqual(set(row["derived_claim"]["parents"]),
                                 {e["claim"]["claim_id"] for e in row["source_evidence"]})
                self.assertEqual(len(row["source_evidence"]), 4)
                sales = [e for e in row["source_evidence"] if e["role"] == "cumulative_sales"]
                self.assertEqual(len(sales), 2)
                for evidence in sales:
                    self.assertEqual(evidence["claim"]["metric_key"], "html:copper_sales_mlb")
                    self.assertEqual(evidence["publication"]["file"], "Rates0.html")
                rendered = fixture.run(("--format", "markdown", "--discover"), arguments=BASE)
                self.assertEqual(rendered.returncode, 0, rendered.stderr)
                self.assertIn("USD/lb", rendered.stdout)
                self.assertNotIn("] USD/oz", rendered.stdout)
                self.assertEqual(payload, self.assess(fixture, "1.40", target, metric, copper=True)[0])

    def test_copper_denominator_metal_units_missing_sales_and_basis_are_not_inferred(self):
        values = ("1.0000", "100.000", "1.2000", "200.000")
        for mutation, expected, code in (
            (lambda rows: rows.pop(), "not_available", None),
            (lambda rows: rows[1].update(metric="Gold sales", unit="koz"), "not_available", None),
            (lambda rows: rows[1].update(metric="Copper production", unit="kt"), "not_available", None),
            (lambda rows: rows[1].update(value="—"), "refused", "FMINE-REF-208"),
            (lambda rows: rows[3].update(value="100.001"), "refused", "FMINE-REF-104"),
            (lambda rows: rows[3].update(basis="Attributable (60%)"), "refused", "FMINE-REF-105"),
        ):
            observations = rate_rows(metric="Copper C1 cash costs", values=values, copper=True)
            mutation(observations)
            _, rows = self.assess(self.fixture(observations), "1.40", metric="C1", copper=True)
            self.assertEqual(rows[0]["status"], expected, rows)
            self.assertIsNone(rows[0]["derived_claim"])
            if code:
                self.assertEqual(rows[0]["refusal"]["code"], code)
        observations = rate_rows(metric="Copper C1 cash costs", values=values, copper=True)
        observations[1]["unit"] = "kt"
        fixture = self.fixture(observations)
        FIXTURES.refused(fixture.invoke(), "FMINE-REF-208")

    def test_copper_signed_rates_publication_competition_and_direct_disclosures_survive(self):
        values = ("-1.0000", "100.000", "-1.2000", "200.000")
        observations = rate_rows(metric="Copper C1 cash costs", values=values, copper=True)
        fixture = self.fixture(observations)
        for amount, status in (("-1.40", "derived"), ("-1.4000", "precision_not_established"),
                               ("-1.50", "contradicted")):
            _, rows = self.assess(fixture, amount, metric="C1", copper=True)
            self.assertEqual(rows[0]["status"], status, rows)
            self.assertEqual(bounds(rows[0]["derived_claim"]), rate_bounds(values))
        fixture = self.fixture(observations, copies=2)
        _, rows = self.assess(fixture, "-1.40", metric="C1", copper=True)
        self.assertEqual(rows[0]["status"], "ambiguous")
        self.assertEqual(len(rows[0]["source_evidence"]), 8)
        fixture = self.fixture(observations, copies=2, omit=("Rates1.html",))
        payload, rows = self.assess(fixture, "-1.40", metric="C1", copper=True,
                                   flags=("--as-of", "2031-02-01"))
        self.assertEqual(rows[0]["status"], "derived")
        self.assertEqual([s["file"] for s in payload["html_projection"]["excluded_sources"]], ["Rates1.html"])
        observations.append(observation("2030Q4", "-1.50", metric="Copper C1 cash costs", unit="US$/lb sold"))
        payload, rows = self.assess(self.fixture(observations), "-1.40", metric="C1", copper=True)
        self.assertEqual(findings(payload)[0]["verdict"], "value_mismatch")
        self.assertEqual(rows, [])

    def test_missing_unusable_conflicting_and_incompatible_operands_never_make_rates(self):
        for mutation, expected, code in (
            (lambda rows: rows.pop(), "not_available", None),
            (lambda rows: rows[1].update(value="—"), "refused", "FMINE-REF-208"),
            (lambda rows: rows[3].update(value="100.001"), "refused", "FMINE-REF-104"),
            (lambda rows: rows[3].update(basis="Attributable (60%)"), "refused", "FMINE-REF-105"),
        ):
            observations = rate_rows()
            mutation(observations)
            _, rows = self.assess(self.fixture(observations))
            self.assertEqual(rows[0]["status"], expected, rows)
            self.assertIsNone(rows[0]["derived_claim"])
            if code:
                self.assertEqual(rows[0]["refusal"]["code"], code)
        fixture = self.fixture(copies=2)
        _, rows = self.assess(fixture)
        self.assertEqual(rows[0]["status"], "ambiguous")
        self.assertEqual(len(rows[0]["source_evidence"]), 8)
        separate = ArithmeticFixture(self.binary, rate_rows())
        separate.write()
        _, rows = self.assess(separate)
        self.assertEqual(rows[0]["refusal"]["code"], "FMINE-REF-105")

    def test_publication_cut_source_mutations_and_direct_assertion_precedence_survive(self):
        fixture = self.fixture(copies=2, omit=("Rates1.html",))
        payload, rows = self.assess(fixture, flags=("--as-of", "2031-02-01"))
        self.assertEqual(rows[0]["status"], "derived")
        self.assertEqual([s["file"] for s in payload["html_projection"]["excluded_sources"]], ["Rates1.html"])
        fixture = self.fixture()
        first, rows = self.assess(fixture)
        old_id = rows[0]["derived_claim"]["claim_id"]
        mapping = fixture.root / MAPPING
        mapping.write_text("# changed interpretation bytes\n" + fixture.mapping)
        changed, rows = self.assess(fixture)
        self.assertNotEqual(old_id, rows[0]["derived_claim"]["claim_id"])
        self.assertNotEqual(first["source_arithmetic"]["digest"], changed["source_arithmetic"]["digest"])
        fixture.documents["Rates0.html"] = fixture.documents["Rates0.html"].replace(b"120.00", b"121.00")
        fixture.write()
        _, rows = self.assess(fixture)
        self.assertEqual(rows[0]["status"], "contradicted")
        self.assertEqual(bounds(rows[0]["derived_claim"]), rate_bounds(("100.00", "100.000", "121.00", "200.000")))
        source = fixture.root / "corpus/Rates0.html"
        source.write_bytes(source.read_bytes() + b" ")
        draft = "Site A Q4 2030 100% basis AISC per ounce sold is US$140.0/oz."
        FIXTURES.refused(fixture.lint(draft), "FMINE-REF-505")
        observations = rate_rows() + [observation("2030Q4", "99.00", metric="AISC", unit="US$/oz sold")]
        fixture = self.fixture(observations)
        payload, rows = self.assess(fixture)
        self.assertEqual(findings(payload)[0]["verdict"], "value_mismatch")
        self.assertEqual(rows, [])
        _, rows = self.assess(fixture, discover=False)
        self.assertEqual(rows[0]["status"], "direct_assertion_available")
        prior = by_id(fixture.projection())["rate_0_0"]
        cited = sentence(prior, "US$140.0/oz", "AISC per ounce sold", "Q4 2030")
        self.assertEqual(success(fixture.lint(cited, ("--discover",)))["source_arithmetic"]["findings"], [])


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    options = parser.parse_args()
    HtmlLint.binary = options.binary.resolve()
    HtmlArithmetic.binary = options.binary.resolve()
    HtmlQuarterRate.binary = options.binary.resolve()
    if options.fixture_only:
        fixture = Fixture(HtmlLint.binary)
        fixture.write()
        print(fixture.root)
    else:
        unittest.main(argv=[__file__])
