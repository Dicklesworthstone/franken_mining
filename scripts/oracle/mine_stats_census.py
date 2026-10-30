"""Mine Statistics census: ownership and quarter-sum identities on the QTD/YTD summary sheets, each instance
decided alone on its own displayed cells, then all decided instances posed jointly by shared cells, with every
minimum correction set of each jointly infeasible component.

Independent reader, written 2026-10-30 without opening any Rust under crates/, planning/mor/MOR_CASE.md, repro/ or
any other agent's scratch files. It reads the xlsx itself (zipfile + xml.etree) and imports formats.py only for its
number-format interpreter (formats.read_displayed, formats.part, formats.row_col). Every block, header, row and
operand it uses is located here, by text. It is the reference the Rust rule must agree with instance by instance.

usage: python3 -I scripts/oracle/mine_stats_census.py [--values] <dir-or-files...>

Rule (as given to this script's author, verbatim in substance)
==============================================================
Input. The 10 Mine Statistics workbooks Barrick_Q{1..4}_{2024,2025}_Mine_Stats.xlsx, Barrick_Q1_2026_Mine_Stats.xlsx
and Barrick_Q2_2026_Mine_Stats.xlsx (the *_AISC.xlsx files are ignored). The release quarter of a file comes from its
name: Barrick_Q3_2025_Mine_Stats.xlsx -> 2025Q3. Locator: <release quarter>:<sheet>!<A1>, e.g. 2026Q1:QTD!B5.

Scope. Only the summary sheets named QTD and YTD (Q1 releases have only QTD). Locate everything by text, never by a
fixed row or column number:
- A gold block and a copper block. Each has a year header row (the years, e.g. 2026 over columns B-G and 2025 over
  I-N) and a column-header row whose texts name the fields. The flow fields are "Gold produced (000s ozs 100%
  basis)", "Gold produced (000s ozs attributable share)", "Gold sold (000s ozs attributable share)" and the copper
  equivalents "Copper produced (kt 100% basis)", "Copper produced (kt attributable share)", "Copper sold (kt
  attributable share)" (match on the meaning - 100% basis production / attributable production / attributable sold
  - tolerating footnote digits, whitespace and case; refuse with SystemExit naming the cell if a header cannot be
  mapped). Unit-cost columns (cost of sales, cash costs / C1, AISC) are out of scope.
- Two column groups per block: current (the columns under the release's own year) and prior (the columns under the
  previous year). Group membership comes from the year header, not from letters.
- Data rows: rows of the block with a subject label in column A, excluding footnote rows (column-A text starting
  with a digit and a space) and header rows. The subject of a row = its column-A label with a trailing 1-2 digit
  footnote number stripped (^(.*?[^\\d\\s])(\\d{1,2})$, whitespace collapsed) and every (x%) share removed, then
  trimmed. The printed share is the (x%) number, if any (as an exact decimal).
- Period of a cell: QTD = the three months of the release quarter; YTD = months 6, 9 or 12 ending at the release
  quarter (H1, 9M, FY). Current group -> the release's year; prior group -> the year before.

Values. Each operand cell is read as displayed:
- a numeric cell whose formats.py kind is "shown": its displayed text is a number; its value is that exact decimal
  and its rounding step is 10^-d, d the number of decimals displayed (d = 0 -> step 1). Half-width h = step/2. A
  displayed percentage in a flow column -> SystemExit naming the cell;
- a placeholder: a text cell whose whitespace-stripped text is one of "—", "–", "-", "n/a", "N/A", or a numeric
  cell whose formats.py kind is "token" (a number shown as a dash);
- blank: no cell or an empty one;
- anything else (other text, formats.py "refused"): SystemExit naming the cell.

Family ownership - attributable production = s x 100%-basis production.
- One instance per (sheet, data row, column group) where the row is not an aggregate. Aggregates are the region
  rows ("North America", "Latin America & Asia Pacific", "South America & Asia Pacific", "Africa & Middle East")
  and "Total Gold", "Total Copper"; they get no ownership instance.
- Operands, in order: the 100% cell (B side) then the attributable cell (C side) of that row and group.
- Share s = printed share / 100. Exception (W1-DR-01): subject "Porgera" printed as 47.5 uses s = 24.5/100, and the
  record keeps the printed 47.5 beside the share used. A row with no printed share -> v, reason share_unprinted.
- Identity: true c = s.b exactly, true b in [B-h_B, B+h_B], true c in [C-h_C, C+h_C] (closed). Consistent iff the
  intervals s.[B-h_B, B+h_B] and [C-h_C, C+h_C] intersect.

Family quarter_sum - the cumulative = the previous cumulative + the quarter: H1 = Q1 + Q2, 9M = H1 + Q3,
FY = 9M + Q4.
- Anchored on each cumulative cell: every data row (aggregates included) x each of the three flow fields x each
  column group, on every YTD sheet. The YTD sheet's release quarter fixes n (Q2 -> H1, Q3 -> 9M, Q4 -> FY).
- Operands, in order: the previous cumulative (H1: the Q1 release's QTD cell; 9M: the Q2 release's YTD cell; FY:
  the Q3 release's YTD cell), the quarter (the anchor release's own QTD cell), the total (the anchor YTD cell). All
  from releases of the SAME year as the anchor release, the same column group, the same field, and the row whose
  subject equals the anchor's subject. Never mix groups, never use a release from another year.
- An operand release file not in the input set -> SystemExit.
- Identity: true t = a + q exactly; consistent iff |A + Q - T| <= h_A + h_Q + h_T.

Exclusions, by name, applied before anything else, to both families (state v with that reason):
- excluded_e818: subject "Loulo-Gounkoto" when any operand's period is 2025Q1, 2025Q2, 2025Q3, 2025 H1 or 2025 9M
  (E818), in both column groups.
- cross_map_region (quarter_sum only): a region subject whose member set differs between the release quarters of its
  operands. Region map keyed by the file's release quarter (never a cell's period): releases through 2025Q3: North
  America = {Nevada Gold Mines, Hemlo}; Latin America & Asia Pacific = {Pueblo Viejo, Veladero, Porgera}; Africa &
  Middle East = {Loulo-Gounkoto, Kibali, North Mara, Tongon, Bulyanhulu}. Releases from 2025Q4: North America =
  {Nevada Gold Mines, Pueblo Viejo, Hemlo}; South America & Asia Pacific = {Veladero, Porgera}; Africa & Middle
  East as above. A region not in the map for a quarter has the empty member set there.

States, in this order of precedence (after exclusions):
1. quarter_sum: an operand release whose sheet has no row with the anchor's subject (same block) -> v,
   operand_row_missing.
2. Any operand blank -> v, blank_operand, at = the first blank operand in operand order.
3. All operands placeholders -> np.
4. Some but not all placeholders -> v, placeholder_operand, at = the first placeholder operand.
5. All numbers -> decide the identity alone on its own cells: consistent -> i; not -> iii. (share_unprinted applies
   here, only when all operands are numbers.)

Components. Every instance in state i or iii is posed jointly; instances sharing a cell are linked; connected
components are decided exactly: do true values exist, each in its closed box, satisfying every identity of the
component (c = s.b, t = a + q)? For each infeasible component: every minimum correction set (smallest k such that
dropping the boxes of some k cells makes it feasible; exhaustive by size; sets sorted), and with --values, for each
set and cell, the implied interval (exact range of the cell's true value with that set's boxes dropped). joint_only
if every instance of the component is i. Every iii instance carries its component id and corrections.

Output (stdout, one compact JSON object per line, ensure_ascii=False, deterministic order):
1. one line per instance sorted by (family, anchor, group, field): {"family","subject","field","group","period",
   "anchor","cells","state","reason","at","share","printed_share","component","corrections"}; ownership's field is
   the attributable field and its anchor the attributable cell; with --values also "values" (displayed operand
   texts) and "implied" ([{locator: [lo, hi]}] per correction set);
2. one line per jointly infeasible component {"component","cells","instances","joint_only","corrections"};
3. per family {"family","n","i","iii","v","np","v_by_reason"}, then {"components","infeasible_components",
   "joint_only"};
4. known-answer checks {"check","ok","detail"}: E113, E114, E115, computed from the instances.
RISK-31: no issuer cell value and no implied interval is printed unless --values is given.

Method for the joint decision (exact): the component's identities are homogeneous linear equalities. Gauss-Jordan
elimination over Fractions (pivot columns in sorted-locator order) writes every cell's true value as a linear form in
the free cells; each kept box becomes two inequalities on the free cells; Fourier-Motzkin elimination of every free
variable (keeping, per normalised direction, only the tightest right-hand side) decides feasibility exactly. The
implied range of a cell substitutes the cell's own value for one free variable it depends on and eliminates the
rest; what remains bounds the cell (a side with no bound is "unbounded").

Judgement calls (the rule is silent or ambiguous here)
======================================================
J1 Period labels: quarters "<year>Q<n>" (2025Q1), cumulatives "<year> H1", "<year> 9M", "<year> FY", spelled as the
   rule spells its E818 list. A quarter_sum record's period is its anchor's (the cumulative).
J2 Blocks: a column-header row is a row (column A excluded) holding at least one flow header; every other non-empty
   cell of that row must map to a flow field or to a unit-cost column (text with "$/oz" or "$/lb" and "cost of
   sales", "cash costs" or "sustaining costs"), else SystemExit. Footnote digits in headers are digits glued to the
   end of a word or ")" ("produced6", "($/oz)1"); "000s" and "100%" are kept. A flow header must name exactly one
   metal (gold or copper), the unit (000s ozs / kt) of that metal and exactly one meaning: produced + "100% basis";
   produced + "attributable"; sold + "attributable". Each sheet must hold exactly one gold and one copper block,
   each with exactly one column for each (group, flow field), else SystemExit.
J3 The year header row of a block is the nearest row above its column-header row whose column-A text begins
   "For the" (case-insensitive) and below the previous block's column-header row. Its column-A text must name
   exactly one of three/six/nine/twelve, which must match the sheet (QTD: three; YTD: six, nine, twelve for a Q2,
   Q3, Q4 release; a YTD sheet in a Q1 release is refused).
J4 A field column's year: the merged range in the year header row that covers the column (its top-left cell holds
   the year); with no such merge, the nearest non-empty cell at or left of the column in that row. That cell must be
   a 4-digit year (number or text) equal to the release's year (current) or the year before (prior), else
   SystemExit.
J5 A block's data rows run from its column-header row + 1 up to (excluding) the first of: the next block's year
   header row, or the first footnote row after the column-header row; with neither, SystemExit. Inside, a row with no
   column-A text is skipped, but if any of its field cells is not blank, SystemExit; a numeric column-A cell or a
   "For the" row inside a block is refused. A footnote row is column-A text matching ^[0-9]+\\s after stripping
   leading whitespace (one or more digits, then whitespace).
J6 Subjects: whitespace collapsed first, then the footnote regex, then every "(x%)" ((digits[.digits] %) with
   optional inner spaces) removed, whitespace collapsed again and trimmed. Two shares in one label, or two rows of
   one block with the same subject, are refused. share and printed_share are the printed decimal text (e.g. "61.5",
   "60"); for the Porgera exception share is "24.5" and printed_share "47.5".
J7 Blank: no cell, a cell with no value, or a text cell whose text is exactly "". A whitespace-only text that is not a
   placeholder is "anything else" (SystemExit). Error (t="e") and boolean (t="b") cells are "anything else". Every
   flow cell of every data row is classified when its sheet is read (each one is an operand of some instance), so a
   cell that is "anything else" stops the run even if its instance would be excluded.
J8 operand_row_missing, share_unprinted, excluded_e818 and cross_map_region have at = null. In "cells" (and
   "values") an operand whose row does not exist is null. Exclusions are tested in the order excluded_e818, then
   cross_map_region (they cannot both apply: Loulo-Gounkoto is not a region). A region subject is any key of either
   map; "Total Gold" and "Total Copper" are aggregates but not regions, so never cross_map_region.
J9 Components are formed over the instances in state i or iii only. "component" and "corrections" are set on every
   instance (i or iii) of a jointly infeasible component and null otherwise; a feasible component prints no line.
   The component id is the smallest locator among its cells (plain string order). Locator lists, sets and lists of
   sets are sorted in plain string (code point) order. "instances" lists the anchors of the component's instances in
   instance output order, so an ownership and a quarter_sum instance anchored on the same cell both appear.
J10 Implied bounds are strings: an integer "281", a fraction "563/2", or "unbounded". With --values the component
   line carries "implied" too. The summary's "components" counts the connected components of posed instances.
J11 Input: a directory contributes its direct children named exactly Barrick_Q<n>_<yyyy>_Mine_Stats.xlsx; a file
   argument must have that name. Two files of one release quarter, or a QTD/YTD sheet that is hidden (formats.py
   reads visible sheets only), are refused. QTD is required in every release, YTD in every Q2-Q4 release.
J12 Known-answer checks are read from the finished instance records. Without --values, E113's "281 lies in the
   implied interval" is not evaluated and "ok" covers the other conditions (the detail says so).
"""
import itertools
import json
import os
import re
import sys
import zipfile
from fractions import Fraction as F
from xml.etree import ElementTree as ET

