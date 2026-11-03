"""Convert transparency-dev/merkle's published RFC 6962 proof probes to the line table fmine-ledger tests read.

Usage: python3 -I scripts/oracle/ct_merkle_probes.py <merkle checkout>

The input is a checkout of https://github.com/transparency-dev/merkle (Apache-2.0). Only its JSON probes under
testdata/inclusion and testdata/consistency are read. Each probe is a proof with its sizes, hashes and whether the
upstream verifier must refuse it (`wantErr`). This script decodes the base64 and writes one line per probe, sorted
by path, to stdout:

    inclusion <path> <leaf_index> <tree_size> <leaf_hash> <root> <proof> <ok|err>
    consistency <path> <size1> <size2> <root1> <root2> <proof> <ok|err>

A hash is lower-case hex, or `-` when it is empty. A proof is its hashes joined by commas, or `none` when it has no
hashes. The script computes nothing about Merkle trees: it is independent of the Rust code, and the probes'
answers are upstream's.

On stderr it prints the inputs digest for the capture record: the sha256 of the lines `sha256:<hex> <path>`, one
per probe file, sorted by path, each ending in a newline.
"""

import base64
import hashlib
import json
import pathlib
import sys

KINDS = {
    "inclusion": ("leafIdx", "treeSize", "leafHash", "root"),
    "consistency": ("size1", "size2", "root1", "root2"),
}


def hex_of(field):
    if field is None:
        return "-"
    raw = base64.b64decode(field, validate=True)
    return raw.hex() if raw else "-"


def proof_of(field):
    if not field:
        return "none"
    return ",".join(hex_of(p) for p in field)


def main(argv):
    if len(argv) != 2:
        sys.exit(__doc__.splitlines()[2])
    root = pathlib.Path(argv[1])
    manifest = []
    lines = []
    for kind, (a, b, h1, h2) in KINDS.items():
        for path in sorted((root / "testdata" / kind).rglob("*.json")):
            rel = path.relative_to(root).as_posix()
            if any(c.isspace() for c in rel):
                sys.exit(f"path with whitespace: {rel}")
            data = path.read_bytes()
            manifest.append(f"sha256:{hashlib.sha256(data).hexdigest()} {rel}")
            probe = json.loads(data)
            for key in (a, b):
                if not isinstance(probe[key], int) or probe[key] < 0:
                    sys.exit(f"{rel}: {key} is not a size")
            if not isinstance(probe["wantErr"], bool):
                sys.exit(f"{rel}: wantErr is not a bool")
            shown = rel.removeprefix("testdata/")
            want = "err" if probe["wantErr"] else "ok"
            lines.append(
                f"{kind} {shown} {probe[a]} {probe[b]} {hex_of(probe.get(h1))} {hex_of(probe.get(h2))} "
                f"{proof_of(probe.get('proof'))} {want}"
            )
    manifest.sort(key=lambda line: line.split(" ", 1)[1])
    lines.sort(key=lambda line: line.split(" ", 2)[1])
    digest = hashlib.sha256("".join(f"{m}\n" for m in manifest).encode()).hexdigest()
    sys.stdout.write("".join(f"{line}\n" for line in lines))
    print(f"inputs {len(manifest)} sha256:{digest}", file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv)
