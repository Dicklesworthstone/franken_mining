"""BD-341 real-command regressions on Rust-built, explicitly invented workbook fixtures.

The Rust tests retain typed Claim/loader/composition assertions. This runner copies
their baseline files and preserves the independent serialized-output expectations.
No fixture here is evidence about a real issuer or a human reader.
"""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


DOCUMENT = b"%PDF-1.7 explicitly synthetic 2023 guidance, never parsed"
DOCUMENT_2024 = b"%PDF-1.7 explicitly synthetic 2024 guidance, never parsed"
TECHNICAL = b"%PDF-1.7 invented optional technical inputs, never parsed"
ORIGINAL, LATER = "Synthetic_2024Q4.xlsx", "Synthetic_2025Q4.xlsx"
TRACK_MODES = ((), ("--as-of", "2026-02-02"))
ZERO = '"z":{"num":"0","den":"1"}'
CONTRIBUTION = '"price_contribution":{"num":"25","den":"1"}'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def dump(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)


def source_path(root, issuer="synthetic"):
    return root / "registries/sources" / f"{issuer}.toml"


def readings_path(root, issuer="synthetic"):
    return root / "registries/transcriptions" / f"{issuer}.toml"


def entry(document, data, table, row, col, token, quantity, unit, basis, period, reader):
    fields = {"doc": document, "doc_sha256": digest(data), "page": 1, "table": table,
              "row": row, "col": col, "token": token, "quantity": quantity, "unit": unit,
              "basis": basis, "period": period, "reader": reader}
    return "\n[[entry]]\n" + "".join(f"{key} = {json.dumps(value)}\n" for key, value in fields.items())


def technical_entry(reader, token):
    return entry("Technical.pdf", TECHNICAL, "Invented technical inputs", "Site A", "Dry feed",
                 token, "technical_site_a_remaining_dry_feed_mt", "dry_mt", "100%;scenario=plan", "2024H2", reader)


def revise_reader(root, quantity, reader, replacement=None, issuer="synthetic"):
    path = readings_path(root, issuer)
    blocks = path.read_text().split("\n[[entry]]")
    revised, changed = [blocks[0]], 0
    for block in blocks[1:]:
        if f'quantity = "{quantity}"' in block and f'reader = "{reader}"' in block:
            changed += 1
            if replacement is None:
                continue
            old, new = replacement
            assert block.count(old) == 1
            block = block.replace(old, new)
        revised.append(block)
    assert changed == 1, "mutation must target one reader at one locator"
    path.write_text("\n[[entry]]".join(revised))


def replace_workbook(root, filename, replacement, issuer="synthetic"):
    path = root / "corpus" / filename
    original = path.read_bytes()
    old = f'bytes = {len(original)}\nsha256 = "{digest(original)}"'
    new = f'bytes = {len(replacement)}\nsha256 = "{digest(replacement)}"'
    registry = source_path(root, issuer)
    text = registry.read_text()
    assert text.count(old) == 1
    registry.write_text(text.replace(old, new))
    path.write_bytes(replacement)


def row(report, metric="aisc_usd_per_oz", year=2023):
    return next(r for r in report["payload"]["rows"]
                if r["site"] == "Site A" and r["metric"] == metric and r["year"] == year)


def price_tail(annual):
    return dump(annual).split('"price_sensitivity":', 1)[1]


class Report(dict):
    def __init__(self, raw):
        super().__init__(json.loads(raw))
        self.raw = raw
        self.markdown = None

    def __eq__(self, other):
        return (isinstance(other, Report) and dict.__eq__(self, other)
                and self.raw == other.raw and self.markdown == other.markdown)