sys.dont_write_bytecode = True  # leave no __pycache__ entries behind in the repository
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import formats  # noqa: E402

M, R, P = formats.M, formats.R, formats.P
FILE_RE = re.compile(r"Barrick_Q([1-4])_([0-9]{4})_Mine_Stats\.xlsx")
TEXT_TYPES = ("s", "inlineStr", "str")
PLACE = {"—", "–", "-", "n/a", "N/A"}
AGGREGATES = {"North America", "Latin America & Asia Pacific", "South America & Asia Pacific",
              "Africa & Middle East", "Total Gold", "Total Copper"}
AFRICA = frozenset({"Loulo-Gounkoto", "Kibali", "North Mara", "Tongon", "Bulyanhulu"})
MAP_TO_2025Q3 = {
    "North America": frozenset({"Nevada Gold Mines", "Hemlo"}),
    "Latin America & Asia Pacific": frozenset({"Pueblo Viejo", "Veladero", "Porgera"}),
    "Africa & Middle East": AFRICA,
}
MAP_FROM_2025Q4 = {
    "North America": frozenset({"Nevada Gold Mines", "Pueblo Viejo", "Hemlo"}),
    "South America & Asia Pacific": frozenset({"Veladero", "Porgera"}),
    "Africa & Middle East": AFRICA,
}
REGIONS = set(MAP_TO_2025Q3) | set(MAP_FROM_2025Q4)
E818_SUBJECT = "Loulo-Gounkoto"
E818_PERIODS = {"2025Q1", "2025Q2", "2025Q3", "2025 H1", "2025 9M"}
FIELD_ID = {("au", "prod_100"): "au_prod_100_koz", ("au", "prod_attr"): "au_prod_attr_koz",
            ("au", "sold_attr"): "au_sold_attr_koz", ("cu", "prod_100"): "cu_prod_100_kt",
            ("cu", "prod_attr"): "cu_prod_attr_kt", ("cu", "sold_attr"): "cu_sold_attr_kt"}
