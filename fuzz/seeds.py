"""Write the starting corpus of every fuzz target (see scripts/fuzz.sh).

Usage: python3 -I fuzz/seeds.py <repo-root> <out-dir>

Each target gets a directory of seed files in its own input format (fuzz_targets/*.rs): a selector byte, where the
target has one, then the bytes. All content is invented or copied from committed registries and rules; no issuer
value, workbook or transcription is read. The one exception is opt-in: with FMINE_OMF_DIR set to an omf-rust
checkout, the omf target also gets the published sample files under 64 KiB (MIT, invented geometry).
"""

import gzip
import io
import json
import os
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


# OMF: Parquet in the subset fmine_io::omf reads (v1 or v2 data pages, PLAIN values, RLE or bit-packed levels, no
# codec, so that a mutated page reaches the decoders and not a gzip CRC-32), ported from omf::parquet::testkit.

BOOL, I32, I64, F32, F64, BYTES = 0, 1, 2, 4, 5, 6


def varint(v: int) -> bytes:
    out = bytearray()
    while True:
        b, v = v & 0x7F, v >> 7
        if v == 0:
            out.append(b)
            return bytes(out)
        out.append(b | 0x80)


def zigzag(v: int) -> bytes:
    return varint(2 * v if v >= 0 else -2 * v - 1)


# A Thrift compact value: ("bool", b), ("i8", v), ("i32", v), ("i64", v), ("bin", bytes), ("list", kind, items) or
# ("struct", [(field id, value)]).
KIND = {"i8": 3, "i32": 5, "i64": 6, "bin": 8, "list": 9, "struct": 12}


def kind(v) -> int:
    return (1 if v[1] else 2) if v[0] == "bool" else KIND[v[0]]


def ser(v) -> bytes:
    t = v[0]
    if t == "bool":
        return bytes([1 if v[1] else 0])
    if t == "i8":
        return bytes([v[1] & 0xFF])
    if t in ("i32", "i64"):
        return zigzag(v[1])
    if t == "bin":
        return varint(len(v[1])) + v[1]
    if t == "list":
        items = v[2]
        head = bytes([(len(items) << 4) | v[1]]) if len(items) < 15 else bytes([0xF0 | v[1]]) + varint(len(items))
        return head + b"".join(ser(i) for i in items)
    out, last = bytearray(), 0
    for fid, x in v[1]:
        delta = fid - last
        out += bytes([(delta << 4) | kind(x)]) if 1 <= delta <= 15 else bytes([kind(x)]) + zigzag(fid)
        if x[0] != "bool":
            out += ser(x)
        last = fid
    return bytes(out + b"\x00")


def st(*fields):
    return ("struct", list(fields))


def i32(v):
    return ("i32", v)


def i64(v):
    return ("i64", v)


def col(name, physical, values, logical=None, present=None):
    """A column; `present` makes it nullable (its values are the present rows only)."""
    return {"name": name, "physical": physical, "values": values, "logical": logical,
            "optional": present is not None, "present": present, "group": None}


def grouped(c, group, present):
    """`c`, required, inside an optional group, with `present` rows."""
    return {**c, "optional": False, "present": present, "group": group}


def logical_fields(logical):
    union = lambda fid, inner: [(10, st((fid, st(*inner))))]  # noqa: E731
    if logical is None:
        return []
    if logical == "string":
        return union(1, [])
    if logical == "date":
        return union(6, [])
    if logical == "micros":
        return union(8, [(1, ("bool", True)), (2, st((2, st())))])
    _, bits, signed = logical
    return union(10, [(1, ("i8", bits)), (2, ("bool", signed))])


