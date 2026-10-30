"""Helper of scripts/register_vintage.sh (bd-s9j3.2). Standard library only; reads no fmine code.

Subcommands, each printing one JSON line per step:
  row <issuer> <url_prefix> <url> <bytes> <sha256> <utc>   one candidate [[source]] row, as TOML, for a file
  rows <workspace> <dir> [--dry-run]                     candidate rows for dir/fetch.jsonl into dir/rows.toml
  oracle <workspace> <dir>                               the independent reader on the fetched files, blind
  diff <workspace> <dir> <bound>                         dir/run_all.json and dir/run_through.json vs the oracle

The oracle step runs scripts/oracle/fidelity.py and census2.py unchanged, census2.py in its --displayed mode
because the fmine ledger scores displayed values (bd-jg43.3), so the two ledger counts the diff prints are
comparable. census2.py reads
<root>/corpus_of_input_docs/downloaded/mine_stats, so the step builds that layout in <dir>/oracle_root. It holds
links to the registered files present in the corpus and the fetched files under their candidate names. Its
output, dir/oracle.jsonl, is written and its sha256 and time are recorded in dir/log.jsonl before any fmine
run; --run checks the sha256 before it runs fmine.
"""
import datetime, hashlib, json, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "oracle"))


def line(**kw):
    print(json.dumps(kw, ensure_ascii=False, separators=(",", ":")))
    sys.stdout.flush()


def refuse(msg, code=2):
    print("register_vintage: " + msg, file=sys.stderr)
    sys.exit(code)


def registry(ws):
    """(top-level keys, [[source]] rows) of registries/sources/barrick.toml, in the subset fetch.sh reads."""
    path = os.path.join(ws, "registries", "sources", "barrick.toml")
    top, rows, table, row = {}, [], None, None
    for n, raw in enumerate(open(path, encoding="utf-8"), 1):
        s = raw.strip()
        if not s or s.startswith("#"):
            continue
        m = re.fullmatch(r"\[\[\s*([a-z_]+)\s*\]\]", s)
        if m:
            table = m.group(1)
            row = {}
            if table == "source":
                rows.append(row)
            continue
        m = re.fullmatch(r'([a-z_][a-z0-9_]*)\s*=\s*(?:"([^"\\]*)"|([0-9_]+))\s*(?:#.*)?', s)
        if not m:
            refuse("%s line %d is outside the subset this helper reads" % (path, n))
        value = m.group(2) if m.group(2) is not None else int(m.group(3).replace("_", ""))
        (top if table is None else row)[m.group(1)] = value
    return top, rows


def grouped(n):
    return "{:,}".format(n).replace(",", "_")


def candidate(issuer, prefix, url, nbytes, sha, utc):
    """(row dict, TOML text, warnings) for one fetched file; refuses when the kind or quarter cannot be read."""
    base = url.rsplit("/", 1)[-1]
    low = base.lower()
    if "aisc" in low:
        kind, suffix = "aisc_reconciliation", "AISC"
    elif "mine_stats" in low or "minestats" in low:
        kind, suffix = "mine_stats", "Mine_Stats"
    else:
        refuse("cannot tell the kind of %s from its name (AISC or Mine_Stats)" % url)
    m = re.search(r"Q([1-4])_(20[0-9]{2})", base, re.I) or None
    if m:
        q, year = m.group(1), m.group(2)
    else:
        m = re.search(r"/(20[0-9]{2})/q([1-4])/", url, re.I)
        if not m:
            refuse("cannot read the quarter of %s" % url)
        year, q = m.group(1), m.group(2)
    warnings = []
    if url.startswith(prefix):
        path = url[len(prefix):]
    else:
        path = url
        warnings.append("outside url_prefix: path is the full URL; registry-check refuses it until resolved")
    row = {"file": "%s_Q%s_%s_%s.xlsx" % (issuer.capitalize(), q, year, suffix), "kind": kind,
           "quarter": "%sQ%s" % (year, q), "path": path,
           "url_basis": "fetched and verified " + utc[:10], "bytes": nbytes, "sha256": sha}
    text = ("# retrieved_utc %s; kind read from the URL name (confirm); evidence OPEN\n[[source]]\n" % utc
            + "".join('%s = "%s"\n' % (k, row[k]) for k in ("file", "kind", "quarter", "path", "url_basis"))
            + "bytes = %s\nsha256 = \"%s\"\n" % (grouped(nbytes), sha))
    return row, text, warnings