FLOWS = ("prod_100", "prod_attr", "sold_attr")
GROUPS = ("current", "prior")
CUM = {2: "H1", 3: "9M", 4: "FY"}
PREV = {2: (1, "QTD"), 3: (2, "YTD"), 4: (3, "YTD")}  # anchor quarter -> (release quarter, sheet) of the previous cumulative
PREV_KIND = {2: "Q1", 3: "H1", 4: "9M"}
WORDS = {"three": "Q", "six": "H1", "nine": "9M", "twelve": "FY"}
HEADER_FOOT = re.compile(r"(?<=[A-Za-z)])[0-9]{1,2}(?=[\s(]|$)")
SHARE_RE = re.compile(r"\(\s*([0-9]+(?:\.[0-9]+)?)\s*%\s*\)")
FOOTNOTE_RE = re.compile(r"[0-9]+\s")


def die(msg):
    raise SystemExit("mine_stats_census: " + msg)


def letters(col):
    s = ""
    while col:
        col, r = divmod(col - 1, 26)
        s = chr(65 + r) + s
    return s


def region_map(year, q):
    return MAP_TO_2025Q3 if (year, q) <= (2025, 3) else MAP_FROM_2025Q4


def period(year, kind):
    """J1: kind is Q1..Q4, H1, 9M or FY."""
    return "%d%s" % (year, kind) if kind.startswith("Q") else "%d %s" % (year, kind)


def fstr(x):
    if x is None:
        return "unbounded"
    return str(x.numerator) if x.denominator == 1 else "%d/%d" % (x.numerator, x.denominator)


# ---------------------------------------------------------------------------------------------------- reading

def shared_strings(z):
    if "xl/sharedStrings.xml" not in z.namelist():
        return []
    out = []
    for si in ET.fromstring(z.read("xl/sharedStrings.xml")).findall(M + "si"):
        parts = []
        for ch in si:
            if ch.tag == M + "t":
                parts.append(ch.text or "")
            elif ch.tag == M + "r":
                parts.extend(t.text or "" for t in ch.iter(M + "t"))
        out.append("".join(parts))
    return out


