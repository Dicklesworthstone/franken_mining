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
    value = compact(payload).rsplit('"digest":"', 1)[1].split('"', 1)[0]
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
        self.assertIn('"refused":[]', text)
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
        self.assertEqual((root / "documents" / FILE).read_bytes(), DOCUMENT)
        for field in ('"corrected":', '"best":', '"fix":'):
            self.assertNotIn(field, text)
        write_readings(root, "closing75")
        changed, _ = successful(root)
        self.assertEqual(compact(changed["payload"]).count('"verdict":"feasible"'), 2)
        self.assertNotEqual(conflict["payload"], changed["payload"])
        self.assertNotEqual(report_digest(conflict["payload"]), report_digest(changed["payload"]))

    def test_missing_and_single_reader_operands_remain_refused_identity_members(self):
        root = workspace("missing")
        for mode in ("missing_opening", "pending_opening"):
            with self.subTest(mode=mode):
                write_readings(root, mode)
                envelope, _ = successful(root)
                text = compact(envelope["payload"])
                self.assertIn('"identity":"reserve_rollforward"', text)
                self.assertIn("opening", text)
                self.assertNotIn('"refused":[]', text)
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
