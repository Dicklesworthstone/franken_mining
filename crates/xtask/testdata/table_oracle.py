"""BD-340 source scenarios against the real fmine executable, using only Python's standard library.

The PDF bytes and reader records are invented. They establish loader/solver behavior,
never the content of a real filing or independent human review. These are the original
check_tables.rs cases; Rust separately retains library-versus-binary rendering parity.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


DOCUMENT = b"%PDF-1.7 synthetic reserve table, not a real PDF or parsed source"
FILE = "Invented.pdf"
DEFINITIONS = "table-identities.toml"
BASE = ["check", "tables", "--issuer", "table_test", "--definitions", DEFINITIONS]
BINARY = None


def digest(data):
    return hashlib.sha256(data).hexdigest()


def compact(value):
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)


def write_definitions(root, net_change_sign):
    text = 'schema = "fmine.table-identities.v1"\nissuer = "table_test"\n'
    for identity, family, kind, total, period in (
        ("reserve_rollforward", "reserve_balance", "roll_forward", "closing", "2025FY"),
        ("multi_year", "reserve_additions", "sum", "total", "2023-2025"),
    ):
        text += (
            f'\n[[identity]]\nid = "{identity}"\ndoc = "{FILE}"\nfamily = "{family}"\n'
            f'kind = "{kind}"\ntotal = "{total}"\ntotal_period = "{period}"\n'
            'unit = "koz"\nbasis = "100%"\nconstruction = "rounded_once"\nconvention = "closed"\n'
        )
    for identity, quantity, period, sign, role in (
        ("reserve_rollforward", "opening", "2024FY", 1, "opening"),
        ("reserve_rollforward", "depletion", "2025FY", 1, "move"),
        ("reserve_rollforward", "net_change", "2025FY", net_change_sign, "move"),
        ("multi_year", "year_2023", "2023FY", 1, "member"),
        ("multi_year", "year_2024", "2024FY", 1, "member"),
        ("multi_year", "year_2025", "2025FY", 1, "member"),
    ):
        text += (
            f'\n[[term]]\nidentity = "{identity}"\nquantity = "{quantity}"\n'
            f'period = "{period}"\nsign = {sign}\nrole = "{role}"\n'
        )
    (root / DEFINITIONS).write_text(text, encoding="utf-8")


def write_readings(root, mode="agreed"):
    text = 'schema = "fmine.transcriptions.v1"\nissuer = "table_test"\n'
    for reader in ("reader_a", "reader_b"):
        for quantity, token, period in (
            ("opening", "100", "2024FY"),
            ("depletion", "(20)", "2025FY"),
            ("net_change", "(5)", "2025FY"),
            ("closing", "85", "2025FY"),
            ("year_2023", "5", "2023FY"),
            ("year_2024", "7", "2024FY"),
            ("year_2025", "8", "2025FY"),
            ("total", "20", "2023-2025"),
        ):
            if quantity == "opening":
                if mode == "missing_opening" or (mode == "pending_opening" and reader == "reader_b"):
                    continue
                if mode == "conflicting_opening" and reader == "reader_b":
                    token = "101"
            if quantity == "closing" and mode == "closing75":
                token = "75"
            text += (
                f'\n[[entry]]\ndoc = "{FILE}"\ndoc_sha256 = "{digest(DOCUMENT)}"\npage = 1\n'
                f'table = "Invented reserves"\nrow = "{quantity}"\ncol = "{period}"\n'
                f'token = "{token}"\nquantity = "{quantity}"\nunit = "koz"\n'
                f'basis = "100%"\nperiod = "{period}"\nreader = "{reader}"\n'
            )
    (root / "registries/transcriptions/table_test.toml").write_text(text, encoding="utf-8")


def workspace(tag):
    # Retain fixtures, as the original Rust tests did; do not delete source artifacts on completion.
    root = Path(tempfile.mkdtemp(prefix=f"fmine-check-tables-{tag}-"))
    for directory in ("documents", "registries/sources", "registries/transcriptions"):
        (root / directory).mkdir(parents=True)
    (root / "documents" / FILE).write_bytes(DOCUMENT)
    # The deliberately absent workbook still exercises PDF-only isolation and the workbook refusal.
    source = (
        'schema = "fmine.sources.v1"\nissuer = "table_test"\ncorpus_dir = "corpus"\n'
        'retrieved_utc = "synthetic fixture"\nurl_prefix = "https://example.invalid/"\n'
        '\n[[source]]\nfile = "Unused.xlsx"\nkind = "mine_stats"\nquarter = "2025Q4"\n'
        f'path = "Unused.xlsx"\nurl_basis = "W1 table"\nbytes = 1\nsha256 = "{digest(b"x")}"\n'
        '\n[[subject]]\nname = "Invented Mine"\n'
        f'\n[[document]]\nfile = "{FILE}"\ndir = "documents"\nkind = "aif"\n'
        f'url = "https://example.invalid/{FILE}"\nurl_basis = "W1 table"\n'
        'published_at = "2026-02-01"\npublished_at_basis = "FACT: synthetic fixture date"\n'
        f'pages = 1\nbytes = {len(DOCUMENT)}\nsha256 = "{digest(DOCUMENT)}"\n'
    )
    (root / "registries/sources/table_test.toml").write_text(source, encoding="utf-8")
    write_readings(root)
    write_definitions(root, -1)
    return root


def reserve_workspace(tag, mode="normal", reverse=False):
    """Invented filings/deck with independently declared reserve and price readings."""
    root = workspace(f"reserves-{tag}")
    documents = [
        ("older", "Earlier.pdf", "mda", "2025-02-01", "10.0", "1500"),
        ("latest", "Later.pdf", "aif", "2026-02-01", "12.0", "1500"),
        ("deck", "Presentation.pdf", "investor_deck", "2026-03-01", "13.0", "1600"),
    ]
    if mode == "deck_only":
        documents = documents[2:]
    elif mode == "same_date":
        documents[0] = (*documents[0][:3], "2026-02-01", *documents[0][4:])
    elif mode == "changed_deck":
        documents[1] = (*documents[1][:-1], "1700")
    elif mode == "secondary":
        documents = [("secondary", "Secondary.pdf", "partner_filing", "2026-03-01", "13.0", "1600")]
    elif mode == "unknown_deck":
        documents = documents[:2]
    if reverse:
        documents.reverse()
    source = (root / "registries/sources/table_test.toml").read_text(encoding="utf-8")
    definitions = 'schema = "fmine.table-identities.v1"\nissuer = "table_test"\n'
    readings = 'schema = "fmine.transcriptions.v1"\nissuer = "table_test"\n'
    for row_id, file, kind, date, value, price in documents:
        contents = f"%PDF-1.7 invented reserve assertion in {file}; no real issuer data".encode()
        (root / "documents" / file).write_bytes(contents)
        source += (
            f'\n[[document]]\nfile = "{file}"\ndir = "documents"\nkind = "{kind}"\n'
            f'url = "https://example.invalid/{file}"\nurl_basis = "W1 table"\n'
            f'published_at = "{date}"\npublished_at_basis = "FACT: invented test date"\n'
            f'pages = 1\nbytes = {len(contents)}\nsha256 = "{digest(contents)}"\n'
        )
        definitions += (
            f'\n[[reserve_row]]\nid = "{row_id}"\ndoc = "{file}"\n'
            'quantity = "reserve_change"\nsubject = "Invented Mine"\nitem = "net_change"\n'
            'reserve_year = 2024\nperiod = "2024FY"\nmetal = "gold"\nunit = "moz"\n'
            'ownership = "full"\nsupport = "contained"\nconvention = "closed"\n'
        )
        if mode == "unknown_deck":
            definitions += 'deck = "unstated"\n'
        else:
            definitions += (
                'deck = "quoted"\ndeck_period = "2024FY"\ndeck_basis = "reserve_prices"\n'
                'gold_price_quantity = "reserve_gold_price"\n'
            )
        for reader in ("reader_b", "reader_a") if reverse else ("reader_a", "reader_b"):
            for quantity, token, unit, basis in (
                ("reserve_change", value, "moz", "gold;full;contained"),
                ("reserve_gold_price", price, "usd_per_oz", "reserve_prices"),
            ):
                if row_id == "latest":
                    if mode == "pending_latest" and quantity == "reserve_change" and reader == "reader_b":
                        continue
                    if mode == "pending_price" and quantity == "reserve_gold_price" and reader == "reader_b":
                        continue
                    if mode == "wrong_price_unit" and quantity == "reserve_gold_price":
                        unit = "usd_per_lb"
                    if mode == "wrong_reserve_basis" and quantity == "reserve_change":
                        basis = "gold;attributable:1/2;contained"
                    if mode == "conflicting_price" and quantity == "reserve_gold_price" and reader == "reader_b":
                        token = "1501"
                readings += (
                    f'\n[[entry]]\ndoc = "{file}"\ndoc_sha256 = "{digest(contents)}"\npage = 1\n'
                    f'table = "Invented reserves"\nrow = "{quantity}"\ncol = "2024"\n'
                    f'token = "{token}"\nquantity = "{quantity}"\nunit = "{unit}"\n'
                    f'basis = "{basis}"\nperiod = "2024FY"\nreader = "{reader}"\n'
                )
        if row_id == "latest" and mode in ("competing_value", "competing_price"):
            quantity, token, unit, basis = (
                ("reserve_change", value, "moz", "gold;full;contained")
                if mode == "competing_value"
                else ("reserve_gold_price", price, "usd_per_oz", "reserve_prices")
            )
            readings += (
                f'\n[[entry]]\ndoc = "{file}"\ndoc_sha256 = "{digest(contents)}"\npage = 1\n'
                f'table = "Invented reserves"\nrow = "Alternative {quantity}"\ncol = "2024"\n'
                f'token = "{token}"\nquantity = "{quantity}"\nunit = "{unit}"\n'
                f'basis = "{basis}"\nperiod = "2024FY"\nreader = "reader_c"\n'
            )
    (root / "registries/sources/table_test.toml").write_text(source, encoding="utf-8")
    (root / "registries/transcriptions/table_test.toml").write_text(readings, encoding="utf-8")
    (root / DEFINITIONS).write_text(definitions, encoding="utf-8")
    return root


def binary(root, arguments, output_format=None):
    args = [str(BINARY), *arguments, "--workspace", str(root)]
    if output_format is not None:
        args.extend(["--format", output_format])
    return subprocess.run(args, capture_output=True, text=True, check=False)


def invoke(root, extra=(), arguments=None):
    output = binary(root, [*BASE, *extra] if arguments is None else arguments)
    try:
        envelope = json.loads(output.stdout)
    except json.JSONDecodeError as error:
        raise AssertionError(f"expected JSON: {output.stdout}\n{output.stderr}") from error
    assert output.returncode == envelope["exit_code"], output.stdout
    return envelope, output.stdout


def successful(root):
    envelope, raw = invoke(root)
    assert envelope["exit_code"] == 0, raw
    assert envelope["refusal"] is None, raw
    return envelope, raw


def report_digest(payload):
    value = payload["checks"]["digest"]
    assert len(value) == 64 and all(c in "0123456789abcdefABCDEF" for c in value)
    return value


def assert_refusal(root, code, exit_code):
    envelope, raw = invoke(root)
    assert envelope["exit_code"] == exit_code, raw
    assert envelope["refusal"]["code"] == code, raw
    assert FILE in raw, raw
    assert envelope["payload"] is None, raw


class TableScenarios(unittest.TestCase):
    def test_registered_pdf_rollforward_and_explicit_multiyear_membership_are_solved(self):
        root = workspace("positive")
        envelope, _ = successful(root)
        text = compact(envelope["payload"])
        self.assertIn('"schema":"fmine.pdf-table-check.v1"', text)
        self.assertEqual(text.count('"verdict":"feasible"'), 2, text)
        self.assertEqual(text.count('"verdict":"feasible","checked":true'), 2)
        self.assertEqual(envelope["payload"]["checks"]["refused"], [])
        for expected in (
            "reserve_rollforward", "multi_year", "roll_forward", "2023-2025",
            "2023FY", "2024FY", "2025FY", "reader_a", "reader_b",
            "Invented.pdf:p1:Invented reserves:closing/2025FY", "(20)", "(5)",
        ):
            self.assertIn(expected, text)
        self.assertIn(digest(DOCUMENT), text)
        self.assertIn(f'"definitions_sha256":"{digest((root / DEFINITIONS).read_bytes())}"', text)
        self.assertIn('"gate_eligible":false', text)
        workbook, raw = invoke(root, arguments=["check", "tables", "--issuer", "table_test"])
        self.assertEqual(workbook["exit_code"], 5, raw)
        self.assertIn("Unused.xlsx", raw)

    def test_declared_sign_changes_finding_and_digest_without_changing_source_bytes(self):
        root = workspace("signed")
        positive, _ = successful(root)
        # Changing the sign of raw (5) changes 100 - 20 + 5 = 85 into 100 - 20 - 5 = 75.
        write_definitions(root, 1)
        conflict, _ = successful(root)
        text = compact(conflict["payload"])
        self.assertEqual(text.count('"verdict":"infeasible"'), 1, text)
        self.assertEqual(text.count('"verdict":"feasible"'), 1)
        self.assertIn("certificate_digest", text)
        self.assertIn('"verdict":"infeasible","checked":true', text)
        self.assertIn('"certificate_digest":"', text)
        self.assertIn('"blame":{"status":"blamed"', text)
        self.assertNotEqual(positive["payload"], conflict["payload"])
        self.assertNotEqual(report_digest(positive["payload"]), report_digest(conflict["payload"]))
        self.assertNotEqual(positive["payload"]["digest"], conflict["payload"]["digest"])
        self.assertEqual((root / "documents" / FILE).read_bytes(), DOCUMENT)
        for field in ('"corrected":', '"best":', '"fix":'):
            self.assertNotIn(field, text)
        write_readings(root, "closing75")
        changed, _ = successful(root)
        self.assertEqual(compact(changed["payload"]).count('"verdict":"feasible"'), 2)
        self.assertNotEqual(conflict["payload"], changed["payload"])
        self.assertNotEqual(report_digest(conflict["payload"]), report_digest(changed["payload"]))
        self.assertNotEqual(conflict["payload"]["digest"], changed["payload"]["digest"])

    def test_missing_and_single_reader_operands_remain_refused_identity_members(self):
        root = workspace("missing")
        for mode in ("missing_opening", "pending_opening"):
            with self.subTest(mode=mode):
                write_readings(root, mode)
                envelope, _ = successful(root)
                text = compact(envelope["payload"])
                self.assertIn('"identity":"reserve_rollforward"', text)
                self.assertIn("opening", text)
                self.assertEqual(
                    [row["identity"] for row in envelope["payload"]["checks"]["refused"]],
                    ["reserve_rollforward"],
                )
                self.assertEqual(text.count('"verdict":"feasible"'), 1, text)
                self.assertIn("multi_year", text)
                output = binary(root, BASE, "markdown")
                self.assertEqual(output.returncode, 0, output.stderr)
                for expected in (
                    "Evidence retained for unresolved identities",
                    "Invented.pdf:p1:Invented reserves:closing/2025FY", digest(DOCUMENT),
                ):
                    self.assertIn(expected, output.stdout)

    def test_altered_and_missing_pdf_bytes_refuse_before_any_certified_result(self):
        root = workspace("bytes")
        path = root / "documents" / FILE
        altered = bytearray(DOCUMENT)
        altered[0] ^= 1
        path.write_bytes(altered)
        assert_refusal(root, "FMINE-REF-505", 8)
        path.write_bytes(DOCUMENT)
        held = root / "documents/Invented.held"
        path.rename(held)
        assert_refusal(root, "FMINE-REF-506", 5)
        held.rename(path)
        successful(root)

    def test_declared_pdf_bytes_are_required_even_before_any_readings_exist(self):
        root = workspace("unread-document")
        (root / "registries/transcriptions/table_test.toml").write_text(
            'schema = "fmine.transcriptions.v1"\nissuer = "table_test"\n', encoding="utf-8"
        )
        path = root / "documents" / FILE
        held = root / "documents/Unread.held"
        path.rename(held)
        assert_refusal(root, "FMINE-REF-506", 5)
        held.rename(path)
        envelope, _ = successful(root)
        text = compact(envelope["payload"])
        self.assertIn('"components":[]', text)
        for identity in ("reserve_rollforward", "multi_year"):
            self.assertIn(f'"identity":"{identity}"', text)
        self.assertNotIn('"checked":true', text)

    def test_conflicting_reader_never_becomes_a_solver_operand(self):
        root = workspace("readers")
        write_readings(root, "conflicting_opening")
        assert_refusal(root, "FMINE-REF-202", 3)
        write_readings(root)
        successful(root)

    def test_exact_binary_json_markdown_and_reruns_keep_citations_and_authority_boundaries(self):
        root = workspace("binary")
        envelope, expected = successful(root)
        self.assertEqual(expected, successful(root)[1])
        for name in (
            "reserve_disclosure_claim", "execution_authorized", "geotechnical_signoff",
            "regulatory_compliance_claim",
        ):
            self.assertIs(envelope["flags"][name], False)
        json_output = binary(root, BASE, "json")
        self.assertEqual(json_output.returncode, 0)
        self.assertEqual(json_output.stdout, expected)
        markdown = binary(root, BASE, "markdown")
        self.assertEqual(markdown.returncode, 0)
        self.assertEqual(markdown.stdout, binary(root, BASE, "markdown").stdout)
        for expected_text in (
            FILE, digest(DOCUMENT), "2023-2025", "Signed members (period, role)",
            "-1 × net\\_change (2025FY, move)", "rounded\\_once / closed",
        ):
            self.assertIn(expected_text, markdown.stdout)

    def test_definitions_selectors_and_unknown_flags_are_usage_errors(self):
        root = workspace("grammar")
        for extra in (
            ["--through", "2025Q4"], ["--synthetic"], ["--fix"],
            ["--as-of", "2026-02-01"], ["--definitions", DEFINITIONS],
        ):
            with self.subTest(extra=extra):
                envelope, raw = invoke(root, extra)
                self.assertEqual(envelope["exit_code"], 2, raw)
                self.assertIsNone(envelope["refusal"])
        for args in (
            ["check", "tables", "--definitions", DEFINITIONS],
            ["check", "tables", "--issuer", "not_registered"],
        ):
            envelope, raw = invoke(root, arguments=args)
            self.assertEqual(envelope["exit_code"], 2, raw)


class ReserveScenarios(unittest.TestCase):
    def reserve_report(self, root):
        envelope, raw = successful(root)
        report = envelope["payload"]["issuer_reserves"]
        self.assertEqual(report["schema"], "fmine.issuer-reserves.v1")
        self.assertFalse(report["gate_eligible"])
        self.assertEqual(len(report["selections"]), 1, raw)
        self.assertEqual(len(report["digest"]), 64)
        return envelope, report

    def test_latest_filing_is_current_while_older_filings_and_decks_remain_cited(self):
        root = reserve_workspace("current")
        envelope, report = self.reserve_report(root)
        rows = {row["id"]: row for row in report["rows"]}
        self.assertEqual(set(rows), {"older", "latest", "deck"})
        current = report["selections"][0]
        self.assertEqual(current["status"], "current")
        self.assertEqual(current["current_id"], "latest")
        self.assertEqual(current["latest_filed_date"], "2026-02-01")
        self.assertEqual(rows["older"]["source"]["reading"]["token"], "10.0")
        self.assertEqual(rows["latest"]["source"]["reading"]["token"], "12.0")
        self.assertEqual(rows["deck"]["source_role"], "investor_deck")
        self.assertEqual(rows["latest"]["source_role"], "filed_report")
        self.assertEqual(rows["latest"]["economic_key"]["reserve_year"], 2024)
        self.assertEqual(rows["latest"]["deck"]["status"], "quoted")
        self.assertIn('"token":"1500"', compact(rows["latest"]["deck"]))
        self.assertEqual(rows["older"]["deck"]["key"], rows["latest"]["deck"]["key"])
        self.assertTrue(any(pair["status"] == "changed" for pair in report["comparisons"]))
        self.assertEqual(report["refused"], [])
        self.assertEqual(report, self.reserve_report(root)[1])
        self.assertIsNone(envelope["anchor_id"])
        self.assertIsNone(envelope["certificate"])
        self.assertTrue(all(value is False for value in envelope["flags"].values()))
        markdown = binary(root, BASE, "markdown")
        self.assertEqual(markdown.returncode, 0, markdown.stderr)
        for file in ("Earlier.pdf", "Later.pdf", "Presentation.pdf"):
            self.assertIn(file, markdown.stdout)
            self.assertIn(digest((root / "documents" / file).read_bytes()), markdown.stdout)
        self.assertIn("reserve_gold_price", compact(report))

    def test_same_date_filings_remain_ambiguous_instead_of_selecting_by_import_order(self):
        root = reserve_workspace("ambiguous", "same_date")
        _, report = self.reserve_report(root)
        selection = report["selections"][0]
        self.assertEqual(selection["status"], "ambiguous_latest")
        self.assertIsNone(selection["current_id"])
        self.assertEqual(set(selection["candidates"]), {"older", "latest"})
        self.assertEqual(len(report["rows"]), 3)

    def test_unusable_latest_value_or_deck_cannot_fall_back_to_an_older_filing(self):
        for mode in ("pending_latest", "pending_price", "wrong_price_unit", "wrong_reserve_basis"):
            with self.subTest(mode=mode):
                root = reserve_workspace(mode, mode)
                envelope, report = self.reserve_report(root)
                selection = report["selections"][0]
                self.assertEqual(selection["status"], "latest_unavailable")
                self.assertIsNone(selection["current_id"])
                self.assertIn("latest", selection["unavailable"])
                self.assertTrue(any(row["id"] == "older" for row in report["rows"]))
                self.assertTrue(any(row["id"] == "latest" for row in report["refused"]))
                if mode.startswith("pending"):
                    self.assertTrue(envelope["payload"]["pending_readings"])

    def test_a_deck_alone_never_supplies_current_filed_reserves(self):
        root = reserve_workspace("deck", "deck_only")
        _, report = self.reserve_report(root)
        self.assertEqual(report["selections"][0]["status"], "no_filed_row")
        self.assertIsNone(report["selections"][0]["current_id"])
        self.assertEqual([row["id"] for row in report["rows"]], ["deck"])

    def test_partner_source_cannot_be_relabelled_as_the_issuers_filed_reserve(self):
        _, report = self.reserve_report(reserve_workspace("secondary", "secondary"))
        self.assertEqual(report["rows"], [])
        self.assertEqual(report["selections"][0]["status"], "no_filed_row")
        self.assertIsNone(report["selections"][0]["current_id"])
        self.assertEqual(report["refused"][0]["id"], "secondary")
        self.assertIn("partner_filing", report["refused"][0]["reason"])

    def test_new_filing_retains_its_changed_price_deck_without_coalescing_assumptions(self):
        normal = self.reserve_report(reserve_workspace("normal-deck"))[1]
        root = reserve_workspace("changed-deck", "changed_deck")
        _, changed = self.reserve_report(root)
        self.assertEqual(changed["selections"][0]["current_id"], "latest")
        rows = {row["id"]: row for row in changed["rows"]}
        self.assertNotEqual(rows["older"]["deck"]["key"], rows["latest"]["deck"]["key"])
        pair = next(pair for pair in changed["comparisons"] if {pair["left"], pair["right"]} == {"older", "latest"})
        self.assertEqual(pair["status"], "deck_differs")
        self.assertNotEqual(normal["digest"], changed["digest"])
        self.assertIn('"token":"1700"', compact(rows["latest"]["deck"]))

    def test_unstated_decks_remain_unknown_even_when_both_rows_omit_prices(self):
        _, report = self.reserve_report(reserve_workspace("unknown-deck", "unknown_deck"))
        self.assertEqual(report["selections"][0]["current_id"], "latest")
        self.assertEqual([pair["status"] for pair in report["comparisons"]], ["deck_unstated"])
        self.assertTrue(all(row["deck"]["key"] is None for row in report["rows"]))

    def test_reserve_documents_and_price_readers_are_checked_through_the_real_loader(self):
        root = reserve_workspace("bytes")
        path = root / "documents/Later.pdf"
        original = path.read_bytes()
        path.write_bytes(bytes([original[0] ^ 1]) + original[1:])
        envelope, raw = invoke(root)
        self.assertEqual((envelope["exit_code"], envelope["refusal"]["code"]), (8, "FMINE-REF-505"), raw)
        self.assertIsNone(envelope["payload"])
        path.write_bytes(original)
        held = path.with_suffix(".held")
        path.rename(held)
        envelope, raw = invoke(root)
        self.assertEqual((envelope["exit_code"], envelope["refusal"]["code"]), (5, "FMINE-REF-506"), raw)
        held.rename(path)
        self.reserve_report(root)
        conflict = reserve_workspace("conflicting-price", "conflicting_price")
        envelope, raw = invoke(conflict)
        self.assertEqual((envelope["exit_code"], envelope["refusal"]["code"]), (3, "FMINE-REF-202"), raw)
        self.assertIn("Later.pdf", raw)
        self.assertIsNone(envelope["payload"])

    def test_declaration_reader_and_document_order_do_not_change_reserve_results(self):
        first, report = self.reserve_report(reserve_workspace("order-a"))
        second, reordered = self.reserve_report(reserve_workspace("order-b", reverse=True))
        self.assertEqual(report, reordered)
        # The outer report additionally binds the exact declaration-file bytes, whose order did change.
        self.assertNotEqual(first["payload"]["definitions_sha256"], second["payload"]["definitions_sha256"])

    def test_pending_competing_value_or_price_location_cannot_be_silently_accepted(self):
        for mode, quantity in (("competing_value", "reserve_change"), ("competing_price", "reserve_gold_price")):
            with self.subTest(mode=mode):
                envelope, raw = invoke(reserve_workspace(mode, mode))
                self.assertEqual((envelope["exit_code"], envelope["refusal"]["code"]), (3, "FMINE-REF-202"), raw)
                self.assertIsNone(envelope["payload"])
                self.assertIn(f"quantity {quantity}", raw)
                self.assertIn(f"accepted at Later.pdf:p1:Invented reserves:{quantity}/2024", raw)
                self.assertIn(f"pending at Later.pdf:p1:Invented reserves:Alternative {quantity}/2024", raw)
                self.assertIn("reader_c", raw)


if __name__ == "__main__":
    if not __debug__:
        raise RuntimeError("Table oracle requires enabled assertions; do not run with -O")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--fixture-only", action="store_true")
    options = parser.parse_args()
    BINARY = options.binary.resolve()
    if options.fixture_only:
        print(workspace("library-parity"))
    else:
        unittest.main(argv=[__file__], verbosity=2)