class Sheet:
    def __init__(self, release, name, root, ss):
        self.release, self.name = release, name
        self.cells = {}  # (row, col) -> (t, text or None)
        for c in root.iter(M + "c"):
            rc = formats.row_col(c.get("r"))
            if rc is None:
                die("%s:%s: cell without a usable reference" % (release.key, name))
            t = c.get("t", "")
            if t == "s":
                v = c.find(M + "v")
                text = ss[int(v.text)] if v is not None and (v.text or "").strip() else None
            elif t == "inlineStr":
                text = "".join(x.text or "" for x in c.iter(M + "t"))
            else:
                v = c.find(M + "v")
                text = v.text if v is not None else None
            self.cells[rc] = (t, text)
        self.merges = []
        for mc in root.iter(M + "mergeCell"):
            a, _, b = mc.get("ref").partition(":")
            ra, rb = formats.row_col(a), formats.row_col(b or a)
            if ra is None or rb is None:
                die("%s:%s: unreadable merged range" % (release.key, name))
            self.merges.append((ra[0], ra[1], rb[0], rb[1]))
        self.rows = {}
        for (r, c) in self.cells:
            self.rows.setdefault(r, []).append(c)
        self.blocks = {}

    def loc(self, row, col):
        return "%s:%s!%s%d" % (self.release.key, self.name, letters(col), row)

    def text(self, row, col):
        """The cell's text if it is a text cell, else None."""
        v = self.cells.get((row, col))
        if v is None or v[0] not in TEXT_TYPES:
            return None
        return v[1] if v[1] is not None else ""

    def is_empty(self, row, col):
        v = self.cells.get((row, col))
        return v is None or v[1] is None or (v[0] in TEXT_TYPES and v[1] == "")


class Op:
    __slots__ = ("loc", "kind", "value", "half", "text")

    def __init__(self, loc, kind, value=None, half=None, text=None):
        self.loc, self.kind, self.value, self.half, self.text = loc, kind, value, half, text


def read_operand(sheet, row, col):
    """Op with kind num, ph or blank (rule 'Values'); anything else stops the run (J7)."""
    loc = sheet.loc(row, col)
    rel = sheet.release
    cell = sheet.cells.get((row, col))
    key = (sheet.name, "%s%d" % (letters(col), row))
    if cell is not None and cell[0] in ("e", "b"):
        die("%s: error or boolean cell in a flow column" % loc)
    if key in rel.shown:
        kind, text = rel.shown[key]
        if kind == "shown":
            if text.endswith("%"):
                die("%s: percentage displayed in a flow column" % loc)
            m = re.fullmatch(r"-?[0-9]+(?:\.([0-9]+))?", text)
            if not m:
                die("%s: displayed text is not a number" % loc)
            return Op(loc, "num", F(text), F(1, 2 * 10 ** len(m.group(1) or "")), text)
        if kind == "token":
            return Op(loc, "ph", text=text)
        die("%s: formats.py refuses this cell's number format" % loc)
    if cell is None:
        return Op(loc, "blank")
    t, text = cell
    if t in TEXT_TYPES:
        if text is None or text == "":
            return Op(loc, "blank")
        if text.strip() in PLACE:
            return Op(loc, "ph", text=text)
        die("%s: text that is not a placeholder in a flow column" % loc)
    if text is None or text.strip() == "":
        return Op(loc, "blank")
    die("%s: stored value formats.py does not read as a number" % loc)


def header_kind(text):
    """('flow', metal, flow) | ('cost', metal) | None (J2)."""
    t = re.sub(r"\s+", " ", HEADER_FOOT.sub("", text)).strip().lower()
    if "$/oz" in t or "$/lb" in t:
        if any(k in t for k in ("cost of sales", "cash costs", "sustaining costs")):
            return ("cost", "au" if "$/oz" in t else "cu")
        return None
    gold, copper = bool(re.search(r"\bgold\b", t)), bool(re.search(r"\bcopper\b", t))
    if gold == copper:
        return None
    metal = "au" if gold else "cu"
    if metal == "au" and "000s ozs" not in t:
        return None
    if metal == "cu" and not re.search(r"\bkt\b", t):
        return None
    produced, sold = bool(re.search(r"\bproduced\b", t)), bool(re.search(r"\bsold\b", t))
    b100, attr = "100% basis" in t, "attributable" in t
    if produced and not sold and b100 and not attr:
        return ("flow", metal, "prod_100")
    if produced and not sold and attr and not b100:
        return ("flow", metal, "prod_attr")
    if sold and not produced and attr and not b100:
        return ("flow", metal, "sold_attr")
    return None


def subject_of(label):
    """(subject, printed share text or None) (J6)."""
    s = re.sub(r"\s+", " ", label).strip()
    m = re.match(r"^(.*?[^\d\s])(\d{1,2})$", s)
    if m:
        s = m.group(1)
    shares = SHARE_RE.findall(s)
    if len(shares) > 1:
        return None, None
    s = re.sub(r"\s+", " ", SHARE_RE.sub("", s)).strip()
    return s, (shares[0] if shares else None)


class Block:
    def __init__(self, metal, year_row, header_row, cols):
        self.metal, self.year_row, self.header_row, self.cols = metal, year_row, header_row, cols
        self.rows = {}  # subject -> (row, label, share text)


def year_of(sheet, yr, col):
    """J4: the year over a field column."""
    anchor = None
    for r1, c1, r2, c2 in sheet.merges:
        if r1 <= yr <= r2 and c1 <= col <= c2:
            anchor = (r1, c1)
            break
    if anchor is None:
        for c in range(col, 1, -1):
            if not sheet.is_empty(yr, c):
                anchor = (yr, c)
                break
    if anchor is None:
        die("%s: no year above this column" % sheet.loc(yr, col))
    t, text = sheet.cells.get(anchor, (None, None))
    if t in TEXT_TYPES:
        text = (text or "").strip()
    else:
        shown = sheet.release.shown.get((sheet.name, "%s%d" % (letters(anchor[1]), anchor[0])))
        text = shown[1] if shown and shown[0] == "shown" else None
    if text is None or not re.fullmatch(r"[0-9]{4}", text):
        die("%s: year header cell is not a year" % sheet.loc(*anchor))
    year = int(text)
    if year == sheet.release.year:
        return "current"
    if year == sheet.release.year - 1:
        return "prior"
    die("%s: year header is neither the release's year nor the year before" % sheet.loc(*anchor))


