"""Independent number-format census of xlsx workbooks (oracle for bd-jg43.1; written before the Rust styles reader).

Spec:
- visible sheets only (state hidden/veryHidden skipped), as fidelity.py (E820);
- a cell is numeric exactly as in fidelity.py: not t="e", "s", "str" or "inlineStr", and its stripped <v> text
  (or inline <t> text) parses as a finite float;
- its format: the cell's s attribute (absent -> 0) indexes <cellXfs>; an index past the end is
  "bad style index <s>". The xf's numFmtId (absent -> 0) resolves to the <numFmts> code with that id if there is
  one, else the ECMA-376 18.8.30 built-in code, else "unknown numFmtId <id>". With no styles part every cell is
  "General";
- the styles part is the workbook relationship whose Type ends in /styles, falling back to xl/styles.xml;
- non-integer is decided on the stored text as an exact decimal (exponent accepted), never on a float.

Output: one JSON line per file, {"file","sha256","formats":{code:[numeric,non_integer]}}, then
{"files","formats","numeric_total","non_integer_total"} with formats summed over files.

--displayed (bd-jg43.2; written before the Rust interpreter). Each numeric cell's displayed text:
- the code splits on ";" outside quotes, escapes ("\\x"), padding ("_x"), fill ("*x") and brackets. More than four
  sections is unsupported; the fourth is text and is not read;
- each of the first three sections is read for digit placeholders 0 # ?, the first ".", "%" and quoted literals.
  Unsupported, refusing the whole code: an empty section, an E or e, an ASCII letter, "/", "@", a bracket
  other than a colour ([Red], [Color7]) and in particular a condition ([>100]), a second "." or "%", a comma
  after the last digit placeholder (scaling), more than 30 decimals. The zero section (third) with no 0
  placeholder and no "." shows its quoted literals as a token, and with none of them is unsupported. The first and
  second sections must hold a 0 placeholder;
- section by the stored value: the second for a negative when there are two or more sections, the third for
  zero when there are three or more, else the first. HYPOTHESIS (the LibreOffice cross-check decides): the
  sections format |v|, and the minus comes from the negative section or, with one section, from the value;
- value: the stored text as an exact decimal, x100 for "%", then rounded half away from zero to 15
  significant digits and then to d decimals. HYPOTHESIS, as above;
- text: "-" when negative, the digits with exactly d decimals, "%" for a percentage; or the token verbatim; or
  REFUSED:<code>, with codes general_non_integer, general_too_wide (General shows an integer of 10 or more
  digits by column width), unsupported_format, unknown_format_id, bad_style_index, out_of_range (the value
  times 10^d is 10^38 or more) and bad_stored_text. General shows an integer as itself;
- the per-file digest is SHA-256 over "{sheet}\\t{cell}\\t{text}\\n" for the numeric cells of the visible sheets,
  sheets in workbook order and cells in (row, column) order.
Output: per file {"file","sha256","shown","token","refused","displayed_digest"}, then {"files","shown","token",
"refused"}. With --cells as well, one {"file","sheet","cell","raw","format","text"} line per numeric cell
precedes each file line.
usage: python3 -I formats.py [--displayed [--cells]] <file.xlsx>...
"""
import hashlib, json, math, posixpath, re, sys, zipfile
from decimal import ROUND_HALF_UP, Context, Decimal, InvalidOperation, localcontext
from xml.etree import ElementTree as ET

M = "{http://schemas.openxmlformats.org/spreadsheetml/2006/main}"
R = "{http://schemas.openxmlformats.org/officeDocument/2006/relationships}"
P = "{http://schemas.openxmlformats.org/package/2006/relationships}"

