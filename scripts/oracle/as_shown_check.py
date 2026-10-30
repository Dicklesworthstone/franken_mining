"""LibreOffice "as shown" cross-check of the displayed values (bd-jg43.2).

It decides the hypotheses that formats.py --displayed and fmine-io's numfmt share: section choice by the
stored sign, 15 significant digits, then half away from zero. It compares their canonical text with what
LibreOffice prints when it saves CSV with "save cell contents as shown" (filter token 9) for every sheet (token
12 = -1, LibreOffice 7.2 or later).

Cells compared:
- a fixture workbook this script writes, holding the hypothesis cases (ties, the 15-digit case, a negative
  that rounds to zero, zero tokens). Its values are synthetic and always logged;
- 50 cells per registered file, stratified by format code with a fixed seed. Cells that formats.py refuses
  (General non-integers) are not sampled.

LibreOffice's text becomes canonical text: whitespace removed (padding and fill), grouping commas removed, and
"(x)" read as "-x". A disagreement is investigated. It is never settled by copying either reader's output.

RISK-31: issuer cell values are logged only with --values (owner machine). Without it, a registered-file
line carries the file, sheet, cell, format and the result, never a value.

Output: one JSON line per compared cell, then {"files","cells","disagreements","fixture_cells",
"fixture_disagreements"}. Exit 0 when nothing disagrees, 1 when something does, 2 when LibreOffice is missing
or a sheet's CSV is missing.
usage: python3 -I as_shown_check.py --soffice <path> [--values] <file.xlsx>...
"""
import csv, json, os, random, re, shutil, subprocess, sys, tempfile, zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
FILTER = "csv:Text - txt - csv (StarCalc):44,34,76,1,,0,false,true,true,false,false,-1"
SEED = "bd-jg43.2"
PER_FILE = 50

ACCOUNTING = '_(* #,##0_);_(* \\(#,##0\\);_(* "-"??_);_(@_)'
# (raw stored text, format code): the hypothesis cases.
FIXTURE = [
    ("1.0049999999999999", "0.00"),   # 15 significant digits first: 1.01, not 1.00
    ("0.28499999999999998", "0.00"),  # the same: 0.29
    ("2.6749999999999998", "0.00"),   # the same: 2.68
    ("1.005", "0.00"),                # the text 1.005 is stored as the double below 1.005
    ("0.125", "0.00"),                # an exact binary half: 0.13
    ("2.5", "0"),                     # half away from zero: 3
    ("-2.5", "0"),                    # -3
    ("0.5", "0"),                     # 1
    ("-0.3", "0"),                    # one section: -0
    ("-0.3", "#,##0_);\\(#,##0\\)"),  # negative section: (0), read as -0
    ("-0.3", ACCOUNTING),             # (0), read as -0
    ("0.0001", ACCOUNTING),           # not zero: 0, not the token
    ("0", ACCOUNTING),                # the zero token -
    ("-37.899642999999998", ACCOUNTING),  # E823 K8: (38)
    ("0.005", "0.00%"),               # 0.50%
    ("0.12345", "0.00%"),             # 12.35%
    ("1234567.125", "#,##0.00"),      # 1,234,567.13
]
BUILTIN_IDS = {"0": 1, "0.00": 2, "#,##0": 3, "#,##0.00": 4, "0%": 9, "0.00%": 10}


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


def write_fixture(path):
    codes = sorted({code for _, code in FIXTURE})
    custom = {c: 164 + i for i, c in enumerate(c for c in codes if c not in BUILTIN_IDS)}
    xf_of = {c: i + 1 for i, c in enumerate(codes)}
    num_fmts = "".join('<numFmt numFmtId="%d" formatCode="%s"/>' % (i, esc(c)) for c, i in custom.items())
    xfs = '<xf numFmtId="0"/>' + "".join(
        '<xf numFmtId="%d" applyNumberFormat="1"/>' % (BUILTIN_IDS.get(c) or custom[c]) for c in codes)
    main = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
    rel = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
    styles = ('<?xml version="1.0" encoding="UTF-8"?><styleSheet xmlns="%s"><numFmts count="%d">%s</numFmts>'
              '<fonts count="1"><font/></fonts><fills count="1"><fill/></fills><borders count="1"><border/>'
              '</borders><cellStyleXfs count="1"><xf/></cellStyleXfs><cellXfs count="%d">%s</cellXfs>'
              '</styleSheet>') % (main, len(custom), num_fmts, len(codes) + 1, xfs)
    rows = "".join('<row r="%d"><c r="A%d" s="%d"><v>%s</v></c></row>' % (i + 1, i + 1, xf_of[c], raw)
                   for i, (raw, c) in enumerate(FIXTURE))
    sheet = '<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="%s"><sheetData>%s</sheetData></worksheet>' % (
        main, rows)
    workbook = ('<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="%s" xmlns:r="%s"><sheets>'
                '<sheet name="H" sheetId="1" r:id="rId1"/></sheets></workbook>') % (main, rel)
    pkg = "http://schemas.openxmlformats.org/package/2006/relationships"
    wb_rels = ('<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="%s">'
               '<Relationship Id="rId1" Type="%s/worksheet" Target="worksheets/sheet1.xml"/>'
               '<Relationship Id="rId2" Type="%s/styles" Target="styles.xml"/></Relationships>') % (pkg, rel, rel)
    root_rels = ('<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="%s">'
                 '<Relationship Id="rId1" Type="%s/officeDocument" Target="xl/workbook.xml"/>'
                 '</Relationships>') % (pkg, rel)
    ct = "application/vnd.openxmlformats-officedocument.spreadsheetml"
    types = ('<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/'
             'content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.'
             'relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>'
             '<Override PartName="/xl/workbook.xml" ContentType="%s.sheet.main+xml"/>'
             '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="%s.worksheet+xml"/>'
             '<Override PartName="/xl/styles.xml" ContentType="%s.styles+xml"/></Types>') % (ct, ct, ct)
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for name, body in [("[Content_Types].xml", types), ("_rels/.rels", root_rels),
                           ("xl/workbook.xml", workbook), ("xl/_rels/workbook.xml.rels", wb_rels),
                           ("xl/styles.xml", styles), ("xl/worksheets/sheet1.xml", sheet)]:
            z.writestr(name, body.encode("utf-8"))