def layout(sheet):
    rel = sheet.release
    headers = []  # (row, metal, {col: kind})
    for r in sorted(sheet.rows):
        kinds = {}
        for c in sorted(sheet.rows[r]):
            if c == 1:
                continue
            txt = sheet.text(r, c)
            if txt is not None and txt.strip():
                kinds[c] = header_kind(txt)
        if not any(k is not None and k[0] == "flow" for k in kinds.values()):
            continue
        metals = set()
        for c, k in kinds.items():
            if k is None:
                die("%s: header cannot be mapped to a field" % sheet.loc(r, c))
            metals.add(k[1])
        for c in sheet.rows[r]:
            if c != 1 and c not in kinds and not sheet.is_empty(r, c):
                die("%s: non-text cell in a column-header row" % sheet.loc(r, c))
        if len(metals) != 1:
            die("%s: column-header row mixes gold and copper" % sheet.loc(r, 1))
        headers.append((r, metals.pop(), kinds))
    if sorted(m for _, m, _ in headers) != ["au", "cu"]:
        die("%s:%s: expected exactly one gold and one copper block" % (rel.key, sheet.name))
    want = "Q" if sheet.name == "QTD" else CUM.get(rel.q)
    if want is None:
        die("%s:%s: YTD sheet in a Q1 release" % (rel.key, sheet.name))
    year_rows = []
    prev_header = 0
    for hr, metal, kinds in headers:
        yr = None
        for r in range(hr - 1, prev_header, -1):
            a = sheet.text(r, 1)
            if a is not None and re.sub(r"\s+", " ", a).strip().lower().startswith("for the"):
                yr = r
                break
        if yr is None:
            die("%s: no 'For the ...' year header row above this column-header row" % sheet.loc(hr, 1))
        a = re.sub(r"\s+", " ", sheet.text(yr, 1)).lower()
        found = [w for w in WORDS if re.search(r"\b%s\b" % w, a)]
        if len(found) != 1 or WORDS[found[0]] != want:
            die("%s: period words do not match the sheet and release" % sheet.loc(yr, 1))
        cols = {}
        for c, k in kinds.items():
            if k[0] != "flow":
                continue
            g = year_of(sheet, yr, c)
            if (g, k[2]) in cols:
                die("%s: second column for one group and field" % sheet.loc(hr, c))
            cols[(g, k[2])] = c
        if len(cols) != 6:
            die("%s: a group lacks a flow field column" % sheet.loc(hr, 1))
        sheet.blocks[metal] = Block(metal, yr, hr, cols)
        year_rows.append(yr)
        prev_header = hr
    max_row = max(sheet.rows)
    for blk in sheet.blocks.values():
        ends = [yr for yr in year_rows if yr > blk.header_row]
        for r in range(blk.header_row + 1, max_row + 1):
            a = sheet.text(r, 1)
            if a is not None and FOOTNOTE_RE.match(a.lstrip()):
                ends.append(r)
                break
        if not ends:
            die("%s: block has no end (no next block, no footnote row)" % sheet.loc(blk.header_row, 1))
        end = min(ends)
        for r in range(blk.header_row + 1, end):
            if not sheet.is_empty(r, 1) and sheet.text(r, 1) is None:
                die("%s: numeric cell where a subject label is expected" % sheet.loc(r, 1))
            a = sheet.text(r, 1)
            if a is None or not a.strip():
                for c in blk.cols.values():
                    if read_operand(sheet, r, c).kind != "blank":
                        die("%s: value in a row with no subject label" % sheet.loc(r, c))
                continue
            if FOOTNOTE_RE.match(a.lstrip()):
                continue
            if re.sub(r"\s+", " ", a).strip().lower().startswith("for the"):
                die("%s: header row inside a block" % sheet.loc(r, 1))
            subj, share = subject_of(a)
            if subj is None:
                die("%s: two shares in one label" % sheet.loc(r, 1))
            if not subj:
                die("%s: empty subject" % sheet.loc(r, 1))
            if subj in blk.rows:
                die("%s: subject repeated in its block" % sheet.loc(r, 1))
            blk.rows[subj] = (r, a, share)
            for c in blk.cols.values():
                read_operand(sheet, r, c)  # J7: classify every flow cell now


class Release:
    def __init__(self, path, year, q):
        self.path, self.year, self.q = path, year, q
        self.key = "%dQ%d" % (year, q)
        self.shown = {}
        formats.read_displayed(path, False, into=self.shown)
        z = zipfile.ZipFile(path)
        names = set(z.namelist())
        ss = shared_strings(z)
        wb = ET.fromstring(z.read("xl/workbook.xml"))
        rels = ET.fromstring(z.read("xl/_rels/workbook.xml.rels")).findall(P + "Relationship")
        target = {r.get("Id"): r.get("Target") for r in rels}
        self.sheets = {}
        for sh in wb.find(M + "sheets").findall(M + "sheet"):
            name = sh.get("name")
            if name not in ("QTD", "YTD"):
                continue
            if name in self.sheets:
                die("%s: two sheets named %s" % (self.key, name))
            if sh.get("state", "visible") in ("hidden", "veryHidden"):
                die("%s:%s: hidden summary sheet" % (self.key, name))
            key = formats.part("xl", target[sh.get(R + "id")])
            if key not in names:
                die("%s:%s: sheet part missing" % (self.key, name))
            self.sheets[name] = Sheet(self, name, ET.fromstring(z.read(key)), ss)
        if "QTD" not in self.sheets:
            die("%s: no QTD sheet" % self.key)
        if q > 1 and "YTD" not in self.sheets:
            die("%s: no YTD sheet" % self.key)
        for sheet in self.sheets.values():
            layout(sheet)