def fetched(dir_):
    return [json.loads(s) for s in open(os.path.join(dir_, "fetch.jsonl"), encoding="utf-8") if s.strip()]


def cmd_rows(ws, dir_, dry_run):
    top, rows = registry(ws)
    known = {r.get("sha256"): r.get("file") for r in rows}
    out = []
    for f in fetched(dir_):
        row, text, warnings = candidate(top["issuer"], top["url_prefix"], f["url"], f["bytes"], f["sha256"],
                                        f["utc"])
        already = f["sha256"] in known
        line(stage="row", file_name=row["file"], quarter=row["quarter"], already_registered=already,
             warnings=warnings)
        if already and not dry_run:
            print("already registered: %s" % known[f["sha256"]], file=sys.stderr)
            continue
        out.append(text)
    with open(os.path.join(dir_, "rows.toml"), "w", encoding="utf-8") as h:
        h.write("\n".join(out))
    sys.stderr.write("\n".join(out))
    line(stage="rows", path=os.path.join(dir_, "rows.toml"), count=len(out))


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def cmd_oracle(ws, dir_):
    import census2, fidelity
    top, rows = registry(ws)
    root = os.path.join(dir_, "oracle_root")
    corpus = os.path.join(root, "corpus_of_input_docs", "downloaded", "mine_stats")
    os.makedirs(corpus)
    names = []
    for f in fetched(dir_):
        row, _, _ = candidate(top["issuer"], top["url_prefix"], f["url"], f["bytes"], f["sha256"], f["utc"])
        if row["file"] in names:
            refuse("two fetched files would both be %s" % row["file"])
        os.symlink(os.path.abspath(f["path"]), os.path.join(corpus, row["file"]))
        names.append(row["file"])
    # Registered files present in the corpus, except a name a fetched file already takes (a dry run).
    registered = os.path.join(ws, top["corpus_dir"])
    for r in rows:
        src = os.path.join(registered, r["file"])
        if r["file"] not in names and os.path.exists(src):
            os.symlink(src, os.path.join(corpus, r["file"]))
    _, res = census2.run(root, displayed=True)
    counts = {}
    for x in res:
        counts[x[5]] = counts.get(x[5], 0) + 1
    out = os.path.join(dir_, "oracle.jsonl")
    with open(out, "w", encoding="utf-8") as h:
        for name in names:
            o = fidelity.read(os.path.join(corpus, name))
            h.write(json.dumps({"file": name, "visible": o["visible"], "hidden": o["hidden"],
                                "numeric": o["numeric"], "placeholder": o["placeholder"], "error": o["error"],
                                "text": o["text"], "digest": o["digest"],
                                "instances": sum(1 for x in res if x[0] == name)}, separators=(",", ":")) + "\n")
        h.write(json.dumps({"ledger": dict(sorted(counts.items())), "instances": len(res),
                            "files": sorted(os.listdir(corpus))}, separators=(",", ":")) + "\n")
    digest = sha256_file(out)
    utc = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
    with open(os.path.join(dir_, "log.jsonl"), "a", encoding="utf-8") as h:
        h.write(json.dumps({"stage": "oracle_sealed", "oracle_sha256": digest, "utc": utc}) + "\n")
    for s in open(out, encoding="utf-8"):
        o = json.loads(s)
        if "file" in o:
            line(stage="oracle", file=o["file"], numeric=o["numeric"], digest=o["digest"],
                 instances=o["instances"], oracle_sha256=digest, utc=utc)
    line(stage="oracle_ledger", counts=dict(sorted(counts.items())), instances=len(res), oracle_sha256=digest)


