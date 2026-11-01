"""BD-340 source scenarios against the real fmine executable, using only Python's standard library.

The PDF bytes and reader records are invented. They establish loader/solver behavior,
never the content of a real filing or independent human review. These are the original
check_tables.rs cases; Rust separately retains library-versus-binary rendering parity.
"""

import argparse
from fractions import Fraction
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


def geo_workspace(tag, mode="matching", reverse=False):
    """Known-answer amounts and independently cited price hypotheses, all invented."""
    root = workspace(f"geo-{tag}")
    source = (root / "registries/sources/table_test.toml").read_text(encoding="utf-8")
    hashes = {}
    for file in ("Geo_Statement.pdf", "Candidate_Deck.pdf"):
        contents = f"%PDF-1.7 invented GEO source {file}; no actual issuer data".encode()
        (root / "documents" / file).write_bytes(contents)
        hashes[file] = digest(contents)
        source += (
            f'\n[[document]]\nfile = "{file}"\ndir = "documents"\nkind = "mda"\n'
            f'url = "https://example.invalid/{file}"\nurl_basis = "W1 table"\n'
            'published_at = "2026-02-01"\npublished_at_basis = "FACT: invented test date"\n'
            f'pages = 1\nbytes = {len(contents)}\nsha256 = "{hashes[file]}"\n'
        )
    specs = [
        ("gold", "gold", "moz", "10.0", "Geo_Statement.pdf"),
        ("copper", "copper", "mlb", "1000", "Geo_Statement.pdf"),
        ("geo", "geo", "moz", "13.0" if mode == "mismatched" else "12.0", "Geo_Statement.pdf"),
        ("candidate", "geo", "moz", "12.0", "Candidate_Deck.pdf"),
    ]
    if reverse:
        specs.reverse()
    definitions = 'schema = "fmine.table-identities.v1"\nissuer = "table_test"\n'
    entries = []
    for row_id, metal, unit, token, file in specs:
        definitions += (
            f'\n[[reserve_row]]\nid = "{row_id}"\ndoc = "{file}"\n'
            f'quantity = "{row_id}_amount"\nsubject = "Invented Mine"\nitem = "net_change"\n'
            f'reserve_year = 2024\nperiod = "2024FY"\nmetal = "{metal}"\nunit = "{unit}"\n'
            'ownership = "full"\nsupport = "contained"\nconvention = "closed"\n'
        )
        if metal == "geo":
            definitions += (
                'deck = "quoted"\ndeck_period = "2024FY"\ndeck_basis = "reserve_prices"\n'
                f'gold_price_quantity = "{row_id}_gold_price"\ncopper_price_quantity = "{row_id}_copper_price"\n'
            )
            gold_price, copper_price = ("1500", "3.00") if row_id == "geo" else ("1500", "4.50")
            if row_id == "candidate" and mode == "same_ratio":
                gold_price, copper_price = "1800", "3.60"
            if row_id == "candidate" and mode == "duplicate_deck":
                gold_price, copper_price = "1500", "3.00"
            entries.extend([
                (file, f"{row_id}_gold_price", gold_price, "usd_per_oz", "reserve_prices"),
                (file, f"{row_id}_copper_price", copper_price, "usd_per_lb", "reserve_prices"),
            ])
        else:
            definitions += 'deck = "unstated"\n'
        if mode == "wrong_unit" and row_id == "copper":
            unit = "kt"
        entries.append((file, f"{row_id}_amount", token, unit, f"{metal};full;contained"))
    definitions += (
        '\n[[geo_check]]\nid = "conversion"\ngold_row = "gold"\ncopper_row = "copper"\ngeo_row = "geo"\n'
        '\n[[geo_candidate]]\ncheck = "conversion"\nrow = "candidate"\n'
    )
    if reverse:
        entries.reverse()
    readings = 'schema = "fmine.transcriptions.v1"\nissuer = "table_test"\n'
    for file, quantity, token, unit, basis in entries:
        if (mode, quantity) in (
            ("missing_amount", "copper_amount"),
            ("missing_candidate_price", "candidate_copper_price"),
            ("missing_candidate_amount", "candidate_amount"),
        ):
            continue
        readers = ("reader_b", "reader_a") if reverse else ("reader_a", "reader_b")
        for reader in readers:
            if mode == "pending_price" and quantity == "geo_gold_price" and reader == "reader_b":
                continue
            readings += (
                f'\n[[entry]]\ndoc = "{file}"\ndoc_sha256 = "{hashes[file]}"\npage = 1\n'
                f'table = "Invented GEO conversion"\nrow = "{quantity}"\ncol = "2024"\n'
                f'token = "{token}"\nquantity = "{quantity}"\nunit = "{unit}"\n'
                f'basis = "{basis}"\nperiod = "2024FY"\nreader = "{reader}"\n'
            )
    (root / "registries/sources/table_test.toml").write_text(source, encoding="utf-8")
    (root / "registries/transcriptions/table_test.toml").write_text(readings, encoding="utf-8")
    (root / DEFINITIONS).write_text(definitions, encoding="utf-8")
    return root