def collect(args):
    paths = []
    for a in args:
        if os.path.isdir(a):
            for n in sorted(os.listdir(a)):
                if FILE_RE.fullmatch(n) and os.path.isfile(os.path.join(a, n)):
                    paths.append(os.path.join(a, n))
        else:
            if not FILE_RE.fullmatch(os.path.basename(a)):
                die("not a Mine Statistics workbook name: %s" % os.path.basename(a))
            paths.append(a)
    releases = {}
    for p in paths:
        m = FILE_RE.fullmatch(os.path.basename(p))
        q, y = int(m.group(1)), int(m.group(2))
        if (y, q) in releases:
            die("two files for release %dQ%d" % (y, q))
        releases[(y, q)] = Release(p, y, q)
    if not releases:
        die("no Mine Statistics workbooks given")
    return releases


# ---------------------------------------------------------------------------------------------------- instances

def decide(inst, ops):
    """Steps 2-5 for an instance whose operands all exist."""
    for o in ops:
        if o.kind == "blank":
            return "v", "blank_operand", o.loc
    kinds = [o.kind for o in ops]
    if all(k == "ph" for k in kinds):
        return "np", None, None
    if any(k == "ph" for k in kinds):
        return "v", "placeholder_operand", next(o.loc for o in ops if o.kind == "ph")
    if inst["family"] == "ownership":
        s = inst["_s"]
        if s is None:
            return "v", "share_unprinted", None
        b, c = ops
        ok = s * (b.value - b.half) <= c.value + c.half and c.value - c.half <= s * (b.value + b.half)
    else:
        a, q, t = ops
        ok = abs(a.value + q.value - t.value) <= a.half + q.half + t.half
    return ("i" if ok else "iii"), None, None


def base(family, subject, metal, flow, group, per, anchor, ops):
    return {"family": family, "subject": subject, "field": FIELD_ID[(metal, flow)], "group": group,
            "period": per, "anchor": anchor, "cells": [o.loc if o is not None else None for o in ops],
            "state": None, "reason": None, "at": None, "share": None, "printed_share": None,
            "component": None, "corrections": None, "_ops": ops, "_s": None}


def ownership_instances(releases):
    out = []
    for (y, q), rel in sorted(releases.items()):
        for sname in ("QTD", "YTD"):
            sheet = rel.sheets.get(sname)
            if sheet is None:
                continue
            kind = "Q%d" % q if sname == "QTD" else CUM[q]
            for metal in ("au", "cu"):
                blk = sheet.blocks[metal]
                for subj, (r, _label, share) in blk.rows.items():
                    if subj in AGGREGATES:
                        continue
                    for g in GROUPS:
                        per = period(y if g == "current" else y - 1, kind)
                        ops = [read_operand(sheet, r, blk.cols[(g, "prod_100")]),
                               read_operand(sheet, r, blk.cols[(g, "prod_attr")])]
                        inst = base("ownership", subj, metal, "prod_attr", g, per, ops[1].loc, ops)
                        if share is not None:
                            used = "24.5" if subj == "Porgera" and F(share) == F("47.5") else share
                            inst["share"], inst["printed_share"], inst["_s"] = used, share, F(used) / 100
                        if subj == E818_SUBJECT and per in E818_PERIODS:
                            inst["state"], inst["reason"] = "v", "excluded_e818"
                        else:
                            inst["state"], inst["reason"], inst["at"] = decide(inst, ops)
                        out.append(inst)
    return out


def quarter_sum_instances(releases):
    out = []
    for (y, q), rel in sorted(releases.items()):
        if q == 1:
            continue
        pq, psheet = PREV[q]
        if (y, pq) not in releases:
            die("operand release %dQ%d is not in the input set (needed by %s:YTD)" % (y, pq, rel.key))
        prev = releases[(y, pq)]
        ytd = rel.sheets["YTD"]
        sources = [(prev, psheet, PREV_KIND[q]), (rel, "QTD", "Q%d" % q), (rel, "YTD", CUM[q])]
        for metal in ("au", "cu"):
            for subj, (r, _label, _share) in ytd.blocks[metal].rows.items():
                for flow in FLOWS:
                    for g in GROUPS:
                        yy = y if g == "current" else y - 1
                        ops, pers = [], []
                        for srel, sname, kind in sources:
                            sh = srel.sheets[sname]
                            blk = sh.blocks[metal]
                            pers.append(period(yy, kind))
                            hit = blk.rows.get(subj)
                            ops.append(read_operand(sh, hit[0], blk.cols[(g, flow)]) if hit else None)
                        inst = base("quarter_sum", subj, metal, flow, g, pers[2], ops[2].loc, ops)
                        maps = {(srel.year, srel.q) for srel, _, _ in sources}
                        members = {region_map(*k).get(subj, frozenset()) for k in maps}
                        if subj == E818_SUBJECT and any(p in E818_PERIODS for p in pers):
                            inst["state"], inst["reason"] = "v", "excluded_e818"
                        elif subj in REGIONS and len(members) > 1:
                            inst["state"], inst["reason"] = "v", "cross_map_region"
                        elif any(o is None for o in ops):
                            inst["state"], inst["reason"] = "v", "operand_row_missing"
                        else:
                            inst["state"], inst["reason"], inst["at"] = decide(inst, ops)
                        out.append(inst)
    return out


# ---------------------------------------------------------------------------------------------------- exact joint decision

