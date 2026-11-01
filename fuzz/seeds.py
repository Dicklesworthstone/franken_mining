"""Write the starting corpus of every fuzz target (see scripts/fuzz.sh).

Usage: python3 -I fuzz/seeds.py <repo-root> <out-dir>

Each target gets a directory of seed files in its own input format (fuzz_targets/*.rs): a selector byte, where the
target has one, then the bytes. All content is invented or copied from committed registries and rules; no issuer
value, workbook or transcription is read.
"""

import io
import random
import re
import struct
import sys
import zipfile
from pathlib import Path


def crc32c(data: bytes) -> int:
    crc = 0xFFFFFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = (crc >> 1) ^ (0x82F63B78 & -(crc & 1))
    return crc ^ 0xFFFFFFFF


def frame(tag: bytes, version: int, body: bytes) -> bytes:
    head = b"FMNE" + tag + struct.pack("<H", version) + body
    return head + struct.pack("<I", crc32c(head))


def put_str(s: str) -> bytes:
    b = s.encode()
    return struct.pack("<I", len(b)) + b


MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PKG = "http://schemas.openxmlformats.org/package/2006/relationships"


def workbook(compression: int) -> bytes:
    """A small invented workbook: a visible and a hidden sheet, styles with custom formats, shared strings with a
    rich-text run and entities, inline strings, styled numbers and errors, and an entry the reader ignores."""
    parts = {
        "[Content_Types].xml": '<?xml version="1.0" encoding="UTF-8"?><Types/>',
        "xl/workbook.xml": (
            f'<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets>'
            '<sheet name="Summary" sheetId="1" r:id="rId1"/>'
            '<sheet name="Workings" sheetId="2" state="hidden" r:id="rId2"/></sheets></workbook>'
        ),
        "xl/_rels/workbook.xml.rels": (
            f'<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="{PKG}">'
            '<Relationship Id="rId1" Target="worksheets/sheet1.xml"/>'
            '<Relationship Id="rId2" Target="worksheets/sheet2.xml"/>'
            f'<Relationship Id="rId3" Type="{REL}/styles" Target="styles.xml"/></Relationships>'
        ),
        "xl/styles.xml": (
            f'<?xml version="1.0" encoding="UTF-8"?><styleSheet xmlns="{MAIN}"><numFmts count="3">'
            '<numFmt numFmtId="164" formatCode="#,##0.0_);(#,##0.0);&quot;-&quot;_)"/>'
            '<numFmt numFmtId="165" formatCode="0.00%"/><numFmt numFmtId="166" formatCode="0.0E+00"/></numFmts>'
            '<cellXfs count="5"><xf numFmtId="0"/><xf numFmtId="164"/><xf numFmtId="165"/><xf numFmtId="166"/>'
            '<xf numFmtId="4"/></cellXfs></styleSheet>'
        ),
        "xl/sharedStrings.xml": (
            f'<?xml version="1.0" encoding="UTF-8"?><sst xmlns="{MAIN}" count="3" uniqueCount="3">'
            "<si><t>Site A</t></si><si><r><t>Gold </t></r><r><t>produced (koz)</t></r></si>"
            '<si><t xml:space="preserve">AISC &amp;lt;$/oz&gt;</t></si></sst>'
        ),
    }
    rows = ['<row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="s"><v>2</v></c></row>']
    for i in range(2, 24):
        err = f'<c r="E{i}" t="e"><v>#REF!</v></c>' if i % 7 == 0 else ""
        rows.append(
            f'<row r="{i}"><c r="A{i}" t="inlineStr"><is><t>Row {i}</t></is></c><c r="B{i}" s="1"><v>{i * 12.5}</v></c>'
            f'<c r="C{i}" s="2"><v>0.{i:02d}</v></c><c r="D{i}" s="3"><v>{i}E+3</v></c>{err}</row>'
        )
    parts["xl/worksheets/sheet1.xml"] = (
        f'<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="{MAIN}"><sheetData>{"".join(rows)}</sheetData>'
        "</worksheet>"
    )
    parts["xl/worksheets/sheet2.xml"] = (
        f'<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="{MAIN}"><sheetData>'
        '<row r="1"><c r="A1" s="4"><v>-37.899642999999998</v></c><c r="AA102"><v>-0.125</v></c></row>'
        "</sheetData></worksheet>"
    )
    rng = random.Random(20261101)
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        for name, text in parts.items():
            info = zipfile.ZipInfo(name, date_time=(2026, 11, 1, 0, 0, 0))
            z.writestr(info, text.encode(), compress_type=compression)
        info = zipfile.ZipInfo("xl/media/c.bin", date_time=(2026, 11, 1, 0, 0, 0))
        z.writestr(info, bytes(rng.randrange(256) for _ in range(200)), compress_type=compression)
    return buf.getvalue()