# ECMA-376 Part 1, 18.8.30 (numFmt): the built-in formats the standard defines.
BUILTIN = {
    0: "General", 1: "0", 2: "0.00", 3: "#,##0", 4: "#,##0.00", 9: "0%", 10: "0.00%", 11: "0.00E+00",
    12: "# ?/?", 13: "# ??/??", 14: "mm-dd-yy", 15: "d-mmm-yy", 16: "d-mmm", 17: "mmm-yy", 18: "h:mm AM/PM",
    19: "h:mm:ss AM/PM", 20: "h:mm", 21: "h:mm:ss", 22: "m/d/yy h:mm", 37: "#,##0 ;(#,##0)",
    38: "#,##0 ;[Red](#,##0)", 39: "#,##0.00;(#,##0.00)", 40: "#,##0.00;[Red](#,##0.00)", 45: "mm:ss",
    46: "[h]:mm:ss", 47: "mmss.0", 48: "##0.0E+0", 49: "@",
}


def part(base, target):
    if target.startswith("/"):
        return target.lstrip("/")
    return posixpath.normpath(posixpath.join(base, target))


def styles_of(z, names, rels):
    key = None
    for r in rels:
        if r.get("Type", "").endswith("/styles"):
            key = part("xl", r.get("Target"))
    if key is None or key not in names:
        key = "xl/styles.xml" if "xl/styles.xml" in names else None
    if key is None:
        return None
    st = ET.fromstring(z.read(key))
    custom = {}
    nf = st.find(M + "numFmts")
    if nf is not None:
        for f in nf.findall(M + "numFmt"):
            custom[int(f.get("numFmtId"))] = f.get("formatCode")
    xfs = []
    cx = st.find(M + "cellXfs")
    if cx is not None:
        xfs = [int(x.get("numFmtId", "0")) for x in cx.findall(M + "xf")]
    return custom, xfs


def code_of(styles, s):
    if styles is None:
        return "General"
    custom, xfs = styles
    if s >= len(xfs):
        return "bad style index %d" % s
    fid = xfs[s]
    if fid in custom:
        return custom[fid]
    return BUILTIN.get(fid, "unknown numFmtId %d" % fid)


def non_integer(text):
    try:
        d = Decimal(text)
    except InvalidOperation:
        return False
    return d != d.to_integral_value()


COLOURS = {"black", "blue", "cyan", "green", "magenta", "red", "white", "yellow"}
BOUND = 10 ** 38


class Unsupported(Exception):
    pass


def split_sections(code):
    """The code's sections, each a list of (kind, text): kind is lit, esc, pad, fill, bracket or ch."""
    sections, cur, i = [], [], 0
    while i < len(code):
        ch = code[i]
        if ch == '"':
            j = code.find('"', i + 1)
            if j < 0:
                raise Unsupported("unterminated quote")
            cur.append(("lit", code[i + 1:j]))
            i = j + 1
        elif ch in "\\_*":
            if i + 1 >= len(code):
                raise Unsupported("dangling " + ch)
            cur.append(({"\\": "esc", "_": "pad", "*": "fill"}[ch], code[i + 1]))
            i += 2
        elif ch == "[":
            j = code.find("]", i)
            if j < 0:
                raise Unsupported("unterminated bracket")
            cur.append(("bracket", code[i + 1:j]))
            i = j + 1
        elif ch == ";":
            sections.append(cur)
            cur = []
            i += 1
        else:
            cur.append(("ch", ch))
            i += 1
    sections.append(cur)
    return sections


def read_section(toks, index):
    """(decimals, percent, token) of numeric section `index` (0, 1 or 2); token is None unless it shows one."""
    if not toks:
        raise Unsupported("empty section")
    decimals, percent, has_zero, point, literal, last_ph, commas = 0, False, False, False, "", -1, []
    for k, (kind, text) in enumerate(toks):
        if kind == "lit":
            literal += text
        elif kind == "bracket":
            low = text.lower()
            if text[:1] in ("<", ">", "="):
                raise Unsupported("condition [%s]" % text)
            if not (low in COLOURS or (low.startswith("color") and low[5:].isdigit())):
                raise Unsupported("bracket [%s]" % text)
        elif kind == "ch":
            if text in "0#?":
                last_ph = k
                has_zero = has_zero or text == "0"
                decimals += point
            elif text == ".":
                if point:
                    raise Unsupported("second decimal point")
                point = True
            elif text == "%":
                if percent:
                    raise Unsupported("second %")
                percent = True
            elif text == ",":
                commas.append(k)
            elif text in "eE":
                raise Unsupported("scientific notation")
            elif text.isascii() and text.isalpha():
                raise Unsupported("letter %s (date, time or keyword)" % text)
            elif text == "/":
                raise Unsupported("fraction")
            elif text == "@":
                raise Unsupported("text placeholder")
    if any(k > last_ph for k in commas):
        raise Unsupported("scaling comma")
    if decimals > 30:
        raise Unsupported("more than 30 decimals")
    if index == 2 and not has_zero and not point:
        if not literal:
            raise Unsupported("zero section shows no digit or literal")
        return decimals, percent, literal
    if not has_zero:
        raise Unsupported("no 0 placeholder")
    return decimals, percent, None