def _norm(cons):
    """Inequalities a.z <= b, deduplicated by normalised direction (tightest b kept); None if infeasible."""
    out = {}
    for a, b in cons:
        piv = next((x for x in a if x != 0), None)
        if piv is None:
            if b < 0:
                return None
            continue
        sc = abs(piv)
        if sc != 1:
            a, b = tuple(x / sc for x in a), b / sc
        old = out.get(a)
        if old is None or b < old:
            out[a] = b
    return out


def _eliminate(cons, k):
    pos, neg, rest = [], [], []
    for a, b in cons.items():
        (pos if a[k] > 0 else neg if a[k] < 0 else rest).append((a, b))
    new = list(rest)
    for ap, bp in pos:
        for an, bn in neg:
            lam, mu = -an[k], ap[k]
            new.append((tuple(lam * x + mu * y for x, y in zip(ap, an)), lam * bp + mu * bn))
    return _norm(new)


class System:
    """A component: cells (sorted locators), boxes, homogeneous equalities, reduced to free variables."""

    def __init__(self, cells, boxes, eqs):
        self.cells = cells
        self.index = {c: i for i, c in enumerate(cells)}
        self.boxes = [boxes[c] for c in cells]
        n = len(cells)
        rows = []
        for eq in eqs:
            row = [F(0)] * n
            for c, coef in eq.items():
                row[self.index[c]] += coef
            rows.append(row)
        pivots, r = [], 0
        for c in range(n):
            i = next((i for i in range(r, len(rows)) if rows[i][c] != 0), None)
            if i is None:
                continue
            rows[r], rows[i] = rows[i], rows[r]
            pv = rows[r][c]
            rows[r] = [x / pv for x in rows[r]]
            for j in range(len(rows)):
                if j != r and rows[j][c] != 0:
                    f = rows[j][c]
                    rows[j] = [x - f * y for x, y in zip(rows[j], rows[r])]
            pivots.append(c)
            r += 1
        free = [c for c in range(n) if c not in pivots]
        self.p = len(free)
        self.expr = [None] * n
        for i, c in enumerate(free):
            self.expr[c] = tuple(F(1) if j == i else F(0) for j in range(self.p))
        for i, c in enumerate(pivots):
            self.expr[c] = tuple(-rows[i][f] for f in free)

    def _ineqs(self, dropped):
        out = []
        for j, (lo, hi) in enumerate(self.boxes):
            if self.cells[j] in dropped:
                continue
            e = self.expr[j]
            out.append((e, hi))
            out.append((tuple(-x for x in e), -lo))
        return out

    def feasible(self, dropped=frozenset()):
        cons = _norm(self._ineqs(dropped))
        for k in range(self.p):
            if cons is None:
                return False
            cons = _eliminate(cons, k)
        return cons is not None

    def implied(self, dropped, cell):
        """Exact (lo, hi) of the cell's true value with `dropped` boxes removed; None side = unbounded."""
        e = self.expr[self.index[cell]]
        f = next((i for i, x in enumerate(e) if x != 0), None)
        if f is None:
            return F(0), F(0)
        sub = []
        for a, b in self._ineqs(dropped):
            af = a[f]
            sub.append((tuple(af / e[f] if g == f else a[g] - af * e[g] / e[f] for g in range(self.p)), b))
        cons = _norm(sub)
        for k in range(self.p):
            if cons is None:
                die("implied range asked of an infeasible set at %s" % cell)
            if k != f:
                cons = _eliminate(cons, k)
        if cons is None:
            die("implied range asked of an infeasible set at %s" % cell)
        lo = hi = None
        for a, b in cons.items():
            if a[f] > 0:
                hi = b if hi is None else min(hi, b)
            else:
                lo = -b if lo is None else max(lo, -b)
        return lo, hi


def components(instances):
    posed = [x for x in instances if x["state"] in ("i", "iii")]
    parent = {}

    def find(a):
        while parent[a] != a:
            parent[a] = parent[parent[a]]
            a = parent[a]
        return a

    for x in posed:
        for c in x["cells"]:
            parent.setdefault(c, c)
        for c in x["cells"][1:]:
            ra, rb = find(x["cells"][0]), find(c)
            if ra != rb:
                parent[max(ra, rb)] = min(ra, rb)
    groups = {}
    for x in posed:
        groups.setdefault(find(x["cells"][0]), []).append(x)
    comps = []
    for members in groups.values():
        boxes, eqs = {}, []
        for x in members:
            for o in x["_ops"]:
                boxes[o.loc] = (o.value - o.half, o.value + o.half)
            if x["family"] == "ownership":
                b, c = x["cells"]
                eqs.append({c: F(1), b: -x["_s"]})
            else:
                a, q, t = x["cells"]
                eqs.append({t: F(1), a: F(-1), q: F(-1)})
        cells = sorted(boxes)
        comps.append((cells, members, System(cells, boxes, eqs)))
    comps.sort(key=lambda t: t[0][0])
    return comps


def analyse(instances, with_values):
    comps = components(instances)
    lines = []
    for cells, members, sysm in comps:
        if sysm.feasible():
            continue
        cid = cells[0]
        sets = []
        for k in range(1, len(cells) + 1):
            sets = [list(s) for s in itertools.combinations(cells, k) if sysm.feasible(frozenset(s))]
            if sets:
                break
        sets.sort()
        implied = None
        if with_values:
            implied = []
            for s in sets:
                d = {}
                for c in s:
                    lo, hi = sysm.implied(frozenset(s), c)
                    d[c] = [fstr(lo), fstr(hi)]
                implied.append(d)
        members.sort(key=sort_key)
        joint_only = all(x["state"] == "i" for x in members)
        for x in members:
            x["component"], x["corrections"] = cid, sets
            if with_values:
                x["implied"] = implied
        line = {"component": cid, "cells": cells, "instances": [x["anchor"] for x in members],
                "joint_only": joint_only, "corrections": sets}
        if with_values:
            line["implied"] = implied
        lines.append(line)
    return comps, lines


