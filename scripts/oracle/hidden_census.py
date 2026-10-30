"""Hidden-scope census for BD-331.S2: the visible-sheet readers, pointed at the hidden AISC sheets instead.

census2.py (written before the Rust) and share_census.py (written by a separate agent before the Rust share rule)
choose AISC sheets through census2.visible. This wrapper adds no ledger logic. For each `*AISC.xlsx` file it
replaces that choice with the file's hidden and very hidden sheets, runs both readers unchanged, and prints their
states. Mine Stats files keep their visible sheets, so share_census.py still reads the visible QTD label.
Written 2026-10-30 by the agent implementing the Rust hidden scope, before that code was run on the corpus.

Output: one JSON line per instance {file, sheet, column, subject, family, state}, where each COS/oz instance
census2.py refuses as "attributable share needed" takes share_census.py's state; then a summary line
{hidden_sheets, instances, footing, per_oz}, each a {state: count} map.
usage: python3 -I scripts/oracle/hidden_census.py <workspace_root>
"""
import json
import os
import sys
from collections import Counter

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import census2  # noqa: E402
import share_census  # noqa: E402
from xl import load  # noqa: E402

VISIBLE = census2.visible


def hidden(path):
    if not path.endswith("AISC.xlsx"):
        return VISIBLE(path)
    shown = set(VISIBLE(path))
    return [s for s in load(path) if s not in shown]


def main(root):
    census2.visible = hidden
    try:
        _, res = census2.run(root)
        shares = {(r["file"], r["sheet"], r["cell"].rstrip("0123456789"), r["subject"]): r["state"]
                  for _, r in share_census.run(root)}
    finally:
        census2.visible = VISIBLE
    sheets, footing, per_oz = set(), Counter(), Counter()
    for fn, sh, col, subj, fam, state, why in res:
        if fam == "COS/oz" and state == "v" and why == "attributable share needed":
            state = shares[(fn, sh, col, subj)]
        print(json.dumps({"file": fn, "sheet": sh, "column": col, "subject": subj, "family": fam, "state": state}))
        sheets.add((fn, sh))
        (per_oz if "/" in fam else footing)[state] += 1
    print(json.dumps({"hidden_sheets": len(sheets), "instances": len(res), "footing": dict(sorted(footing.items())),
                      "per_oz": dict(sorted(per_oz.items()))}))


if __name__ == "__main__":
    main(sys.argv[1])