def parse_format(code):
    sections = split_sections(code)
    if len(sections) > 4:
        raise Unsupported("more than four sections")
    return [read_section(sections[i], i) for i in range(min(len(sections), 3))], len(sections)


CTX = Context(prec=1000, Emax=10 ** 6, Emin=-10 ** 6)


def displayed(raw, code):
    """(kind, text): kind is shown, token or refused."""
    if code.startswith("bad style index "):
        return "refused", "REFUSED:bad_style_index"
    if code.startswith("unknown numFmtId "):
        return "refused", "REFUSED:unknown_format_id"
    try:
        v = Decimal(raw)
    except InvalidOperation:
        return "refused", "REFUSED:bad_stored_text"
    sign = "-" if v < 0 else ""
    with localcontext(CTX):
        try:
            if code.lower() == "general":
                if v != v.to_integral_value():
                    return "refused", "REFUSED:general_non_integer"
                if abs(v) >= 10 ** 10:
                    return "refused", "REFUSED:general_too_wide"
                return "shown", sign + format(abs(v).quantize(Decimal(1)), "f")
            try:
                specs, n = parse_format(code)
            except Unsupported:
                return "refused", "REFUSED:unsupported_format"
            idx = 1 if v < 0 and n >= 2 else 2 if v == 0 and n >= 3 else 0
            d, pct, token = specs[idx]
            if token is not None:
                return "token", token
            negative = idx == 1 or (n == 1 and v < 0)
            m = abs(v)
            if pct:
                m = m.scaleb(2)
            if m:
                m = m.quantize(Decimal(1).scaleb(m.adjusted() - 14), rounding=ROUND_HALF_UP)
            m = m.quantize(Decimal(1).scaleb(-d), rounding=ROUND_HALF_UP)
            if m.scaleb(d) >= BOUND:
                return "refused", "REFUSED:out_of_range"
        except InvalidOperation:
            return "refused", "REFUSED:out_of_range"
    return "shown", ("-" if negative else "") + format(m, "f") + ("%" if pct else "")


def row_col(ref):
    m = re.fullmatch(r"([A-Z]+)([0-9]+)", ref or "")
    if not m:
        return None
    col = 0
    for ch in m.group(1):
        col = col * 26 + ord(ch) - 64
    return int(m.group(2)), col