def main() -> None:
    root, out = Path(sys.argv[1]), Path(sys.argv[2])
    seeds: dict[str, list[bytes]] = {t: [] for t in
                                     ("codec", "anchor", "journal", "xlsx", "numfmt", "registry", "record", "html")}

    # codec: mode, script length, script (one read per byte), payload.
    rng = random.Random(17)
    payload = bytes(rng.randrange(256) for _ in range(160))
    for mode in (0, 1, 2, 5):
        script = bytes(range(10)) + bytes([0x18, 0x29, 0x38, 0x49, 0xF8, 0xF9])
        seeds["codec"].append(bytes([mode, len(script)]) + script + payload)

    # anchor: selector 2 wraps the body in a version-0 frame, selector 5 in a version-1 frame.
    common = put_str("cell-a") + struct.pack("<QQIIII", 3, 41, 1, 2, 3, 4)
    seeds["anchor"].append(b"\x02" + common + bytes(range(32)))
    seeds["anchor"].append(b"\x05" + common + struct.pack("<I", 5) + bytes(range(32)))
    seeds["anchor"].append(b"\x00" + frame(b"ANCR", 1, common + struct.pack("<I", 5) + bytes(range(32))))

    # journal: a forecast genesis record as a wrapped body (selector 8), a hex line (1), a one-line journal (2), and a
    # reliance body that starts with a zero previous head (11).
    genesis = struct.pack("<Q", 0) + bytes(32) + struct.pack("<q", 1_790_000_000_000_000_000) + b"\x00"
    genesis += put_str("cell-a") + struct.pack("<Q", 1_000_000_000)
    line = frame(b"FJR1", 1, genesis).hex().encode() + b"\n"
    seeds["journal"] += [b"\x08" + genesis, b"\x01" + line, b"\x02" + line, b"\x0b" + bytes(32) + b"\x00"]

    # xlsx: the same invented workbook deflated and stored.
    seeds["xlsx"] += [workbook(zipfile.ZIP_DEFLATED), workbook(zipfile.ZIP_STORED)]

    # numfmt: selector, format code, newline, stored cell text.
    codes = ["General", "0", "0.00", "#,##0", "#,##0.0_);(#,##0.0);\"-\"_)", "0.00%", "0.0E+00", "[Red]0.0",
             "#,##0;[Red]-#,##0", "\"$\"#,##0.00", "0.0\" koz\"", "@", "#,##0,", "0.000;-0.000;0"]
    raws = ["1234.5", "-37.899642999999998", "0", "1.5E+3", "12345678901", "0.0425", "-0.125"]
    for i, code in enumerate(codes):
        seeds["numfmt"].append(bytes([i]) + f"{code}\n{raws[i % len(raws)]}".encode())

    # registry: selector 0 the subset parser (every committed registry), 1 sources, 2 table identities, 4 calendars,
    # 5 metrics.
    reg = root / "registries"
    for p in sorted(reg.glob("*.toml")):
        seeds["registry"].append(b"\x00" + p.read_bytes())
    sources = (reg / "sources" / "barrick.toml").read_bytes()
    seeds["registry"].append(b"\x01" + sources)
    m = re.search(rb'\[\[document\]\][^\[]*?\nfile = "([^"]+)"', sources)
    doc = m.group(1).decode() if m else "Report.pdf"
    identities = (
        'schema = "fmine.table-identities.v1"\nissuer = "barrick"\n[[identity]]\nid = "reserves"\n'
        f'doc = "{doc}"\nfamily = "reserves"\nkind = "roll_forward"\ntotal = "closing"\ntotal_period = "2030FY"\n'
        'unit = "moz"\nbasis = "attributable;contained"\nconstruction = "rounded_once"\nconvention = "closed"\n'
        '[[term]]\nidentity = "reserves"\nquantity = "opening"\nperiod = "2029FY"\nsign = 1\nrole = "opening"\n'
        '[[term]]\nidentity = "reserves"\nquantity = "depletion"\nperiod = "2030FY"\nsign = 1\nrole = "move"\n'
    )
    seeds["registry"].append(b"\x02" + identities.encode())
    seeds["registry"].append(b"\x04" + (reg / "calendars.toml").read_bytes())
    seeds["registry"].append(b"\x04" + (reg / "zones.toml").read_bytes())
    seeds["registry"].append(b"\x05" + (reg / "metrics.toml").read_bytes())

    # record: the committed rules (0), an invented manifest (1) and forecasts.csv (2).
    rules = (root / "forecasts" / "fy2026" / "rules.toml").read_bytes()
    seeds["record"].append(b"\x00" + rules)
    manifest = (
        'record_id = "FM-SYNTHETIC-v1"\nauthor_role = "public-lane-author"\n'
        'information_cutoff_utc = "2026-10-01T00:00:00Z"\nlabel = "forecast"\ncontact = "pre-contact"\n'
        'attestation = "No non-public issuer information was used to make these forecasts."\n'
        f'rules_sha256 = "{"cd" * 32}"\n\n[[input]]\ndoc_id = "Synthetic_Workbook.xlsx"\n'
        'url = "https://example.invalid/synthetic.xlsx"\npublication_date = "2026-08-10"\n'
        f'retrieved_utc = "2026-09-01T12:00:00Z"\nbytes = 1_524\nsha256 = "{"ab" * 32}"\n'
    )
    seeds["record"].append(b"\x01" + manifest.encode())
    live = re.findall(rb'name = "([^"]+)"\nstatus = "live"', rules)
    csv = "site,target,point,p10,p90,uncalibrated\n" + "".join(
        f"{s},production,100,90,110,true\n{s},unit_cost,1500,1350.5,1700,true\n" for s in (n.decode() for n in live))
    seeds["record"].append(b"\x02" + csv.encode())

    # html: two invented tables.
    seeds["html"].append(
        b"<html><body><table><tr><th>Site</th><th>Gold (koz)</th></tr><tr><td>A</td><td>1,234</td></tr>"
        b"<tr><td colspan=\"2\">Total &amp; more</td></tr></table><p>x</p><table><tr><td>1</td></tr></table>"
        b"</body></html>")
    seeds["html"].append(b"<table><thead><tr><th>a<th>b</thead><tbody><tr><td>1<td>2</tbody></table>")

    for target, items in seeds.items():
        d = out / target
        d.mkdir(parents=True, exist_ok=True)
        for i, item in enumerate(items):
            (d / f"seed-{i:02d}").write_bytes(item)
        print(f'{{"target":"{target}","seeds":{len(items)}}}')


if __name__ == "__main__":
    main()