def cutoff_workspace(tag, mode="constant", reverse=False):
    """Exact invented cut-off assumptions read through the same pinned-source loader."""
    root = workspace(f"cutoff-{tag}")
    file = "Technical_Report.pdf"
    contents = b"%PDF-1.7 invented cutoff assumptions; not an authentic technical report"
    (root / "documents" / file).write_bytes(contents)
    source = (root / "registries/sources/table_test.toml").read_text(encoding="utf-8")
    kind = "investor_deck" if mode == "wrong_source" else "technical_report"
    source += (
        f'\n[[document]]\nfile = "{file}"\ndir = "documents"\nkind = "{kind}"\n'
        f'url = "https://example.invalid/{file}"\nurl_basis = "W1 table"\n'
        'published_at = "2026-02-01"\npublished_at_basis = "FACT: invented test date"\n'
        f'pages = 1\nbytes = {len(contents)}\nsha256 = "{digest(contents)}"\n'
    )
    unit = "oz_per_short_ton" if mode == "short_ton" else "g_per_t"
    model = "tcm_over_gold" if mode == "grade_dependent" else "constant_percent"
    published = {
        "grade_dependent": "1.50", "short_ton": "0.0583", "deductions": "2.50",
        "within_rounding": "2.01", "conflict": "3.00",
    }.get(mode, "2.00")
    entries = [
        ("published", "cutoff_published", published, unit, "gold;dry;model"),
        ("gold_price", "cutoff_gold_price", "3110.34768", "usd_per_oz", "evaluation_price"),
        ("cost", "cutoff_cost", "100" if mode == "within_rounding" else "100.0", "usd_per_t", "gold;dry;model"),
    ]
    if mode == "grade_dependent":
        entries.extend([
            ("recovery_intercept", "cutoff_intercept", "100.0", "percent", "gold;dry;model"),
            ("recovery_slope", "cutoff_slope", "50.0", "g_per_t", "gold;dry;model"),
            ("tcm", "cutoff_tcm", "1.00", "percent", "gold;dry;model"),
        ])
    else:
        entries.append(("recovery", "cutoff_recovery", "50.0", "percent", "gold;dry;model"))
    if mode == "short_ton":
        entries.append(("published_metric", "cutoff_metric", "2.00", "g_per_t", "gold;dry;model"))
    if mode == "deductions":
        entries[2] = ("cost", "cutoff_cost", "60.0", "usd_per_t", "gold;dry;model")
        entries.extend([
            ("cost", "cutoff_sustaining", "40.0", "usd_per_t", "gold;dry;model"),
            ("revenue_deduction", "cutoff_tax", "10.0", "percent", "gold;dry;model"),
            ("ounce_deduction", "cutoff_refining", "311.034768", "usd_per_oz", "gold;dry;model"),
        ])
    if reverse:
        entries.reverse()
    definitions = (
        'schema = "fmine.table-identities.v1"\nissuer = "table_test"\n'
        f'\n[[cutoff]]\nid = "cutoff_example"\ndoc = "{file}"\nsubject = "Invented Mine"\n'
        'period = "effective_2026_01_01"\nbasis = "gold;dry;model"\nprice_basis = "evaluation_price"\n'
        f'unit = "{unit}"\nrecovery = "{model}"\n'
    )
    readings = 'schema = "fmine.transcriptions.v1"\nissuer = "table_test"\n'
    for role, quantity, token, reading_unit, basis in entries:
        definitions += f'\n[[cutoff_input]]\ncutoff = "cutoff_example"\nrole = "{role}"\nquantity = "{quantity}"\n'
        if mode == "missing_cost" and role == "cost":
            continue
        if mode == "wrong_unit" and role == "cost":
            reading_unit = "usd_per_short_ton"
        if mode == "wrong_price_basis" and role == "gold_price":
            basis = "different_price_assumption"
        if mode == "negative_cost" and role == "cost":
            token = "(100.0)"
        if mode == "invalid_recovery" and role == "recovery":
            token = "150.0"
        if mode == "range_input" and role == "cost":
            token = "99.0 - 101.0"
        for reader in ("reader_b", "reader_a") if reverse else ("reader_a", "reader_b"):
            if mode == "pending_price" and role == "gold_price" and reader == "reader_b":
                continue
            actual_token = "100.1" if mode == "disagreed_cost" and role == "cost" and reader == "reader_b" else token
            readings += (
                f'\n[[entry]]\ndoc = "{file}"\ndoc_sha256 = "{digest(contents)}"\npage = 1\n'
                f'table = "Invented cutoff assumptions"\nrow = "{quantity}"\ncol = "Model"\n'
                f'token = "{actual_token}"\nquantity = "{quantity}"\nunit = "{reading_unit}"\n'
                f'basis = "{basis}"\nperiod = "effective_2026_01_01"\nreader = "{reader}"\n'
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
            ["--as-of", "2026-02-01T00:00:00Z"], ["--definitions", DEFINITIONS],
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

    def test_publication_cut_checks_every_declared_document_and_preserves_arithmetic(self):
        root = workspace("evidence-cut")
        retrospective, _ = successful(root)
        admitted, raw = invoke(root, ["--as-of", "2026-02-01"])
        self.assertEqual(admitted["exit_code"], 0, raw)
        self.assertEqual(admitted["payload"]["as_of"], "2026-02-01")
        self.assertTrue(admitted["payload"]["source_bytes_verified"])
        self.assertEqual(admitted["payload"]["checks"], retrospective["payload"]["checks"])
        self.assertNotEqual(admitted["payload"]["digest"], retrospective["payload"]["digest"])
        self.assertEqual([row["file"] for row in admitted["payload"]["checked_documents"]], [FILE])
        unavailable, raw = invoke(root, ["--as-of", "2026-01-31"])
        self.assertEqual((unavailable["exit_code"], unavailable["refusal"]["code"]),
                         (3, "FMINE-REF-202"), raw)
        self.assertIn(FILE, unavailable["refusal"]["what"])
        self.assertIsNone(unavailable["payload"])


class CutoffScenarios(unittest.TestCase):
    def cutoff_report(self, root):
        envelope, raw = successful(root)
        section = envelope["payload"]["checks"]["cutoffs"]
        self.assertEqual(section["schema"], "fmine.source-cutoffs.v1")
        self.assertFalse(section["gate_eligible"])
        self.assertEqual(len(section["rows"]), 1, raw)
        self.assertEqual(len(section["digest"]), 64)
        self.assertEqual(section["rows"][0]["id"], "cutoff_example")
        return envelope, section

    def test_constant_recovery_cutoff_is_computed_from_cited_exact_inputs(self):
        root = cutoff_workspace("constant")
        envelope, section = self.cutoff_report(root)
        row = section["rows"][0]
        self.assertEqual(row["status"], "reproduced")
        result = row["result"]
        self.assertEqual(Fraction(result["exact"]["computed"]), 2)
        self.assertEqual(Fraction(result["exact"]["low"]), Fraction(1999, 1001))
        self.assertEqual(Fraction(result["exact"]["high"]), Fraction(2001, 999))
        self.assertEqual(result["computed"], "2.000000")
        self.assertIsNone(row["refusal"])
        self.assertEqual(len(row["evidence"]), 4)
        text = compact(section)
        for quantity in ("cutoff_published", "cutoff_gold_price", "cutoff_cost", "cutoff_recovery"):
            self.assertIn(quantity, text)
        self.assertIn(digest((root / "documents/Technical_Report.pdf").read_bytes()), text)
        self.assertIn("reader_a", text)
        self.assertIn("reader_b", text)
        self.assertTrue(all(value is False for value in envelope["flags"].values()))
        self.assertIsNone(envelope["certificate"])
        markdown = binary(root, BASE, "markdown")
        self.assertEqual(markdown.returncode, 0, markdown.stderr)
        for expected in ("cutoff_example", "Technical_Report.pdf", "cutoff_cost", "reproduced"):
            self.assertIn(expected, markdown.stdout)

    def test_grade_dependent_recovery_and_named_cost_deductions_use_existing_mathematics(self):
        for mode, expected in (("grade_dependent", Fraction(3, 2)), ("deductions", Fraction(5, 2))):
            with self.subTest(mode=mode):
                _, section = self.cutoff_report(cutoff_workspace(mode, mode))
                row = section["rows"][0]
                self.assertEqual(row["status"], "reproduced")
                self.assertEqual(Fraction(row["result"]["exact"]["computed"]), expected)
                if mode == "grade_dependent":
                    self.assertIsNotNone(row["result"]["solver_iterations"])
                else:
                    self.assertEqual(len(row["evidence"]), 7)

    def test_short_ton_output_is_explicit_and_keeps_the_published_metric_equivalent(self):
        _, section = self.cutoff_report(cutoff_workspace("short-ton", "short_ton"))
        result = section["rows"][0]["result"]
        self.assertEqual(Fraction(result["exact"]["computed"]), Fraction(7, 120))
        self.assertEqual(result["unit"], "oz/st")
        self.assertTrue(result["metric_from_rounded"])

    def test_rounding_compatible_and_conflicting_published_grades_remain_distinct(self):
        for mode, expected in (("within_rounding", "within_input_rounding"), ("conflict", "conflict")):
            with self.subTest(mode=mode):
                _, section = self.cutoff_report(cutoff_workspace(mode, mode))
                row = section["rows"][0]
                self.assertEqual(row["status"], expected)
                self.assertEqual(Fraction(row["result"]["exact"]["computed"]), 2)
                self.assertIsNone(row["refusal"])

    def test_incomplete_or_incompatible_source_inputs_are_retained_as_refusals(self):
        for mode in ("missing_cost", "pending_price", "wrong_unit", "wrong_price_basis", "wrong_source",
                     "negative_cost", "invalid_recovery", "range_input"):
            with self.subTest(mode=mode):
                envelope, section = self.cutoff_report(cutoff_workspace(mode, mode))
                row = section["rows"][0]
                self.assertEqual(row["status"], "refused")
                self.assertIsNone(row["result"])
                self.assertTrue(row["refusal"])
                self.assertTrue(row["evidence"])
                self.assertIn("Technical_Report.pdf", compact(row["documents"]))
                if mode == "pending_price":
                    self.assertTrue(envelope["payload"]["pending_readings"])

    def test_real_loader_refuses_changed_documents_and_disagreeing_cost_readers(self):
        root = cutoff_workspace("bytes")
        path = root / "documents/Technical_Report.pdf"
        contents = path.read_bytes()
        path.write_bytes(bytes([contents[0] ^ 1]) + contents[1:])
        envelope, raw = invoke(root)
        self.assertEqual((envelope["exit_code"], envelope["refusal"]["code"]), (8, "FMINE-REF-505"), raw)
        self.assertIsNone(envelope["payload"])
        envelope, raw = invoke(cutoff_workspace("disagreed", "disagreed_cost"))
        self.assertEqual((envelope["exit_code"], envelope["refusal"]["code"]), (3, "FMINE-REF-202"), raw)
        self.assertIsNone(envelope["payload"])

    def test_declaration_and_reader_order_keep_the_source_bound_cutoff_digest(self):
        root = cutoff_workspace("order-a")
        _, original = self.cutoff_report(root)
        self.assertEqual(original, self.cutoff_report(root)[1])
        self.assertEqual(original, self.cutoff_report(cutoff_workspace("order-b", reverse=True))[1])


class GeoScenarios(unittest.TestCase):
    def geo_report(self, root):
        envelope, raw = successful(root)
        reserves = envelope["payload"]["issuer_reserves"]
        self.assertEqual(len(reserves["geo_checks"]), 1, raw)
        check = reserves["geo_checks"][0]
        self.assertEqual(check["id"], "conversion")
        self.assertEqual(len(check["digest"]), 64)
        self.assertFalse(reserves["gate_eligible"])
        return envelope, check

    def test_printed_deck_conversion_retains_exact_bounds_and_every_source(self):
        root = geo_workspace("matching")
        envelope, check = self.geo_report(root)
        self.assertEqual(check["status"], "checked")
        declared = check["declared"]
        self.assertEqual(declared["fit"], "proper")
        rational = lambda value: Fraction(int(value["num"]), int(value["den"]))
        self.assertEqual(rational(declared["converted"]["lo"]), Fraction(11949, 1000))
        self.assertEqual(rational(declared["converted"]["hi"]), Fraction(12051, 1000))
        self.assertEqual(rational(declared["reported"]["lo"]), Fraction(239, 20))
        self.assertEqual(rational(declared["reported"]["hi"]), Fraction(241, 20))
        self.assertEqual(check["discovery"]["finding"], "found")
        self.assertEqual(check["discovery"]["deck_keys"], [declared["deck_key"]])
        self.assertEqual(len(check["discovery"]["fits"]), 8)
        self.assertEqual({row["row"] for row in check["amounts"]}, {"gold", "copper", "geo"})
        self.assertEqual({row["row"] for row in check["decks"]}, {"geo", "candidate"})
        text = compact(check)
        for quantity in ("gold_amount", "copper_amount", "geo_amount", "geo_gold_price", "geo_copper_price"):
            self.assertIn(quantity, text)
        for file in ("Geo_Statement.pdf", "Candidate_Deck.pdf"):
            self.assertIn(digest((root / "documents" / file).read_bytes()), text)
        self.assertTrue(all(value is False for value in envelope["flags"].values()))
        self.assertIsNone(envelope["certificate"])
        markdown = binary(root, BASE, "markdown")
        self.assertEqual(markdown.returncode, 0, markdown.stderr)
        self.assertIn("conversion", markdown.stdout)
        self.assertIn("Geo_Statement.pdf", markdown.stdout)
        self.assertIn("Candidate_Deck.pdf", markdown.stdout)
        self.assertEqual(check, self.geo_report(root)[1])

    def test_alternative_fit_does_not_rewrite_the_reported_price_deck(self):
        _, matching = self.geo_report(geo_workspace("matching-control"))
        envelope, check = self.geo_report(geo_workspace("mismatched", "mismatched"))
        self.assertEqual(check["status"], "checked")
        self.assertEqual(check["declared"]["fit"], "empty")
        self.assertEqual(check["discovery"]["finding"], "found")
        primary = next(row for row in check["decks"] if row["primary"])
        candidate = next(row for row in check["decks"] if not row["primary"])
        self.assertEqual(check["declared"]["deck_key"], primary["key"])
        self.assertEqual(check["discovery"]["deck_keys"], [candidate["key"]])
        self.assertNotEqual(primary["key"], candidate["key"])
        self.assertNotEqual(matching["digest"], check["digest"])
        geo = next(row for row in envelope["payload"]["issuer_reserves"]["rows"] if row["id"] == "geo")
        self.assertEqual(geo["deck"]["key"], primary["key"])
        self.assertEqual(geo["source"]["reading"]["token"], "13.0")

    def test_equal_ratios_remain_ambiguous_and_duplicate_decks_keep_their_providers(self):
        _, ambiguous = self.geo_report(geo_workspace("same-ratio", "same_ratio"))
        self.assertEqual(ambiguous["declared"]["fit"], "proper")
        self.assertEqual(ambiguous["discovery"]["finding"], "ambiguous")
        self.assertEqual(len(ambiguous["discovery"]["deck_keys"]), 2)
        _, duplicate = self.geo_report(geo_workspace("duplicate", "duplicate_deck"))
        self.assertEqual(duplicate["discovery"]["finding"], "found")
        self.assertEqual(len(duplicate["decks"]), 2)
        self.assertEqual(len({row["key"] for row in duplicate["decks"]}), 1)
        self.assertEqual(len(duplicate["discovery"]["fits"]), 4)

    def test_missing_inputs_or_named_candidate_prices_never_create_a_unique_explanation(self):
        for mode in ("missing_amount", "missing_candidate_price", "pending_price", "wrong_unit"):
            with self.subTest(mode=mode):
                _, check = self.geo_report(geo_workspace(mode, mode))
                self.assertEqual(check["status"], "unavailable")
                self.assertIsNone(check["discovery"])
                self.assertTrue(check["refusal"])
                self.assertEqual(len(check["decks"]), 2)
                if mode == "missing_candidate_price":
                    self.assertEqual(check["declared"]["fit"], "proper")
                    candidate = next(row for row in check["decks"] if not row["primary"])
                    self.assertTrue(candidate["refusal"])

    def test_candidate_prices_are_usable_without_its_unrelated_reserve_amount(self):
        envelope, check = self.geo_report(geo_workspace("candidate-amount", "missing_candidate_amount"))
        self.assertEqual(check["status"], "checked")
        self.assertEqual(check["discovery"]["finding"], "found")
        self.assertTrue(any(row["id"] == "candidate" for row in envelope["payload"]["issuer_reserves"]["refused"]))

    def test_source_bytes_are_authenticated_and_input_order_is_irrelevant(self):
        root = geo_workspace("byte-check")
        _, original = self.geo_report(root)
        _, reordered = self.geo_report(geo_workspace("reordered", reverse=True))
        self.assertEqual(original, reordered)
        path = root / "documents/Candidate_Deck.pdf"
        contents = path.read_bytes()
        path.write_bytes(bytes([contents[0] ^ 1]) + contents[1:])
        envelope, raw = invoke(root)
        self.assertEqual((envelope["exit_code"], envelope["refusal"]["code"]), (8, "FMINE-REF-505"), raw)
        self.assertIsNone(envelope["payload"])


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