def read_displayed(path, cells_out, into=None):
    """The file line; with `into` (a dict), also each numeric cell's (kind, text) by (sheet, cell)."""
    with open(path, "rb") as fh:
        data = fh.read()
    z = zipfile.ZipFile(path)
    names = set(z.namelist())
    wb = ET.fromstring(z.read("xl/workbook.xml"))
    rels = ET.fromstring(z.read("xl/_rels/workbook.xml.rels")).findall(P + "Relationship")
    target = {r.get("Id"): r.get("Target") for r in rels}
    styles = styles_of(z, names, rels)
    name = posixpath.basename(path)
    kinds = {"shown": 0, "token": 0, "refused": 0}
    listing = []
    for sh in wb.find(M + "sheets").findall(M + "sheet"):
        if sh.get("state", "visible") in ("hidden", "veryHidden"):
            continue
        key = part("xl", target[sh.get(R + "id")])
        if key not in names:
            continue
        by_coord = {}
        for c in ET.fromstring(z.read(key)).iter(M + "c"):
            t = c.get("t", "")
            if t in ("e", "s", "str", "inlineStr"):
                continue
            v = c.find(M + "v")
            if v is None:
                v = c.find(M + "is/" + M + "t")
            text = (v.text or "").strip() if v is not None else ""
            if not text:
                continue
            try:
                if not math.isfinite(float(text)):
                    continue
            except ValueError:
                continue
            rc = row_col(c.get("r"))
            if rc is None:
                continue
            code = code_of(styles, int(c.get("s", "0")))
            by_coord[rc] = (c.get("r"), text, code, displayed(text, code))
        for rc in sorted(by_coord):
            ref, text, code, (kind, shown) = by_coord[rc]
            kinds[kind] += 1
            if into is not None:
                into[(sh.get("name"), ref)] = (kind, shown)
            listing.append("%s\t%s\t%s\n" % (sh.get("name"), ref, shown))
            if cells_out:
                print(json.dumps({"file": name, "sheet": sh.get("name"), "cell": ref, "raw": text,
                                  "format": code, "text": shown}, ensure_ascii=False, separators=(",", ":")))
    return {"file": name, "sha256": hashlib.sha256(data).hexdigest(), **kinds,
            "displayed_digest": hashlib.sha256("".join(listing).encode("utf-8")).hexdigest()}


def read(path):
    with open(path, "rb") as fh:
        data = fh.read()
    z = zipfile.ZipFile(path)
    names = set(z.namelist())
    wb = ET.fromstring(z.read("xl/workbook.xml"))
    rels = ET.fromstring(z.read("xl/_rels/workbook.xml.rels")).findall(P + "Relationship")
    target = {r.get("Id"): r.get("Target") for r in rels}
    styles = styles_of(z, names, rels)
    counts = {}
    for sh in wb.find(M + "sheets").findall(M + "sheet"):
        if sh.get("state", "visible") in ("hidden", "veryHidden"):
            continue
        key = part("xl", target[sh.get(R + "id")])
        if key not in names:
            continue
        for c in ET.fromstring(z.read(key)).iter(M + "c"):
            t = c.get("t", "")
            if t in ("e", "s", "str", "inlineStr"):
                continue
            v = c.find(M + "v")
            if v is None:
                v = c.find(M + "is/" + M + "t")
            text = (v.text or "").strip() if v is not None else ""
            if not text:
                continue
            try:
                if not math.isfinite(float(text)):
                    continue
            except ValueError:
                continue
            code = code_of(styles, int(c.get("s", "0")))
            n = counts.setdefault(code, [0, 0])
            n[0] += 1
            n[1] += non_integer(text)
    return {"file": posixpath.basename(path), "sha256": hashlib.sha256(data).hexdigest(),
            "formats": dict(sorted(counts.items()))}


def main_displayed(paths, cells_out):
    total = {"shown": 0, "token": 0, "refused": 0}
    for p in paths:
        o = read_displayed(p, cells_out)
        print(json.dumps(o, ensure_ascii=False, separators=(",", ":")))
        for k in total:
            total[k] += o[k]
    print(json.dumps({"files": len(paths), **total}, separators=(",", ":")))


if __name__ == "__main__" and sys.argv[1:2] == ["--displayed"]:
    cells = sys.argv[2:3] == ["--cells"]
    main_displayed(sys.argv[3:] if cells else sys.argv[2:], cells)
elif __name__ == "__main__":
    total = {}
    files = 0
    for p in sys.argv[1:]:
        o = read(p)
        files += 1
        print(json.dumps(o, ensure_ascii=False, separators=(",", ":")))
        for code, (n, ni) in o["formats"].items():
            t = total.setdefault(code, [0, 0])
            t[0] += n
            t[1] += ni
    print(json.dumps({"files": files, "formats": dict(sorted(total.items())),
                      "numeric_total": sum(n for n, _ in total.values()),
                      "non_integer_total": sum(ni for _, ni in total.values())},
                     ensure_ascii=False, separators=(",", ":")))