class GuidanceBinding(unittest.TestCase):
    binary = normal = prices = None

    def fixture(self, price=False, subject="site_a", as_of=False):
        template = self.prices if price else self.normal
        root = Path(tempfile.mkdtemp(prefix="fmine-guidance-binding-")) / "workspace"
        shutil.copytree(template, root)
        if price and subject != "site_a":
            path = readings_path(root)
            path.write_text(path.read_text().replace("realized_price_site_a_", f"realized_price_{subject}_"))
        if as_of:
            source_path(root, "pricecut").write_text(source_path(root).read_text()
                .replace('issuer = "synthetic"', 'issuer = "pricecut"')
                .replace('retrieved_utc = "synthetic"', 'retrieved_utc = "2024-06-30T00:00:00Z"'))
            readings_path(root, "pricecut").write_text(readings_path(root).read_text()
                .replace('issuer = "synthetic"', 'issuer = "pricecut"'))
        return root

    def invoke(self, root, extra=(), issuer="synthetic"):
        command = [str(self.binary), "track-record", "--issuer", issuer, *extra, "--workspace", str(root)]
        output = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", check=False)
        try:
            result = Report(output.stdout)
        except ValueError as error:
            raise AssertionError((output.returncode, output.stdout, output.stderr)) from error
        self.assertEqual(output.returncode, result["exit_code"], output.stderr)
        if output.returncode == 0:
            # Rust Envelope equality also includes its rendered Markdown. Retain that check
            # alongside byte-identical JSON when comparing reruns or isolated evidence.
            markdown = subprocess.run(command + ["--format", "markdown"], capture_output=True,
                                      text=True, encoding="utf-8", check=False)
            self.assertEqual(markdown.returncode, 0, markdown.stderr)
            result.markdown = markdown.stdout
        return result

    def successful(self, root, extra=(), issuer="synthetic"):
        result = self.invoke(root, extra, issuer)
        self.assertEqual(result["exit_code"], 0, result)
        return result

    def refused(self, root, code, exit_code, extra=()):
        result = self.invoke(root, extra)
        self.assertEqual(result["exit_code"], exit_code, result)
        self.assertEqual(result["refusal"]["code"], code)
        self.assertNotIn('"score":', dump(result["payload"]))

    def price_at(self, root, cut, subject=None, success=True):
        args = ["--as-of", cut]
        if subject is not None:
            args += ["--price-subject", subject]
        return (self.successful if success else self.invoke)(root, args, "pricecut")

    def contains(self, text, *expected):
        for token in expected:
            self.assertIn(token, text)

    def test_ownership_selection_never_substitutes_full_project_production(self):
        root = self.fixture()
        path = readings_path(root)
        original_readings = path.read_text()
        # Replace only the production basis; AISC keeps its explicitly stated sold-ounce basis.
        path.write_text(original_readings.replace('basis = "attributable:3/4"', 'basis = "100%"'))
        full = row(self.successful(root, ["--site", "site-a"]), "production_koz", 2024)
        self.contains(dump(full["outcome"]), '"displayed":"200"')
        self.contains(dump(full), "Synthetic_2024Q4.xlsx:YTD!B5", '"side":"above"')
        path.write_text(original_readings)
        original_book = (root / "corpus" / ORIGINAL).read_bytes()
        replace_workbook(root, ORIGINAL, (root / "attribute-absent.xlsx").read_bytes())
        annual = row(self.successful(root), "production_koz", 2024)
        self.contains(dump(annual), '"state":"unresolved"')
        self.contains(dump(annual["outcome"]), "no annual summary cell in the original registered release")
        self.assertIsNone(annual["score"]["value"])
        self.assertNotIn('"displayed":"200"', dump(annual["outcome"]))
        replace_workbook(root, ORIGINAL, original_book)
        path.write_text(original_readings.replace('basis = "attributable:3/4"', 'basis = "attributable:1/2"'))
        annual = row(self.successful(root), "production_koz", 2024)
        self.contains(dump(annual["outcome"]), '"state":"unresolved"', '"code":"FMINE-REF-105"')
        self.assertIsNone(annual["score"]["value"])

    def test_cutoff_and_source_reload_preserve_snapshot_identity(self):
        root = self.fixture()
        original = self.successful(root, ["--through", "2024Q4"])
        full = self.successful(root)
        self.assertEqual(original["payload"]["through"], "2024Q4")
        self.assertEqual(row(original, "production_koz", 2024)["outcome"], row(full, "production_koz", 2024)["outcome"])
        replace_workbook(root, ORIGINAL, (root / "changed-original.xlsx").read_bytes())
        changed = self.successful(root, ["--through", "2024Q4"])
        self.assertNotEqual(original["payload"], changed["payload"])
        annual = row(changed, "production_koz", 2024)
        self.contains(dump(annual["outcome"]), '"displayed":"105"')
        self.contains(dump(annual), '"z":{"num":"1","den":"2"}')
        self.assertEqual(changed, self.successful(root, ["--through", "2024Q4"]))

    def test_changed_or_missing_workbooks_and_documents_refuse(self):
        root = self.fixture()
        for relative, held in (("documents/Guidance2023.pdf", "documents/guidance.held"),
                               (f"corpus/{ORIGINAL}", "corpus/original.held")):
            path, held_path = root / relative, root / held
            original = path.read_bytes()
            path.write_bytes(bytes([original[0] ^ 1]) + original[1:])
            self.refused(root, "FMINE-REF-505", 8)
            path.write_bytes(original)
            path.rename(held_path)
            self.refused(root, "FMINE-REF-506", 5)
            held_path.rename(path)
        self.successful(root)

    def test_disagreement_and_incomplete_readings_never_produce_scores(self):
        root = self.fixture()
        path = readings_path(root)
        original = path.read_text()
        blocks = original.split("\n[[entry]]")
        path.write_text("\n[[entry]]".join(b.replace('token = "80 - 100"', 'token = "81 - 101"')
                                           if 'reader = "reader_b"' in b else b for b in blocks))
        self.refused(root, "FMINE-REF-202", 3)
        path.write_text("\n[[entry]]".join(b for b in blocks if 'reader = "reader_b"' not in b))
        self.refused(root, "FMINE-REF-202", 3)
        path.write_text(original)
        self.successful(root)

    def test_price_sensitivity_retains_both_exact_scores_and_isolates_optional_locators(self):
        root = self.fixture(price=True)
        result = self.successful(root, ["--through", "2024Q2"])
        annual = row(result)
        self.contains(dump(annual), '"basis":"attributable:6/8;aisc=issuer-reported-per-sold-oz"')
        self.contains(price_tail(annual), CONTRIBUTION, '"unconditional":', ZERO, '"conditional":',
                      '"z":{"num":"-1","den":"4"}', '"price_proxy":false',
                      "Guidance2023.pdf:p1:Invented price sensitivity", "Prices.pdf:p1:Invented price sensitivity")
        self.assertEqual(result, self.successful(root, ["--through", "2024Q2"]))
        path = readings_path(root)
        readings = path.read_text()
        def optional(reader, token):
            return technical_entry(reader, token).replace("Technical.pdf", "Guidance2023.pdf").replace(digest(TECHNICAL), digest(DOCUMENT))
        for extra in (optional("optional-a", "10"), optional("optional-a", "10") + optional("optional-b", "11")):
            path.write_text(readings + extra)
            self.assertEqual(result, self.successful(root, ["--through", "2024Q2"]))
        price = price_tail(row(self.successful(root)))
        self.assertIn("2026Q2", price)
        self.assertNotIn('"price_contribution":', price)

    def test_price_sensitivity_requires_complete_history_and_an_explicit_company_proxy(self):
        root = self.fixture(price=True)
        for reader in ("reader_a", "reader_b"):
            revise_reader(root, "realized_price_site_a_2024q2_usd_per_oz", reader)
        annual = row(self.successful(root, ["--through", "2024Q2"]))
        self.assertIn(ZERO, dump(annual))
        self.assertNotIn('"price_contribution":', price_tail(annual))
        self.assertIn("2024Q2", price_tail(annual))
        root = self.fixture(price=True, subject="company")
        self.assertNotIn('"price_contribution":', dump(row(self.successful(root, ["--through", "2024Q2"]))))
        proxy = row(self.successful(root, ["--through", "2024Q2", "--price-subject", "Company"]))
        self.contains(dump(proxy), '"price_proxy":true', '"price_subject":"Company"', CONTRIBUTION)

    def test_shared_price_assumption_retains_its_source_and_refuses_competing_site_assumptions(self):
        site, all_sites = "aisc_price_assumption_site_a_2023fy_usd_per_oz", "aisc_price_assumption_all_sites_2023fy_usd_per_oz"
        root = self.fixture(price=True)
        before = row(self.successful(root, ["--through", "2024Q2"]))
        path = readings_path(root)
        readings = path.read_text()
        self.assertEqual(readings.count(f'quantity = "{site}"'), 2)
        shared_readings = readings.replace(f'quantity = "{site}"', f'quantity = "{all_sites}"')
        path.write_text(shared_readings)
        shared = row(self.successful(root, ["--through", "2024Q2"]))
        self.assertEqual(before["outcome"], shared["outcome"])
        self.assertEqual(price_tail(shared), price_tail(before).replace(f'"quantity":"{site}"', f'"quantity":"{all_sites}"'))
        self.assertIn(CONTRIBUTION, price_tail(shared))
        for reader in ("reader_a", "reader_b"):
            shared_readings += entry("Guidance2023.pdf", DOCUMENT, "Invented price sensitivity", "Additional site assumption",
                                     "2023FY", "1600", site, "usd_per_oz", "realized-sales-price", "2023FY", reader)
        path.write_text(shared_readings)
        annual = row(self.successful(root, ["--through", "2024Q2"]))
        self.assertEqual(annual["outcome"], before["outcome"])
        self.assertIn(ZERO, dump(annual))
        price = price_tail(annual)
        self.assertNotIn('"price_contribution":', price)
        self.contains(price, '"unsplit_reason":"unusable_price_assumption"', "found 2 in guidance document Guidance2023.pdf")
        for quantity in (site, all_sites):
            self.assertIn(f'"quantity":"{quantity}"', price)
        for source_row in (site, "Additional site assumption"):
            self.assertIn(f"Guidance2023.pdf:p1:Invented price sensitivity:{source_row}/2023FY", price)

    def test_optional_technical_failures_do_not_block_either_track_record_path(self):
        root = self.fixture()
        before = [self.successful(root, extra) for extra in TRACK_MODES]
        registry = source_path(root)
        registry.write_text(registry.read_text() + '\n[[document]]\nfile = "Technical.pdf"\ndir = "documents"\n'
            'kind = "technical_report"\nurl = "https://example.invalid/Technical.pdf"\nurl_basis = "W1 table"\n'
            'published_at = "2024-01-01"\npublished_at_basis = "FACT: invented technical fixture"\n'
            f'pages = 1\nbytes = {len(TECHNICAL)}\nsha256 = "{digest(TECHNICAL)}"\n')
        path = readings_path(root)
        base = path.read_text()
        agreed = base + technical_entry("reader_a", "10") + technical_entry("reader_b", "10")
        path.write_text(agreed)
        def unchanged():
            for mode, expected in zip(TRACK_MODES, before):
                self.assertEqual(self.successful(root, mode), expected)
        unchanged()
        (root / "documents/Technical.pdf").write_bytes(b"altered optional bytes")
        unchanged()
        (root / "documents/Technical.pdf").write_bytes(TECHNICAL)
        path.write_text(base + technical_entry("reader_a", "10"))
        unchanged()
        path.write_text(base + technical_entry("reader_a", "10") + technical_entry("reader_b", "11"))
        unchanged()
        path.write_text(agreed.replace(digest(TECHNICAL), "0" * 64))
        for mode in TRACK_MODES:
            result = self.invoke(root, mode)
            self.assertEqual(result["exit_code"], 5)
            self.assertEqual(result["refusal"]["code"], "FMINE-REF-506")

    def test_prefix_selection_retains_conflicting_guidance_readers_and_unsupported_keys(self):
        root = self.fixture()
        path = readings_path(root)
        base = path.read_text()
        def reading(reader, source_row, quantity):
            return entry("Guidance2024.pdf", DOCUMENT_2024, "Invented guidance", source_row, "2024", "90 - 110",
                         quantity, "koz", "attributable:3/4", "2024FY", reader)
        path.write_text(base + reading("reader_c", "Site A production_koz", "technical_site_a_another_interpretation"))
        for mode in TRACK_MODES:
            self.assertEqual(self.invoke(root, mode)["refusal"]["code"], "FMINE-REF-202")
        unsupported = "guidance_site_a_unsupported_output"
        path.write_text(base + reading("reader_a", "Site A unsupported output", unsupported)
                        + reading("reader_b", "Site A unsupported output", unsupported))
        public = self.successful(root, TRACK_MODES[1])
        self.contains(dump(public["payload"]),
            '"unsupported_guidance":[{"at":"Guidance2024.pdf:p1:Invented guidance:Site A unsupported output/2024"',
            f'"quantity":"{unsupported}"')
        self.refused(root, "FMINE-REF-201", 3)
        path.write_text(base + reading("reader_a", "Site A unsupported output", unsupported))
        for mode in TRACK_MODES:
            self.assertEqual(self.invoke(root, mode)["refusal"]["code"], "FMINE-REF-202")

    def test_realized_price_document_cannot_precede_its_quarter_end(self):
        root = self.fixture(price=True)
        before = row(self.successful(root, ["--through", "2024Q2"]))
        path = source_path(root)
        text = path.read_text()
        self.assertEqual(text.count('published_at = "2024-07-01"'), 1)
        path.write_text(text.replace('published_at = "2024-07-01"', 'published_at = "2024-01-01"'))
        annual = row(self.successful(root, ["--through", "2024Q2"]))
        self.assertEqual(annual["outcome"], before["outcome"])
        self.assertIn(ZERO, dump(annual))
        price = price_tail(annual)
        self.assertNotIn('"price_contribution":', price)
        self.contains(price, "precedes period end 2024-06-30", '"published_at":"2024-01-01"',
                      "Prices.pdf:p1:Invented price sensitivity:realized_price_site_a_2024q2_usd_per_oz/2024Q2")

    def test_incomplete_optional_price_evidence_keeps_the_unconditional_track_record(self):
        for disagree in (False, True):
            with self.subTest(disagree=disagree):
                root = self.fixture(price=True)
                before = row(self.successful(root, ["--through", "2024Q2"]))
                revise_reader(root, "realized_price_site_a_2024q2_usd_per_oz", "reader_b",
                              ('token = "2000"', 'token = "2100"') if disagree else None)
                after = self.successful(root, ["--through", "2024Q2"])
                annual = row(after)
                self.assertEqual(annual["outcome"], before["outcome"])
                self.assertIn(ZERO, dump(annual))
                price = price_tail(annual)
                self.contains(price, '"unsplit_reason":"incomplete_transcription_agreement"', "reader_a",
                              "Prices.pdf:p1:Invented price sensitivity:realized_price_site_a_2024q2_usd_per_oz/2024Q2")
                self.assertNotIn('"price_contribution":', price)
                self.assertEqual(after, self.successful(root, ["--through", "2024Q2"]))
                (root / "documents/Prices.pdf").write_bytes(b"altered optional price bytes")
                result = self.invoke(root, ["--through", "2024Q2"])
                self.assertEqual(result["exit_code"], 8)
                self.assertIn("FMINE-REF-505", dump(result))

    def test_incomplete_or_ambiguously_named_guidance_still_refuses_the_population(self):
        missing = self.fixture(price=True)
        revise_reader(missing, "guidance_site_a_aisc_usd_per_oz", "reader_b")
        mixed = self.fixture(price=True)
        revise_reader(mixed, "realized_price_site_a_2024q2_usd_per_oz", "reader_b",
                      ('quantity = "realized_price_site_a_2024q2_usd_per_oz"', 'quantity = "guidance_site_a_aisc_usd_per_oz"'))
        for root in (missing, mixed):
            result = self.invoke(root, ["--through", "2024Q2"])
            self.assertEqual(result["exit_code"], 3, result)
            self.assertIn("FMINE-REF-202", dump(result))

    def test_as_of_price_documents_after_the_cut_cannot_change_earlier_output(self):
        root = self.fixture(price=True, subject="company", as_of=True)
        before = self.price_at(root, "2024-06-30", "Company")
        self.assertNotIn('"price_contribution":', dump(before["payload"]))
        after = self.price_at(root, "2024-07-01", "Company")
        self.contains(dump(after["payload"]), CONTRIBUTION, '"explicit_proxy":true')
        path = readings_path(root, "pricecut")
        readings = path.read_text()
        revise_reader(root, "realized_price_company_2024q2_usd_per_oz", "reader_b",
                      ('token = "2000"', 'token = "2100"'), "pricecut")
        self.assertEqual(before, self.price_at(root, "2024-06-30", "Company"))
        conflicted = self.price_at(root, "2024-07-01", "Company")
        self.assertIn("incomplete_transcription_agreement", dump(conflicted["payload"]))
        self.assertNotIn('"price_contribution":', dump(conflicted["payload"]))
        path.write_text(readings)
        document = root / "documents/Prices.pdf"
        original = document.read_bytes()
        document.write_bytes(bytes([original[0] ^ 1]) + original[1:])
        self.assertEqual(before, self.price_at(root, "2024-06-30", "Company"))
        self.assertEqual(self.price_at(root, "2024-07-01", "Company", success=False)["refusal"]["code"], "FMINE-REF-505")
        document.write_bytes(original)

    def test_as_of_price_population_keeps_completed_but_unregistered_quarters(self):
        root = self.fixture(price=True, as_of=True)
        self.assertIn('"price_contribution":', dump(self.price_at(root, "2024-09-29")["payload"]))
        completed = self.price_at(root, "2024-09-30")
        price = dump(completed["payload"]).split('"price_sensitivity":', 1)[1]
        self.contains(price, '"required_quarters":["2024Q1","2024Q2","2024Q3"]',
                      "required original Mine Stats release is not registered")
        self.assertNotIn('"price_contribution":', price)
        self.assertIn('"n_scored":1', dump(completed["payload"]))

    def test_as_of_price_selections_use_their_own_outcome_and_royalty_vintage(self):
        root = self.fixture(price=True, as_of=True)
        replace_workbook(root, "Synthetic_Price_2024Q2.xlsx", (root / "comparative-q2.xlsx").read_bytes(), "pricecut")
        result = self.price_at(root, "2024-07-01")
        prices = dump(result["payload"]).split('"price_sensitivity":', 1)[1]
        first, latest = prices.split('"latest_registered":', 1)
        self.contains(first, "Synthetic_Price_2023Q4.xlsx:YTD!D5", '"z":{"num":"-1","den":"4"}')
        self.contains(latest, "Synthetic_Price_2024Q2.xlsx:YTD!K5", "Synthetic_Price_2024Q2.xlsx:Minestats1!B5",
                      '"z":{"num":"-3","den":"4"}')
        self.assertIn('"later_comparative"', dump(result["payload"]))


if __name__ == "__main__":
    if not __debug__:
        raise RuntimeError("Guidance oracle requires assertions; do not run with -O")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--normal", type=Path, required=True)
    parser.add_argument("--prices", type=Path, required=True)
    options = parser.parse_args()
    GuidanceBinding.binary = options.binary.resolve(strict=True)
    GuidanceBinding.normal = options.normal.resolve(strict=True)
    GuidanceBinding.prices = options.prices.resolve(strict=True)
    unittest.main(argv=[sys.argv[0]], verbosity=2)