def oracle_cells(path):
    out = subprocess.run([sys.executable, "-I", os.path.join(HERE, "formats.py"), "--displayed", "--cells", path],
                         check=True, capture_output=True, text=True).stdout
    return [json.loads(line) for line in out.splitlines() if '"cell"' in line]


def sample(cells, name):
    by_format = {}
    for c in cells:
        if not c["text"].startswith("REFUSED:"):
            by_format.setdefault(c["format"], []).append(c)
    rng = random.Random("%s:%s" % (SEED, name))
    queues = []
    for code in sorted(by_format):
        group = list(by_format[code])
        rng.shuffle(group)
        queues.append(group)
    picked = []
    while len(picked) < PER_FILE and any(queues):
        for q in queues:
            if q and len(picked) < PER_FILE:
                picked.append(q.pop())
    return picked


def canonical(shown):
    t = re.sub(r"\s+", "", shown.replace(" ", " "))
    negative = False
    if t.startswith("(") and t.endswith(")"):
        negative, t = True, t[1:-1]
    elif t.startswith("-") and t[1:2].isdigit():
        negative, t = True, t[1:]
    return ("-" if negative else "") + t.replace(",", "")


def row_col(ref):
    m = re.fullmatch(r"([A-Z]+)([0-9]+)", ref)
    col = 0
    for ch in m.group(1):
        col = col * 26 + ord(ch) - 64
    return int(m.group(2)), col


def as_shown(soffice, path, outdir):
    """{sheet: rows} from LibreOffice's per-sheet CSV of `path`, saved as shown."""
    profile = "file://" + os.path.join(outdir, "profile")
    subprocess.run([soffice, "-env:UserInstallation=" + profile, "--headless", "--norestore", "--convert-to",
                    FILTER, "--outdir", outdir, path], check=True, capture_output=True)
    stem = os.path.splitext(os.path.basename(path))[0]
    sheets = {}
    for f in os.listdir(outdir):
        if f.startswith(stem + "-") and f.endswith(".csv"):
            with open(os.path.join(outdir, f), newline="", encoding="utf-8") as h:
                sheets[f[len(stem) + 1:-4]] = list(csv.reader(h))
    return sheets


def lookup(sheets, sheet, ref):
    if sheet not in sheets:
        print(json.dumps({"error": "no CSV for sheet", "sheet": sheet}), file=sys.stderr)
        sys.exit(2)
    r, c = row_col(ref)
    rows = sheets[sheet]
    return rows[r - 1][c - 1] if r <= len(rows) and c <= len(rows[r - 1]) else ""


def main(argv):
    if len(argv) < 2 or argv[0] != "--soffice":
        sys.exit(__doc__)
    soffice, rest = argv[1], argv[2:]
    values = rest[:1] == ["--values"]
    files = rest[1:] if values else rest
    if not (shutil.which(soffice) or os.access(soffice, os.X_OK)):
        print(json.dumps({"error": "LibreOffice not found", "soffice": soffice}))
        sys.exit(2)
    work = tempfile.mkdtemp(prefix="as_shown_")
    fixture = os.path.join(work, "fixture.xlsx")
    write_fixture(fixture)
    totals = {"files": len(files), "cells": 0, "disagreements": 0, "fixture_cells": 0, "fixture_disagreements": 0}
    for path, is_fixture in [(fixture, True)] + [(f, False) for f in files]:
        name = os.path.basename(path)
        outdir = os.path.join(work, "csv", name)
        os.makedirs(outdir)
        sheets = as_shown(soffice, path, outdir)
        cells = oracle_cells(path)
        for c in (cells if is_fixture else sample(cells, name)):
            lo = lookup(sheets, c["sheet"], c["cell"])
            agree = canonical(lo) == c["text"]
            line = {"file": name, "sheet": c["sheet"], "cell": c["cell"], "format": c["format"]}
            if is_fixture or values:
                line.update(raw=c["raw"], oracle=c["text"], libreoffice=lo)
            line["agree"] = agree
            print(json.dumps(line, ensure_ascii=False, separators=(",", ":")))
            prefix = "fixture_" if is_fixture else ""
            totals[prefix + "cells"] += 1
            totals[prefix + "disagreements"] += not agree
    print(json.dumps(totals, separators=(",", ":")))
    sys.exit(1 if totals["disagreements"] or totals["fixture_disagreements"] else 0)


if __name__ == "__main__":
    main(sys.argv[1:])