def sort_key(x):
    return (x["family"], x["anchor"], x["group"], x["field"])


# ---------------------------------------------------------------------------------------------------- checks

def find(instances, **kw):
    return [x for x in instances if all(x.get(k) == v for k, v in kw.items())]


def check_pair(name, own, qs, expect_sets=None, value_cell=None, value=None, with_values=False):
    parts, ok = [], True
    for label, hits in (("quarter_sum", qs), ("ownership", own)):
        if len(hits) != 1:
            parts.append("%s: %d matching instances" % (label, len(hits)))
            ok = False
            continue
        x = hits[0]
        parts.append("%s %s %s state %s" % (label, x["anchor"], x["subject"], x["state"]))
        ok = ok and x["state"] == "iii"
    comps = sorted({h[0]["component"] for h in (qs, own) if len(h) == 1 and h[0]["component"]} )
    sets = None
    if len(qs) == 1 and len(own) == 1:
        if qs[0]["component"] is None or qs[0]["component"] != own[0]["component"]:
            parts.append("not in one infeasible component (%s)" % ", ".join(comps) if comps else
                         "no infeasible component")
            ok = False
        sets = qs[0]["corrections"] or own[0]["corrections"]
        parts.append("component %s minimum correction sets %s" % (", ".join(comps) or "none", json.dumps(sets)))
    if expect_sets is not None:
        same = sets == expect_sets
        parts.append("sets %s the expected %s" % ("equal" if same else "differ from", json.dumps(expect_sets)))
        ok = ok and same
    if value_cell is not None:
        if not with_values:
            parts.append("value part not evaluated without --values")
        elif sets is None or [value_cell] not in sets:
            parts.append("no correction set [%s]; value part fails" % value_cell)
            ok = False
        else:
            x = qs[0]
            imp = x["implied"][sets.index([value_cell])][value_cell]
            lo = None if imp[0] == "unbounded" else F(imp[0])
            hi = None if imp[1] == "unbounded" else F(imp[1])
            inside = (lo is None or lo <= value) and (hi is None or value <= hi)
            parts.append("%s implied [%s, %s]; %s %s it" % (value_cell, imp[0], imp[1], fstr(value),
                                                            "lies in" if inside else "is outside"))
            ok = ok and inside
    return {"check": name, "ok": ok, "detail": "; ".join(parts)}


def checks(instances, with_values):
    out = []
    own = find(instances, family="ownership", anchor="2026Q1:QTD!C5", group="current", subject="Carlin")
    qs = find(instances, family="quarter_sum", anchor="2026Q2:YTD!B5", group="current", subject="Carlin",
              field="au_prod_100_koz")
    out.append(check_pair("E113", own, qs, expect_sets=[["2026Q1:QTD!B5"]], value_cell="2026Q1:QTD!B5",
                          value=F(281), with_values=with_values))
    own = find(instances, family="ownership", anchor="2026Q2:YTD!C7", group="current", subject="Turquoise Ridge")
    qs = find(instances, family="quarter_sum", anchor="2026Q2:YTD!B7", group="current", subject="Turquoise Ridge",
              field="au_prod_100_koz")
    out.append(check_pair("E114", own, qs))
    own = [x for x in find(instances, family="ownership", subject="Jabal Sayid", group="current", period="2026Q2")
           if x["anchor"].startswith("2026Q2:QTD!")]
    qs = find(instances, family="quarter_sum", subject="Jabal Sayid", group="current", period="2026 H1",
              field="cu_prod_100_kt")
    rec = check_pair("E115", own, qs)
    rows = sorted({re.sub(r"^.*![A-Z]+", "", x["anchor"]) for x in own + qs})
    rec["detail"] += "; anchor rows %s" % ",".join(rows)
    if rows != ["28"]:
        rec["ok"] = False
    out.append(rec)
    return out


# ---------------------------------------------------------------------------------------------------- main

def emit(obj):
    print(json.dumps(obj, ensure_ascii=False, separators=(",", ":")))


def main(argv):
    with_values = "--values" in argv
    bad = [a for a in argv if a.startswith("--") and a != "--values"]
    args = [a for a in argv if not a.startswith("--")]
    if bad or not args:
        die("usage: mine_stats_census.py [--values] <dir-or-files...>")
    releases = collect(args)
    instances = ownership_instances(releases) + quarter_sum_instances(releases)
    instances.sort(key=sort_key)
    seen = set()
    for x in instances:
        k = sort_key(x)
        if k in seen:
            die("two instances with one key %r" % (k,))
        seen.add(k)
    comps, comp_lines = analyse(instances, with_values)
    keys = ["family", "subject", "field", "group", "period", "anchor", "cells", "state", "reason", "at", "share",
            "printed_share", "component", "corrections"]
    for x in instances:
        rec = {k: x[k] for k in keys}
        if with_values:
            rec["values"] = [o.text if o is not None and o.kind != "blank" else None for o in x["_ops"]]
            rec["implied"] = x.get("implied")
        emit(rec)
    for line in comp_lines:
        emit(line)
    for fam in ("ownership", "quarter_sum"):
        xs = [x for x in instances if x["family"] == fam]
        by = {}
        for x in xs:
            if x["state"] == "v":
                by[x["reason"]] = by.get(x["reason"], 0) + 1
        emit({"family": fam, "n": len(xs), **{s: sum(1 for x in xs if x["state"] == s)
                                               for s in ("i", "iii", "v", "np")},
              "v_by_reason": dict(sorted(by.items()))})
    emit({"components": len(comps), "infeasible_components": len(comp_lines),
          "joint_only": sum(1 for c in comp_lines if c["joint_only"])})
    for c in checks(instances, with_values):
        emit(c)


if __name__ == "__main__":
    main(sys.argv[1:])