def levels(present, bitpacked) -> bytes:
    if bitpacked:
        out = bytearray(varint(((len(present) + 7) // 8) << 1 | 1))
        for g in range(0, len(present), 8):
            out.append(sum(1 << i for i, x in enumerate(present[g:g + 8]) if x))
        return bytes(out)
    out, i = bytearray(), 0
    while i < len(present):
        j = next((j for j in range(i, len(present)) if present[j] != present[i]), len(present))
        out += varint((j - i) << 1) + bytes([1 if present[i] else 0])
        i = j
    return bytes(out)


def plain(physical, values) -> bytes:
    if physical == BOOL:
        return bytes(sum(1 << i for i, x in enumerate(values[g:g + 8]) if x) for g in range(0, len(values), 8))
    if physical == BYTES:
        return b"".join(struct.pack("<I", len(v)) + v for v in values)
    fmt = {I32: "<i", I64: "<q", F32: "<f", F64: "<d"}[physical]
    return b"".join(struct.pack(fmt, v) for v in values)


def parquet(cols, rows, form) -> bytes:
    """A Parquet file of `rows` rows holding `cols`, in `form` = (v2, rows per page, rows per group, bit-packed)."""
    v2, per_page, per_group, bitpacked = form
    out = bytearray(b"PAR1")
    groups, next_value, g0 = [], [0] * len(cols), 0
    while True:
        g1 = min(rows, g0 + per_group)
        chunks = []
        for ci, c in enumerate(cols):
            start, p0 = len(out), g0
            while True:
                p1 = min(g1, p0 + per_page)
                pres = c["present"][p0:p1] if c["present"] is not None else [True] * (p1 - p0)
                k = sum(pres)
                vals = plain(c["physical"], c["values"][next_value[ci]:next_value[ci] + k])
                next_value[ci] += k
                lv = levels(pres, bitpacked) if c["present"] is not None else b""
                n = p1 - p0
                if v2:
                    body = lv + vals
                    header = st((1, i32(3)), (2, i32(len(body))), (3, i32(len(body))),
                                (8, st((1, i32(n)), (2, i32(n - k)), (3, i32(n)), (4, i32(0)), (5, i32(len(lv))),
                                       (6, i32(0)), (7, ("bool", False)))))
                else:
                    body = (struct.pack("<I", len(lv)) + lv if c["present"] is not None else b"") + vals
                    header = st((1, i32(0)), (2, i32(len(body))), (3, i32(len(body))),
                                (5, st((1, i32(n)), (2, i32(0)), (3, i32(3)), (4, i32(3)))))
                out += ser(header) + body
                p0 = p1
                if p0 >= g1:
                    break
            path = ([("bin", c["group"].encode())] if c["group"] else []) + [("bin", c["name"].encode())]
            size = len(out) - start
            chunks.append(st((2, i64(start)), (3, st((1, i32(c["physical"])), (2, ("list", 5, [i32(0), i32(3)])),
                                                     (3, ("list", 8, path)), (4, i32(0)), (5, i64(g1 - g0)),
                                                     (6, i64(size)), (7, i64(size)), (9, i64(start))))))
        groups.append(st((1, ("list", 12, chunks)), (2, i64(0)), (3, i64(g1 - g0))))
        if g1 >= rows:
            break
        g0 = g1
    top, i = [], 0
    while i < len(cols):
        c = cols[i]
        leaf = lambda c: st((1, i32(c["physical"])), (3, i32(1 if c["optional"] else 0)),  # noqa: E731
                            (4, ("bin", c["name"].encode())), *logical_fields(c["logical"]))
        if c["group"]:
            members = [m for m in cols[i:] if m["group"] == c["group"]]
            top.append(st((3, i32(1)), (4, ("bin", c["group"].encode())), (5, i32(len(members)))))
            top += [leaf(m) for m in members]
            i += len(members)
        else:
            top.append(leaf(c))
            i += 1
    roots = sum(1 for j, c in enumerate(cols) if not c["group"] or j == 0 or cols[j - 1]["group"] != c["group"])
    schema = [st((4, ("bin", b"schema")), (5, i32(roots)))] + top
    meta = ser(st((1, i32(2)), (2, ("list", 12, schema)), (3, i64(rows)), (4, ("list", 12, groups)),
                  (6, ("bin", b"fmine fuzz seeds"))))
    return bytes(out + meta + struct.pack("<I", len(meta)) + b"PAR1")


def omf_input(index: bytes, members) -> bytes:
    """The omf target's input: the index text and the members, each length-prefixed."""
    out = struct.pack("<I", len(index)) + index
    for name, data in members:
        out += bytes([len(name)]) + name.encode() + struct.pack("<I", len(data)) + data
    return out


def omf_every_kind(form) -> bytes:
    """Every geometry and attribute type, as omf::tests::every_kind builds them, with Parquet in `form`."""
    members = []

    def arr(cols, rows):
        name = f"{len(members) + 1}.parquet"
        members.append((name, parquet(cols, rows, form)))
        return {"filename": name, "item_count": rows}

    def image(data, ext):
        name = f"{len(members) + 1}.{ext}"
        members.append((name, data))
        return {"filename": name, "item_count": 0}

    def xyz(points, physical=F64):
        return arr([col(a, physical, [p[i] for p in points]) for i, a in enumerate("xyz")], len(points))

    def u32(name, v):
        return col(name, I32, v, ("int", 32, False))

    def u8(name, v):
        return col(name, I32, v, ("int", 8, False))

    def ramp(rows):
        return arr([u8(ch, [r[i] for r in rows]) for i, ch in enumerate("rgba")], len(rows))

    def attribute(name, location, data):
        return {"name": name, "units": "u", "location": location, "data": data}

    def element(name, geometry, attributes):
        return {"name": name, "geometry": geometry, "attributes": attributes}

    tri, micros_2000 = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], 946_684_800_000_000
    points = {"name": "points", "color": [1, 2, 3, 255], "metadata": {"m": "x"},
              "geometry": {"type": "PointSet", "origin": [1, 2, 3], "vertices": xyz(tri)},
              "attributes": [attribute("grade", "Vertices", {
                  "type": "Number", "values": arr([col("number", F64, [1.5, 2.5], None, [True, False, True])], 3),
                  "colormap": {"type": "Continuous", "range": {"min": 0, "max": 10},
                               "gradient": ramp([[0, 0, 0, 255], [255, 255, 255, 255]])}})]}
    lines = element("lines", {"type": "LineSet",
                              "vertices": xyz([[0.0, 0.0, 0.0], [0.1, 1.0, 1.0], [2.0, 2.0, -2.5]], F32),
                              "segments": arr([u32("a", [0, 1]), u32("b", [1, 2])], 2)},
                    [attribute("label", "Primitives", {
                        "type": "Text", "values": arr([col("text", BYTES, [b"a"], "string", [True, False])], 2)})])
    present = [True, False, True]
    paint = arr([grouped(u8(ch, v), "color", present) for ch, v in zip("rgba", ([1, 5], [2, 6], [3, 7], [4, 8]))], 3)
    png = image(b"\x89PNG\r\n\x1a\nnot really an image", "png")
    uv = arr([col("u", F64, [0.0, 1.0, 0.0]), col("v", F64, [0.0, 0.0, 1.0])], 3)
    jpg = image(bytes([0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3]), "jpg")
    surface = element("tri", {"type": "Surface", "vertices": xyz(tri),
                              "triangles": arr([u32("a", [0]), u32("b", [1]), u32("c", [2])], 1)}, [
        attribute("paint", "Vertices", {"type": "Color", "values": paint}),
        attribute("tex", "Vertices", {"type": "MappedTexture", "image": png, "texcoords": uv}),
        attribute("proj", "Projected", {"type": "ProjectedTexture", "image": jpg,
                                        "orient": {"origin": [0, 0, 0], "u": [1, 0, 0], "v": [0, 1, 0]},
                                        "width": 2, "height": 3})])
    grid = element("grid", {"type": "GridSurface", "orient": {"origin": [0, 0, 0], "u": [0, 1, 0], "v": [-1, 0, 0]},
                            "grid": {"type": "Regular", "size": [1, 2], "count": [2, 1]},
                            "heights": arr([col("scalar", F64, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0])], 6)},
                   [attribute("wet", "Primitives", {
                       "type": "Boolean", "values": arr([col("bool", BOOL, [True], None, [True, False])], 2)})])
    parents, corners = [[0, 0, 0], [1, 0, 0], [1, 0, 0]], [[0, 0, 0, 2, 2, 2], [0, 0, 0, 1, 1, 1], [1, 1, 1, 2, 2, 2]]
    sub = arr([u32(f"parent_{a}", [p[i] for p in parents]) for i, a in enumerate("uvw")]
              + [u32(f"corner_{m}_{a}", [c[3 * j + i] for c in corners])
                 for j, m in enumerate(("min", "max")) for i, a in enumerate("uvw")], 3)
    blocks = element("blocks", {
        "type": "BlockModel",
        "orient": {"origin": [10, 20, 30], "u": [0.6, 0.8, 0], "v": [-0.8, 0.6, 0], "w": [0, 0, 1]},
        "grid": {"type": "Regular", "size": [10, 10, 5], "count": [2, 1, 1]},
        "subblocks": {"type": "Regular", "count": [2, 2, 2], "subblocks": sub, "mode": "Octree"}}, [
        {"name": "rock", "units": "u", "location": "Subblocks", "data": {
            "type": "Category",
            "values": arr([col("index", I32, [0, 1], ("int", 32, False), [True, True, False])], 3),
            "names": arr([col("name", BYTES, [b"ore", b"waste"], "string")], 2),
            "gradient": ramp([[255, 0, 0, 255], [0, 0, 255, 255]]),
            "attributes": [attribute("density", "Categories", {
                "type": "Number", "values": arr([col("number", I64, [3], None, [True, False])], 2)})]}},
        attribute("mined", "Primitives", {
            "type": "Number", "values": arr([col("number", I64, [micros_2000], "micros", [True, False])], 2),
            "colormap": {"type": "Discrete",
                         "boundaries": arr([col("value", I64, [0, micros_2000], "micros"),
                                            col("inclusive", BOOL, [False, True])], 2),
                         "gradient": ramp([[1, 1, 1, 1], [2, 2, 2, 2], [3, 3, 3, 3]])}})])
    free = arr([u32("parent_u", [1]), u32("parent_v", [0]), u32("parent_w", [2])]
               + [col(f"corner_{m}_{a}", F32, [v]) for m, vs in (("min", (0.0, 0.0, 0.0)), ("max", (0.5, 1.0, 0.25)))
                  for a, v in zip("uvw", vs)], 1)
    tensor = element("tensor", {
        "type": "BlockModel", "orient": {"origin": [0, 0, 0]},
        "grid": {"type": "Tensor", "u": arr([col("scalar", F64, [1.0, 2.0])], 2),
                 "v": arr([col("scalar", F64, [3.0])], 1), "w": arr([col("scalar", F32, [4.0, 5.0, 6.5])], 3)},
        "subblocks": {"type": "Freeform", "subblocks": free}}, [
        attribute("when", "Subblocks", {
            "type": "Number", "values": arr([col("number", I32, [10_957], "date", [True])], 1)}),
        attribute("dir", "Subblocks", {"type": "Vector", "values": arr(
            [grouped(col("x", F64, [1.0]), "vector", [True]), grouped(col("y", F64, [2.0]), "vector", [True])], 1)})])
    children = [element("child", {"type": "PointSet", "vertices": xyz([p])}, []) for p in ([5.0] * 3, [6.0] * 3)]
    composite = element("group", {"type": "Composite", "elements": children}, [
        attribute("arrow", "Elements", {"type": "Vector", "values": arr(
            [grouped(col(a, F64, [v]), "vector", [True, False]) for a, v in zip("xyz", (0.0, 0.0, 1.0))], 2)})])
    project = {"name": "pit", "description": "d", "coordinate_reference_system": "EPSG:32633", "units": "m",
               "origin": [100, 200, 0], "author": "a", "application": "t", "date": "2000-01-01T00:00:00Z",
               "metadata": {"k": 1}, "unknown_key": True,
               "elements": [points, lines, surface, grid, blocks, tensor, composite]}
    return omf_input(json.dumps(project).encode(), members)


def omf_samples(directory: str) -> list[bytes]:
    """The omf-rust sample files under `directory` of at most 64 KiB, in the omf target's input format."""
    out = []
    for path in sorted(Path(directory).rglob("*.omf")):
        if path.stat().st_size > 64 << 10:
            continue
        try:
            with zipfile.ZipFile(path) as z:
                index = gzip.decompress(z.read("index.json.gz"))
                members = [(n, z.read(n)) for n in z.namelist() if n != "index.json.gz" and len(n) < 256]
        except (OSError, KeyError, zipfile.BadZipFile):
            continue
        out.append(omf_input(index, members))
    return out


def main() -> None:
    root, out = Path(sys.argv[1]), Path(sys.argv[2])
    seeds: dict[str, list[bytes]] = {t: [] for t in
                                     ("codec", "anchor", "journal", "xlsx", "numfmt", "registry", "record", "html",
                                      "cli", "cli_run", "pit", "cpit", "pcpsp", "omf")}

    # omf: every geometry and attribute type in v1 and v2 pages, then with pages and row groups split small and
    # bit-packed levels; then the omf-rust samples, when FMINE_OMF_DIR names a checkout.
    big = 1 << 62
    for form in ((False, big, big, False), (True, big, big, False), (False, 2, 3, True), (True, 1, 2, True)):
        seeds["omf"].append(omf_every_kind(form))
    if os.environ.get("FMINE_OMF_DIR"):
        seeds["omf"] += omf_samples(os.environ["FMINE_OMF_DIR"])

    # codec: mode, script length, script (one read per byte), payload. A script byte's kind is its value mod 11 and
    # its bound its high nibble: every kind once, then bytes, strings and sequences under small, mid and no bounds.
    rng = random.Random(17)
    payload = bytes(rng.randrange(256) for _ in range(160))
    for mode in (0, 1, 2, 5):
        script = bytes(range(11)) + bytes([0x13, 0x2A, 0x36, 0x4A, 0x15, 0xFA, 0xFB, 0xFC])
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

    # cli: selector 0 and NUL-separated arguments, one seed per registered verb, plain and with common options;
    # selector 1 and NUL-separated envelope fields.
    verbs = re.findall(rb'^path = "([^"]+)"', (reg / "verbs.toml").read_bytes(), re.MULTILINE)
    options = [b"--issuer", b"barrick", b"--through", b"2026Q2", b"--format", b"markdown", b"--workspace", b"w"]
    for v in verbs:
        words = v.split(b" ")
        seeds["cli"].append(b"\x00" + b"\x00".join(words))
        seeds["cli"].append(b"\x00" + b"\x00".join(words + options))
    seeds["cli"].append(b"\x00" + b"\x00".join([b"assure", b"public", b"--issuer", b"barrick", b"--output", b"o"]))
    seeds["cli"].append(b"\x01" + b"\x00".join(
        [b"assure public", b"what \"quoted\"", b"fmine robot-docs", b"why\tnow", b"w1", b"anc_x", b"00ff"]))

    # pit: a selector (decimal places mod 10 in the low nibble; bit 4 asks for rounding), then the .upit text, NUL,
    # the .prec text, NUL, and a candidate pit. One invented five-block pair: 2 places, rounding to 2 places, 6
    # places, and 1 place, which refuses 6.25 unless rounding is asked for.
    upit = "NAME: tiny\nTYPE: UPIT\nNBLOCKS: 5\nOBJECTIVE_FUNCTION:\n0 -1.50\n1 -2.00\n2 -1.00\n3 6.25\n4 -0.10\nEOF\n"
    prec = "0 0\n1 0\n2 0\n3 3 0 1 2\n4 1 3\n"
    for selector, pick in ((2, "0 1 2 3"), (0x12, "% candidate\n3 4"), (6, ""), (1, "0 1 2 3 4")):
        seeds["pit"].append(bytes([selector]) + f"{upit}\0{prec}\0{pick}".encode())

    # cpit: a selector (value places mod 10 in the low nibble; bit 4 asks for rounding; use places in the top three
    # bits), then the .cpit text, NUL, the .prec text, NUL, and rank bytes (255 leaves a block out). One invented
    # three-block pair with one resource over two periods.
    cpit = ("NAME: tiny\nTYPE: CPIT\nNBLOCKS: 3\nNPERIODS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 1\nDISCOUNT_RATE: 0.08\n"
            "RESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n0 1 L 2\nOBJECTIVE_FUNCTION:\n0 -1.5\n1 -2\n2 9.25\n"
            "RESOURCE_CONSTRAINT_COEFFICIENTS:\n0 0 1\n1 0 1\n2 0 1.5\nEOF\n")
    cprec = "0 0\n1 0\n2 2 0 1\n"
    for selector, ranks in ((0x22, b""), (0x22, bytes([0, 1, 255])), (0x31, bytes([2, 1, 0])), (0x00, b"")):
        seeds["cpit"].append(bytes([selector]) + f"{cpit}\0{cprec}\0".encode() + ranks)

    # pcpsp: the selector as for cpit, then the .pcpsp text, NUL, the .prec text, NUL, and choice bytes (low nibble:
    # destination; high nibble: rank, 15 leaves the block out). The unit tests' three-block pair: two destinations
    # (dump, mill), a mining resource both use and a mill resource only the mill uses.
    pcpsp = ("NAME: tiny\nTYPE: PCPSP\nNBLOCKS: 3\nNPERIODS: 2\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 2\n"
             "NGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0.1\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n0 1 L 2\n1 0 L 1\n"
             "1 1 L 1\nOBJECTIVE_FUNCTION:\n% dump, mill\n0 -1 -3\n1 -1 -3\n2 -2 9.5\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n"
             "0 0 0 1\n0 1 0 1\n0 1 1 1\n1 0 0 1\n1 1 0 1\n1 1 1 1\n2 0 0 1\n2 1 0 1\n2 1 1 1\nEOF\n")
    for selector, choices in ((0x01, b""), (0x01, bytes([0x00, 0x00, 0x01])), (0x21, bytes([0x10, 0x20, 0x01])),
                              (0x11, bytes([0xF0, 0x01, 0x11])), (0x00, b"")):
        seeds["pcpsp"].append(bytes([selector]) + f"{pcpsp}\0{cprec}\0".encode() + choices)
    # The unit tests' blend (bd-ixfw): a mill for two blocks and a blending row with limit 0, where block 1's negative
    # use makes room for one of blocks 0 and 2. Uses at one place (selector 0x20); every block to the mill, ranked
    # either way round.
    blend = ("NAME: blend\nTYPE: PCPSP\nNBLOCKS: 3\nNPERIODS: 1\nNDESTINATIONS: 2\nNRESOURCE_SIDE_CONSTRAINTS: 2\n"
             "NGENERAL_SIDE_CONSTRAINTS: 0\nDISCOUNT_RATE: 0\nRESOURCE_CONSTRAINT_LIMITS:\n0 0 L 2\n1 0 L 0\n"
             "OBJECTIVE_FUNCTION:\n0 -1 4\n1 -1 1\n2 -1 3\nRESOURCE_CONSTRAINT_COEFFICIENTS:\n0 1 0 1\n0 1 1 1\n1 1 0 1\n"
             "1 1 1 -1.25\n2 1 0 1\n2 1 1 0.5\nEOF\n")
    bprec = "0 0\n1 0\n2 0\n"
    for choices in (bytes([0x01, 0x21, 0x11]), bytes([0x21, 0x01, 0x11])):
        seeds["pcpsp"].append(bytes([0x20]) + f"{blend}\0{bprec}\0".encode() + choices)

    # cli_run: one byte per vocabulary entry, in the order fuzz_targets/cli_run.rs builds it (verb paths, then its
    # FLAGS, PAIRS and VALUES). Each verb alone, with a selector, with options, and as a full mutation command. A
    # tuple names a pair.
    source = (root / "fuzz" / "fuzz_targets" / "cli_run.rs").read_text()
    def block(name: str) -> str:
        found = re.search(r"const " + name + r": &\[[^\n]*\] = &\[(.*?)\n\];", source, re.DOTALL)
        return found.group(1) if found else ""
    lit = r'"((?:[^"\\]|\\.)*)"'
    flags, values = re.findall(lit, block("FLAGS")), re.findall(lit, block("VALUES"))
    pairs = re.findall(r"\(\s*" + lit + r",\s*" + lit + r",?\s*\)", block("PAIRS"))
    vocab = [v.decode() for v in verbs] + flags + pairs + values
    assert len(vocab) <= 256 and len(flags) > 50 and len(pairs) > 40 and len(values) > 40
    def run(*words) -> bytes:
        return bytes(vocab.index(w) for w in words)
    for v in verbs:
        p = v.decode()
        seeds["cli_run"] += [run(p), run(p, "--synthetic"), run(p, ("--format", "markdown")),
                             run(p, ("--issuer", "barrick"), ("--through", "2025Q2"), "--as-of", "2026-01-15",
                                 "--workspace", "--json")]
    # Commands each parser accepts, so the fuzzer starts at the verbs behind them. The reliance verbs come in both
    # forms: host clock (--trust-host-clock) and explicit trust inputs (--key, --now, --uncertainty-ns).
    zero = "0" * 64
    jr, hd, tr = ("--journal", "journal"), ("--head", zero), "--trust-host-clock"
    now, unc = ("--now", "2026-11-01T00:00:00Z"), ("--uncertainty-ns", "0")
    auth = (jr, ("--expected-head", zero), ("--key", "key"), now, unc)
    seeds["cli_run"] += [
        run("forecast journal init", jr, ("--cell", "demo"), ("--max-clock-skew-ms", "1"), tr),
        *(run(v, jr, tr, hd, ("--request", "request.toml"))
          for v in ("forecast journal register", "forecast journal freeze", "forecast journal outcome")),
        run("forecast journal assess", jr, tr, hd, ("--cohort", "demo"), ("--as-of", "2026-10-31T00:00:00Z"),
            ("--selector", "latest-as-of"), ("--expected-digest", zero)),
        run("forecast journal inspect", jr, hd),
        run("forecast journal report", jr, hd, ("--assessment", zero), ("--provenance", "provenance.toml")),
        run("reliance init", jr, ("--release-key", "release.key"), ("--max-age-seconds", "300"),
            ("--max-clock-skew-ms", "1")),
        run("reliance init", jr, ("--key", "key"), now, unc, ("--max-age-ns", "10000000000"),
            ("--max-skew-ns", "0")),
        run("reliance issue", jr, hd, tr, "--synthetic", ("--release-key", "release.key"),
            ("--output", "report.md"), ("--purpose", "review"), ("--recipient-role", "sponsor"),
            ("--code-revision", "0123456789abcdef0123456789abcdef01234567"), ("--artifact-format", "json")),
        run("reliance reuse", jr, hd, tr, ("--artifact", "report.json"), ("--output", "report.md")),
        run("reliance reissue", jr, hd, tr, ("--release-key", "release.key"), ("--artifact", "report.json"),
            ("--output", "report.md"), ("--source-issuance", zero), ("--recipient-role", "sponsor"),
            ("--purpose", "review")),
        run("reliance withdraw", jr, hd, tr, "--all-claims", ("--reason", "seeded-code-defect"),
            ("--release-key", "release.key"), ("--digest", zero), ("--defect", "code-defect")),
        run("reliance withdraw", zero, *auth, ("--reason", "seeded-code-defect"), ("--defect", "code-defect"),
            ("--claim", "clm_00112233445566778899aabbccddeeff")),
        run("reliance supersede", jr, hd, tr, ("--release-key", "release.key"), ("--digest", zero),
            ("--replacement", zero)),
        run("reliance supersede", zero, *auth, ("--replacement", zero)),
        run("reliance refresh", jr, hd, tr, ("--release-key", "release.key")),
        run("reliance refresh", *auth),
        run("reliance recall", jr, ("--code-revision", "0123456789abcdef0123456789abcdef01234567")),
        run("reliance check", zero, jr, hd, now, unc, ("--as-of", "2026-10-31T00:00:00Z")),
        run("reliance check", jr, ("--digest", zero)),
        run("recall", jr, ("--revision", "0123456789abcdef0123456789abcdef01234567")),
        run("recall", jr, ("--claim", "clm_00112233445566778899aabbccddeeff")),
        run("assure issue", "--synthetic", ("--output", "report.md"), *auth, ("--role", "sponsor"),
            ("--purpose", "review")),
        run("reliance export", "report", ("--output", "report.md"), jr, hd, now, unc),
        run("forecast record score", ("--record", "record"), ("--as-of", "2026-10-31T00:00:00Z"),
            ("--tsa-roots", "trusted.pem")),
        run("lint", "draft.md", "--synthetic"),
    ]

    for target, items in seeds.items():
        d = out / target
        d.mkdir(parents=True, exist_ok=True)
        for i, item in enumerate(items):
            (d / f"seed-{i:02d}").write_bytes(item)
        print(f'{{"target":"{target}","seeds":{len(items)}}}')


if __name__ == "__main__":
    main()
