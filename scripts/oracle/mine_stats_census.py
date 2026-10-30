"""Mine Statistics census: ownership and quarter-sum identities on the QTD/YTD summary sheets, and the site_sheet
identity (summary value = Minestats value), each instance decided alone on its own displayed cells, then all decided
instances posed jointly by shared cells, with every minimum correction set of each jointly infeasible component.

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

Family site_sheet (the plan's family 7, "summary equals site sheet"; added 2026-10-30 by a second reader, from the
rule as given, without reading crates/ or any Rust). Mine Stats print each site's flows and unit costs twice: on
QTD/YTD (B-G the release's own year, I-N the prior year) and in the site's column on a Minestats* sheet (two columns
per site, current year then prior year; years on the block header row). Identity: the summary value equals the
Minestats value of the same release, site, period and column-year group.
- Minestats reading. A Minestats* sheet is gold if its A1 title contains "(Gold)", copper if "(Copper)"; otherwise a
  refusal line is printed and the sheet skipped. Blocks start at a column-A cell beginning "For the" and naming
  three, six, nine or twelve months; the site header row is the row above; a site name is carried right across
  blank (merged) header cells; a column's year is the integer on the block header row. Data rows run down to the
  next block header or footnote. Row labels: column A, whitespace collapsed and trimmed, trailing footnote digits
  stripped (^(.*?[^\\d\\s])(\\d{1,2})$), matched case-insensitively. Site names: trailing footnote number and any
  (x%) share stripped; "Copper - Total" is the subject "Total Copper". Summary subjects as read above. Period of a
  block: three months = the release's quarter, six = H1, nine = 9M, twelve = FY, with the column's year (J1).
- Field map (summary field id -> Minestats row label, on a sheet of that metal). Gold: au_prod_attr_koz "Production
  (thousands of ounces)", au_sold_attr_koz "Sales (thousands of ounces)", cos_usd_per_oz "Cost of sales per ounce
  ($/oz)", tcc_usd_per_oz "Total cash costs", aisc_usd_per_oz "All-in sustaining costs". Copper: cu_prod_attr_kt
  "Production (thousands of tonnes)", cu_sold_attr_kt "Sales (thousands of tonnes)", cos_usd_per_lb "Cost of sales
  per pound ($/lb)", c1_usd_per_lb "C1 cash costs", aisc_usd_per_lb "All-in sustaining costs". W1-DR-02: in releases
  2024Q1 to 2024Q4 only, the copper rows "Production (millions of pounds)" and "Sales (millions of pounds)" are read
  as the thousands-of-tonnes rows (values unchanged, no conversion); no other unit substitution is ever made.
  Summary cost headers map as the flow headers do: "Cost of sales ($/oz)" or "Total Cost of sales ($/oz)" ->
  cos_usd_per_oz, "Total cash costs ($/oz)" -> tcc_usd_per_oz, "All-In sustaining costs ($/oz)" -> aisc_usd_per_oz;
  the $/lb headers -> cos_usd_per_lb, c1_usd_per_lb ("C1 cash costs"), aisc_usd_per_lb. A header naming none of
  these is not read.
- Instances: one per summary value of a mapped field (both sheets, both column groups) whose subject has a column on
  some Minestats* sheet of the field's metal in the same release. Subjects with no such column pose none (regions,
  "Total Gold"); one line per release lists the summary subjects (not a region, not "Total Gold") with no Minestats
  column. A subject with columns on two sheets of one metal in one release: every instance for it is v,
  operand_unreadable.
- Decision, first match wins: 1. withheld (Loulo-Gounkoto, period in the E818 list) -> v, excluded_e818. 2. The
  block for the period and group has no row with the counterpart label, or the subject's column has no block for the
  period -> v, operand_row_missing; the label twice in one block -> v, operand_unreadable at the first. 3. Either
  cell blank -> v, blank_operand, at = the blank cell. 4. Both cells tokens ("—", "— ", "n/a", any combination) ->
  np. 5. One cell the nil token "—", the other a number: the identity reads number = 0; iii if the number's closed
  box [v - step/2, v + step/2] excludes 0, else i; certificate "nil_token"; decided alone, NOT posed jointly. 6. One
  cell "n/a", the other a number -> v, placeholder_operand, at = the n/a cell. 7. A number with no exact displayed
  value (or shown as a percentage) -> v, operand_unreadable. 8. Both numbers: equal identity over the two closed
  boxes: i if they intersect, else iii; posed jointly with the ownership and quarter_sum instances.
- Record: family "site_sheet", field = the summary field id, anchor = the summary cell, cells = [summary cell,
  Minestats cell or null], share and printed_share null, certificate null except "nil_token" (step 5).

Components. Every instance in state i or iii (and certificate null) is posed jointly; instances sharing a cell are
linked; connected components are decided exactly: do true values exist, each in its closed box, satisfying every
identity of the component (c = s.b, t = a + q, summary = Minestats)? For each infeasible component: every minimum
correction set (smallest k such that
dropping the boxes of some k cells makes it feasible; exhaustive by size; sets sorted), and with --values, for each
set and cell, the implied interval (exact range of the cell's true value with that set's boxes dropped). joint_only
if every instance of the component is i. Every iii instance carries its component id and corrections.

Output (stdout, one compact JSON object per line, ensure_ascii=False, deterministic order):
1. one line per instance sorted by (family, anchor, group, field): {"family","subject","field","group","period",
   "anchor","cells","state","reason","at","certificate","share","printed_share","component","corrections"};
   ownership's field is the attributable field and its anchor the attributable cell; with --values also "values"
   (displayed operand texts) and "implied" ([{locator: [lo, hi]}] per correction set);
2. one line per jointly infeasible component {"component","cells","instances","joint_only","corrections"};
3. per family {"family","n","i","iii","v","np","v_by_reason"} (ownership, quarter_sum, site_sheet); then one line
   per refused Minestats* sheet {"family":"site_sheet","refused","reason"}; then one line per release
   {"family":"site_sheet","release","no_minestats_column"}; then {"components","infeasible_components","joint_only"};
4. known-answer checks {"check","ok","detail"}: E113, E114, E115, E116, computed from the instances.
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
J13-J22 belong to family site_sheet (second reader, 2026-10-30).
J13 Minestats* sheets: the workbook's sheets whose name begins "Minestats", in workbook order. A hidden one gets a
   refusal line and is skipped (formats.py shows no values on hidden sheets), as does one whose A1 (a text cell)
   names neither or both of "(Gold)" and "(Copper)" (substrings, case-sensitive). No sheet of this corpus is refused.
J14 Minestats blocks: a block header is column-A text that, whitespace collapsed, begins "for the" in any case (as
   J3); it must name exactly one of three/six/nine/twelve as a word, else SystemExit. Two blocks of one period on one
   sheet, or a block header in row 1, SystemExit. A block's columns are the non-empty cells of its header row from
   column B; each must be a 4-digit year (text, or a number displayed as one) equal to the release's year (current)
   or the year before (prior), else SystemExit. A column's site is the nearest non-empty cell at or left of it (from
   column B) on the row above, which must be text; merges are not consulted. A site with two columns for one year in
   one block: SystemExit. A block ends at the next block header on the sheet or at the first footnote row below its
   header (J5's footnote test), whichever comes first; with neither, SystemExit. Every row in between with column-A
   text contributes its label.
J15 Subjects: a Minestats site name goes through J6 (whitespace, footnote, shares), then "Copper - Total" becomes
   "Total Copper"; it is compared with summary subjects by exact string equality (no alias table: "Nevada Gold Mines
   LLC" is spelled so on both sides). Row labels are compared after str.lower().
J16 This family reads its operands with its own reader, which never stops the run. A number displayed per 'Values'
   is a number. A text cell whose raw text is exactly "—" or "— " is the nil token (step 4 lists both spellings;
   step 5 names "—"), exactly "n/a" is n/a; blank as J7. Everything else is unreadable (step 7; at = that cell, the
   summary cell first when both are): a displayed percentage, a formats.py token or refused format, any other text
   (including "–", "-" and "N/A", which the other two families accept as placeholders), error and boolean cells.
   "A number" in steps 5, 6 and 8 means a readable number. In this corpus the mapped cells hold only numbers, "—",
   "— " and two blanks, so no instance is unreadable and none is n/a.
J17 at is null for excluded_e818, operand_row_missing and the two-sheet operand_unreadable. cells[1] is null when no
   counterpart cell is located (no block, no column, no row, two sheets) and names the located cell otherwise, also
   for an excluded instance. A label occurring twice: at and cells[1] are its first (lowest-row) occurrence in the
   subject's column. In 2024 releases the thousands-of-tonnes label and its W1-DR-02 "millions of pounds" alias count
   together, so a block holding both rows holds the label twice.
J18 The two-sheet operand_unreadable is applied before excluded_e818 ("every instance for it"). No subject of this
   corpus has columns on two sheets of one metal, so the order changes nothing here.
J19 Summary unit-cost headers: whitespace collapsed, lower case, must read "<base> ($/oz)" or "<base> ($/lb)",
   optionally followed by 1-2 footnote digits; <base> must be "cost of sales" or "total cost of sales", "total cash
   costs" ($/oz), "c1 cash costs" ($/lb) or "all-in sustaining costs". 1-2 digits glued to the end of <base> are
   dropped only if <base> does not map as it stands (so "C1" keeps its digit; J2's HEADER_FOOT is not used here
   because it reads "C1 cash costs" as "c cash costs"). Group from J4. A second column for one (group, field):
   SystemExit. All 12 unit-cost headers of every summary sheet in this corpus map.
J20 Joint decision: posed instances are those in state i or iii with certificate null; a site_sheet instance adds the
   equality summary - Minestats = 0 over its two boxes. A nil_token instance has component and corrections null.
   Component ids still follow J9, so a component holding a Minestats cell is usually named by it (":Minestats1!"
   sorts before ":QTD!"). The rule poses the two prints as independent boxes, so an equal Minestats print pins its
   summary cell and a correction set that dropped only the summary cell now also needs the Minestats cell; whether
   the two prints are independent evidence is outside this census.
J21 Output: the site_sheet family summary follows quarter_sum's; then the refusal lines (release order, then sheet
   order), then the per-release "no_minestats_column" lines (subjects sorted), then the components line. Every
   instance line gains "certificate" after "at". With --values, a site_sheet line's "values" is [summary text,
   Minestats text]. The comparison with W1's T4 table is not printed.
J22 E116 is read from the finished records: exactly one site_sheet instance anchored at 2026Q2:YTD!E11, in state
   iii, certificate "nil_token", cells ["2026Q2:YTD!E11", "2026Q2:Minestats1!N103"].
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
# site_sheet (family 7): summary field id -> Minestats row label (lower case), per metal, in output field order
SITE_FIELDS = {
    "au": {"au_prod_attr_koz": "production (thousands of ounces)", "au_sold_attr_koz": "sales (thousands of ounces)",
           "cos_usd_per_oz": "cost of sales per ounce ($/oz)", "tcc_usd_per_oz": "total cash costs",
           "aisc_usd_per_oz": "all-in sustaining costs"},
    "cu": {"cu_prod_attr_kt": "production (thousands of tonnes)", "cu_sold_attr_kt": "sales (thousands of tonnes)",
           "cos_usd_per_lb": "cost of sales per pound ($/lb)", "c1_usd_per_lb": "c1 cash costs",
           "aisc_usd_per_lb": "all-in sustaining costs"},
}
DR02_YEAR = 2024  # W1-DR-02: releases 2024Q1-2024Q4 label the copper kt rows "millions of pounds"
DR02_LABELS = {"cu_prod_attr_kt": "production (millions of pounds)", "cu_sold_attr_kt": "sales (millions of pounds)"}
COST_FIELD = {("oz", "cost of sales"): "cos_usd_per_oz", ("oz", "total cost of sales"): "cos_usd_per_oz",
              ("oz", "total cash costs"): "tcc_usd_per_oz", ("oz", "all-in sustaining costs"): "aisc_usd_per_oz",
              ("lb", "cost of sales"): "cos_usd_per_lb", ("lb", "total cost of sales"): "cos_usd_per_lb",
              ("lb", "c1 cash costs"): "c1_usd_per_lb", ("lb", "all-in sustaining costs"): "aisc_usd_per_lb"}
SITE_NIL = ("—", "— ")  # the nil token, with and without its trailing space (J16)
SITE_NA = "n/a"
LABEL_FOOT = re.compile(r"^(.*?[^\d\s])(\d{1,2})$")


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


def cost_field(text):
    """site_sheet: the field id a summary unit-cost header names, or None (not read) (J19)."""
    t = re.sub(r"\s+", " ", text).strip().lower()
    m = re.fullmatch(r"(.*?) ?\(\$/(oz|lb)\)(?:[0-9]{1,2})?", t)
    if not m:
        return None
    base = m.group(1).strip()
    fid = COST_FIELD.get((m.group(2), base))
    if fid is None:
        fid = COST_FIELD.get((m.group(2), re.sub(r"(?<=[a-z])[0-9]{1,2}$", "", base)))
    return fid


class Block:
    def __init__(self, metal, year_row, header_row, cols):
        self.metal, self.year_row, self.header_row, self.cols = metal, year_row, header_row, cols
        self.rows = {}  # subject -> (row, label, share text)
        self.cost_cols = {}  # site_sheet: (group, cost field id) -> column


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
        blk = Block(metal, yr, hr, cols)
        for c, k in sorted(kinds.items()):
            if k[0] != "cost":
                continue
            fid = cost_field(sheet.text(hr, c))
            if fid is None:
                continue
            g = year_of(sheet, yr, c)
            if (g, fid) in blk.cost_cols:
                die("%s: second column for one group and unit-cost field" % sheet.loc(hr, c))
            blk.cost_cols[(g, fid)] = c
        sheet.blocks[metal] = blk
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


def ms_label(text):
    """A Minestats row label as matched: whitespace collapsed, trimmed, footnote digits stripped, lower case."""
    s = re.sub(r"\s+", " ", text).strip()
    m = LABEL_FOOT.match(s)
    return (m.group(1) if m else s).lower()


class MsBlock:
    def __init__(self, kind, header_row):
        self.kind, self.header_row = kind, header_row
        self.end = None
        self.cols = {}  # (subject, group) -> column
        self.labels = {}  # matched label -> [rows], ascending


def ms_metal(sheet):
    """'au', 'cu', or None when the A1 title names neither or both metals (refusal, J13)."""
    title = sheet.text(1, 1) or ""
    gold, copper = "(Gold)" in title, "(Copper)" in title
    if gold == copper:
        return None
    return "au" if gold else "cu"


def ms_year(sheet, row, col):
    """J14: the 4-digit year of a non-empty block-header cell, else None."""
    t, text = sheet.cells[(row, col)]
    if t in TEXT_TYPES:
        text = (text or "").strip()
    else:
        shown = sheet.release.shown.get((sheet.name, "%s%d" % (letters(col), row)))
        text = shown[1] if shown and shown[0] == "shown" else None
    return int(text) if text is not None and re.fullmatch(r"[0-9]{4}", text) else None


def ms_layout(sheet):
    """Blocks of a Minestats* sheet by period kind (J14)."""
    rel = sheet.release
    heads = []
    for r in sorted(sheet.rows):
        a = sheet.text(r, 1)
        if a is None or not re.sub(r"\s+", " ", a).strip().lower().startswith("for the"):
            continue
        low = re.sub(r"\s+", " ", a).lower()
        found = [w for w in WORDS if re.search(r"\b%s\b" % w, low)]
        if len(found) != 1:
            die("%s: block header names no single period (three, six, nine or twelve months)" % sheet.loc(r, 1))
        kind = "Q%d" % rel.q if WORDS[found[0]] == "Q" else WORDS[found[0]]
        if r < 2:
            die("%s: block header with no site header row above it" % sheet.loc(r, 1))
        heads.append((r, kind))
    sheet.ms_blocks = {}
    max_row = max(sheet.rows)
    for i, (hr, kind) in enumerate(heads):
        if kind in sheet.ms_blocks:
            die("%s: second block for one period on this sheet" % sheet.loc(hr, 1))
        blk = MsBlock(kind, hr)
        for c in sorted(sheet.rows[hr]):
            if c == 1 or sheet.is_empty(hr, c):
                continue
            year = ms_year(sheet, hr, c)
            if year is None:
                die("%s: block header cell is not a year" % sheet.loc(hr, c))
            if year == rel.year:
                g = "current"
            elif year == rel.year - 1:
                g = "prior"
            else:
                die("%s: year is neither the release's year nor the year before" % sheet.loc(hr, c))
            name = None
            for cc in range(c, 1, -1):
                if not sheet.is_empty(hr - 1, cc):
                    name = sheet.text(hr - 1, cc)
                    if name is None:
                        die("%s: site header cell is not text" % sheet.loc(hr - 1, cc))
                    break
            if name is None:
                die("%s: no site name over this column" % sheet.loc(hr - 1, c))
            subj, _share = subject_of(name)
            if subj is None:
                die("%s: two shares in one site name" % sheet.loc(hr - 1, c))
            if subj == "Copper - Total":
                subj = "Total Copper"
            if not subj:
                die("%s: empty site name" % sheet.loc(hr - 1, c))
            if (subj, g) in blk.cols:
                die("%s: second column for one site and year in this block" % sheet.loc(hr, c))
            blk.cols[(subj, g)] = c
        ends = [h for h, _ in heads[i + 1:i + 2]]
        for r in range(hr + 1, max_row + 1):
            a = sheet.text(r, 1)
            if a is not None and FOOTNOTE_RE.match(a.lstrip()):
                ends.append(r)
                break
        if not ends:
            die("%s: block has no end (no next block, no footnote row)" % sheet.loc(hr, 1))
        blk.end = min(ends)
        for r in range(hr + 1, blk.end):
            a = sheet.text(r, 1)
            if a is not None and a.strip():
                blk.labels.setdefault(ms_label(a), []).append(r)
        sheet.ms_blocks[kind] = blk


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
        # site_sheet: the Minestats* sheets, in workbook order (J13)
        self.ms_sheets, self.refusals = [], []
        self.ms_cols = {"au": {}, "cu": {}}  # metal -> subject -> [sheet names with a column for it]
        for sh in wb.find(M + "sheets").findall(M + "sheet"):
            name = sh.get("name")
            if not name.startswith("Minestats"):
                continue
            where = "%s:%s!A1" % (self.key, name)
            if sh.get("state", "visible") in ("hidden", "veryHidden"):
                self.refusals.append((where, "hidden sheet"))
                continue
            key = formats.part("xl", target[sh.get(R + "id")])
            if key not in names:
                die("%s:%s: sheet part missing" % (self.key, name))
            sheet = Sheet(self, name, ET.fromstring(z.read(key)), ss)
            metal = ms_metal(sheet)
            if metal is None:
                self.refusals.append((where, "A1 title names neither (Gold) nor (Copper), or both"))
                continue
            ms_layout(sheet)
            sheet.metal = metal
            self.ms_sheets.append(sheet)
            for blk in sheet.ms_blocks.values():
                for subj, _g in blk.cols:
                    have = self.ms_cols[metal].setdefault(subj, [])
                    if name not in have:
                        have.append(name)


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
            "state": None, "reason": None, "at": None, "certificate": None, "share": None, "printed_share": None,
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


def read_site_operand(sheet, row, col):
    """site_sheet operand: Op kind num, nil, na, blank or unreadable; never stops the run (J16)."""
    loc = sheet.loc(row, col)
    cell = sheet.cells.get((row, col))
    if cell is not None and cell[0] in ("e", "b"):
        return Op(loc, "unreadable", text=cell[1])
    shown = sheet.release.shown.get((sheet.name, "%s%d" % (letters(col), row)))
    if shown is not None:
        kind, text = shown
        if kind == "shown":
            m = re.fullmatch(r"-?[0-9]+(?:\.([0-9]+))?", text)
            if m:
                return Op(loc, "num", F(text), F(1, 2 * 10 ** len(m.group(1) or "")), text)
        return Op(loc, "unreadable", text=text)  # a percentage, a format token or a refused format
    if cell is None:
        return Op(loc, "blank")
    t, text = cell
    if t in TEXT_TYPES:
        if text is None or text == "":
            return Op(loc, "blank")
        if text in SITE_NIL:
            return Op(loc, "nil", text=text)
        if text == SITE_NA:
            return Op(loc, "na", text=text)
        return Op(loc, "unreadable", text=text)
    if text is None or text.strip() == "":
        return Op(loc, "blank")
    return Op(loc, "unreadable", text=text)


def decide_site(ops):
    """Steps 3-8 for a site_sheet instance whose two operands exist: (state, reason, at, certificate)."""
    for o in ops:
        if o.kind == "blank":
            return "v", "blank_operand", o.loc, None
    kinds = [o.kind for o in ops]
    if all(k in ("nil", "na") for k in kinds):
        return "np", None, None, None
    if sorted(kinds) == ["nil", "num"]:
        n = next(o for o in ops if o.kind == "num")
        ok = n.value - n.half <= 0 <= n.value + n.half
        return ("i" if ok else "iii"), None, None, "nil_token"
    if sorted(kinds) == ["na", "num"]:
        return "v", "placeholder_operand", next(o.loc for o in ops if o.kind == "na"), None
    if any(k == "unreadable" for k in kinds):
        return "v", "operand_unreadable", next(o.loc for o in ops if o.kind == "unreadable"), None
    a, b = ops
    ok = a.value - a.half <= b.value + b.half and b.value - b.half <= a.value + a.half
    return ("i" if ok else "iii"), None, None, None


def site_counterpart(rel, metal, subj, kind, g, fid):
    """('two_sheets'|'missing', None) or ('dup'|'ok', Minestats op at the (first) counterpart cell) (J17)."""
    have = rel.ms_cols[metal][subj]
    if len(have) > 1:
        return "two_sheets", None
    sheet = next(s for s in rel.ms_sheets if s.name == have[0])
    blk = sheet.ms_blocks.get(kind)
    if blk is None or (subj, g) not in blk.cols:
        return "missing", None
    labels = [SITE_FIELDS[metal][fid]]
    if metal == "cu" and rel.year == DR02_YEAR and fid in DR02_LABELS:
        labels.append(DR02_LABELS[fid])
    rows = sorted(r for lab in labels for r in blk.labels.get(lab, []))
    if not rows:
        return "missing", None
    op = read_site_operand(sheet, rows[0], blk.cols[(subj, g)])
    return ("dup" if len(rows) > 1 else "ok"), op


def site_sheet_instances(releases):
    """Family site_sheet; also, per release, the summary subjects with no Minestats column."""
    out, unmatched = [], []
    for (y, q), rel in sorted(releases.items()):
        missing = set()
        for sname in ("QTD", "YTD"):
            sheet = rel.sheets.get(sname)
            if sheet is None:
                continue
            kind = "Q%d" % q if sname == "QTD" else CUM[q]
            for metal in ("au", "cu"):
                blk = sheet.blocks[metal]
                cols = dict(blk.cost_cols)
                for g in GROUPS:
                    for flow in ("prod_attr", "sold_attr"):
                        cols[(g, FIELD_ID[(metal, flow)])] = blk.cols[(g, flow)]
                for subj, (r, _label, _share) in blk.rows.items():
                    if subj not in rel.ms_cols[metal]:
                        if subj not in REGIONS and subj != "Total Gold":
                            missing.add(subj)
                        continue
                    for g in GROUPS:
                        per = period(y if g == "current" else y - 1, kind)
                        for fid in SITE_FIELDS[metal]:
                            if (g, fid) not in cols:
                                continue
                            sop = read_site_operand(sheet, r, cols[(g, fid)])
                            how, mop = site_counterpart(rel, metal, subj, kind, g, fid)
                            ops = [sop, mop]
                            inst = {"family": "site_sheet", "subject": subj, "field": fid, "group": g,
                                    "period": per, "anchor": sop.loc, "cells": [sop.loc, mop.loc if mop else None],
                                    "state": "v", "reason": None, "at": None, "certificate": None, "share": None,
                                    "printed_share": None, "component": None, "corrections": None, "_ops": ops,
                                    "_s": None}
                            if how == "two_sheets":
                                inst["reason"] = "operand_unreadable"
                            elif subj == E818_SUBJECT and per in E818_PERIODS:
                                inst["reason"] = "excluded_e818"
                            elif how == "missing":
                                inst["reason"] = "operand_row_missing"
                            elif how == "dup":
                                inst["reason"], inst["at"] = "operand_unreadable", mop.loc
                            else:
                                inst["state"], inst["reason"], inst["at"], inst["certificate"] = decide_site(ops)
                            out.append(inst)
        unmatched.append((rel.key, sorted(missing)))
    return out, unmatched


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
    posed = [x for x in instances if x["state"] in ("i", "iii") and x["certificate"] is None]
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
            elif x["family"] == "site_sheet":
                s, m = x["cells"]
                eqs.append({s: F(1), m: F(-1)})
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
    hits = find(instances, family="site_sheet", anchor="2026Q2:YTD!E11")
    want = ["2026Q2:YTD!E11", "2026Q2:Minestats1!N103"]
    if len(hits) != 1:
        out.append({"check": "E116", "ok": False, "detail": "site_sheet: %d instances anchored at 2026Q2:YTD!E11"
                    % len(hits)})
    else:
        x = hits[0]
        ok = x["state"] == "iii" and x["certificate"] == "nil_token" and x["cells"] == want
        out.append({"check": "E116", "ok": ok, "detail": "site_sheet %s %s %s %s state %s certificate %s cells %s; "
                    "expected state iii, certificate nil_token, cells %s"
                    % (x["anchor"], x["subject"], x["field"], x["period"], x["state"], x["certificate"],
                       json.dumps(x["cells"]), json.dumps(want))})
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
    site, unmatched = site_sheet_instances(releases)
    instances = ownership_instances(releases) + quarter_sum_instances(releases) + site
    instances.sort(key=sort_key)
    seen = set()
    for x in instances:
        k = sort_key(x)
        if k in seen:
            die("two instances with one key %r" % (k,))
        seen.add(k)
    comps, comp_lines = analyse(instances, with_values)
    keys = ["family", "subject", "field", "group", "period", "anchor", "cells", "state", "reason", "at", "certificate",
            "share", "printed_share", "component", "corrections"]
    for x in instances:
        rec = {k: x[k] for k in keys}
        if with_values:
            rec["values"] = [o.text if o is not None and o.kind != "blank" else None for o in x["_ops"]]
            rec["implied"] = x.get("implied")
        emit(rec)
    for line in comp_lines:
        emit(line)
    for fam in ("ownership", "quarter_sum", "site_sheet"):
        xs = [x for x in instances if x["family"] == fam]
        by = {}
        for x in xs:
            if x["state"] == "v":
                by[x["reason"]] = by.get(x["reason"], 0) + 1
        emit({"family": fam, "n": len(xs), **{s: sum(1 for x in xs if x["state"] == s)
                                               for s in ("i", "iii", "v", "np")},
              "v_by_reason": dict(sorted(by.items()))})
    for _y, rel in sorted(releases.items()):
        for where, why in rel.refusals:
            emit({"family": "site_sheet", "refused": where, "reason": why})
    for key, subjects in unmatched:
        emit({"family": "site_sheet", "release": key, "no_minestats_column": subjects})
    emit({"components": len(comps), "infeasible_components": len(comp_lines),
          "joint_only": sum(1 for c in comp_lines if c["joint_only"])})
    for c in checks(instances, with_values):
        emit(c)


if __name__ == "__main__":
    main(sys.argv[1:])