def sealed(dir_):
    """The oracle record: its sha256 must still match, and its time precedes every fmine run."""
    log = [json.loads(s) for s in open(os.path.join(dir_, "log.jsonl"), encoding="utf-8") if s.strip()]
    seals = [o for o in log if o.get("stage") == "oracle_sealed"]
    if len(seals) != 1:
        refuse("expected one oracle_sealed record in %s/log.jsonl, found %d" % (dir_, len(seals)))
    if sha256_file(os.path.join(dir_, "oracle.jsonl")) != seals[0]["oracle_sha256"]:
        refuse("oracle.jsonl changed after it was sealed")
    return seals[0]


def cmd_sealed(dir_):
    s = sealed(dir_)
    line(stage="oracle_check", oracle_sha256=s["oracle_sha256"], oracle_utc=s["utc"], intact=True)


def cmd_diff(ws, dir_, bound):
    seal = sealed(dir_)
    runs = {k: json.load(open(os.path.join(dir_, "run_%s.json" % k), encoding="utf-8")) for k in ("all", "through")}
    for o in (json.loads(s) for s in open(os.path.join(dir_, "log.jsonl"), encoding="utf-8") if s.strip()):
        if o.get("stage") == "fmine" and not o["utc"] > seal["utc"]:
            refuse("an fmine run at %s does not follow the oracle seal at %s" % (o["utc"], seal["utc"]))
    for k, v in runs.items():
        if not isinstance(v.get("payload"), dict):
            refuse("the fmine run on %s has no payload (status %s, exit %s)" % (k, v.get("status"),
                                                                                 v.get("exit_code")), 1)
    pay = {k: v["payload"] for k, v in runs.items()}
    oracle = [json.loads(s) for s in open(os.path.join(dir_, "oracle.jsonl"), encoding="utf-8") if s.strip()]
    per_file = {o["file"]: o for o in oracle if "file" in o}
    sources = {s["file"]: s for s in pay["all"]["sources"]}
    census_equal = True
    for name, o in sorted(per_file.items()):
        s = sources.get(name)
        got = None if s is None else [s["visible_sheets"], s["hidden_sheets"], s["numeric_cells"],
                                      s["placeholder_cells"], s["formula_error_cells"], s["text_cells"],
                                      s["numeric_digest"]]
        want = [o["visible"], o["hidden"], o["numeric"], o["placeholder"], o["error"], o["text"], o["digest"]]
        equal = got == want
        census_equal = census_equal and equal
        line(stage="census", file=name, registered=s is not None, equal=equal)
    ledger = {k: pay[k]["identity_ledger"] for k in pay}
    new_files = set(per_file)
    new_exceptions = [e for e in ledger["all"]["exceptions"] if e["file"] in new_files]
    for e in new_exceptions:
        line(stage="new_instance", file=e["file"], sheet=e["sheet"], cell=e["cell"], subject=e["subject"],
             family=e["family"], state=e["state"])
    for k in ("unregistered_headers", "table_refusals"):
        for item in ledger["all"][k]:
            line(stage=k, item=item)
    oracle_ledger = next(o for o in oracle if "ledger" in o)
    line(stage="diff", bound=bound, root_all=pay["all"]["state_root"], root_through=pay["through"]["state_root"],
         census_equal=census_equal,
         counts={"through": ledger["through"]["totals"], "all": ledger["all"]["totals"],
                 "oracle_census2": oracle_ledger["ledger"]},
         new_instances=ledger["all"]["instances"] - ledger["through"]["instances"],
         new_exceptions=len(new_exceptions))


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["row"] and len(a) == 7:
        _, text, warnings = candidate(a[1], a[2], a[3], int(a[4]), a[5], a[6])
        sys.stdout.write(text)
    elif a[:1] == ["rows"] and len(a) in (3, 4):
        cmd_rows(a[1], a[2], a[3:] == ["--dry-run"])
    elif a[:1] == ["oracle"] and len(a) == 3:
        cmd_oracle(a[1], a[2])
    elif a[:1] == ["sealed"] and len(a) == 2:
        cmd_sealed(a[1])
    elif a[:1] == ["diff"] and len(a) == 4:
        cmd_diff(a[1], a[2], a[3])
    else:
        sys.exit(__doc__)
