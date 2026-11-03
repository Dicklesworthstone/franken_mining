"""Assemble the upstream winding oracle for fmine-geom's differential corpus (bd-192).

The oracle is frankensim's fs-rep-mesh `winding.rs` and `shapes.rs` at the pinned commit, copied byte for byte,
with the `Point3` and `Vec3` items of fs-geom's `lib.rs` (lines 86 to 165, nothing else from that file, which pulls
in six other crates) as a one-file `fs-geom` crate. Each source is checked against its sha256 at the pinned commit
before it is written. `driver.rs`, from this directory, is the only new code: it builds the soups and query points
and prints every answer's f64 bits. Upstream evaluates `atan2` with std, which is the point of the comparison.

Usage:
    python3 -I scripts/oracle/winding_corpus/assemble.py <frankensim checkout> <empty output directory>
    cd <output directory> && cargo run -q -p winding-oracle > winding_corpus.txt

stderr gets the inputs digest: the sha256 of the lines `sha256:<hex> <name>` for the four inputs (the three
upstream files and driver.rs), in the order below, each ending in a newline.
"""

import hashlib
import os
import subprocess
import sys

COMMIT = "20463525d9775a18c1b237d2676f364f367175ab"

# (path at the commit, its sha256 there)
UPSTREAM = {
    "winding": ("crates/fs-rep-mesh/src/winding.rs", "563a937992d4b3ca9c2e56d6e494d620e71fb72db622ce67e8ea694d941f55cc"),
    "shapes": ("crates/fs-rep-mesh/src/shapes.rs", "2856e21467392176d581a33b56084fd1c9d0c1777ca8c477bc778c63049ea1c7"),
    "fs_geom": ("crates/fs-geom/src/lib.rs", "534066ae3ab0b6eba2970a7d540497327dbb274ca3577b11add040c7b88dd232"),
}

# 1-based, inclusive: "/// A point in ℝ³." through the closing brace of `impl Vec3`.
POINT_VEC_LINES = (86, 165)

WORKSPACE_TOML = """[workspace]
members = ["fs-geom", "oracle"]
resolver = "3"
"""

FS_GEOM_TOML = """[package]
name = "fs-geom"
version = "0.0.0"
edition = "2024"
publish = false
"""

ORACLE_TOML = """[package]
name = "winding-oracle"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
fs-geom = { path = "../fs-geom" }
"""


def git_show(checkout, path):
    return subprocess.run(
        ["git", "-C", checkout, "show", f"{COMMIT}:{path}"], check=True, capture_output=True, timeout=60
    ).stdout


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def write(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "xb") as f:
        f.write(data)


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    checkout, out = sys.argv[1], sys.argv[2]
    if os.path.exists(out) and os.listdir(out):
        sys.exit(f"{out} is not empty")

    sources = {}
    for name, (path, want) in UPSTREAM.items():
        data = git_show(checkout, path)
        got = sha256(data)
        if got != want:
            sys.exit(f"{path} @ {COMMIT}: sha256 {got}, expected {want}")
        sources[name] = data
    with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "driver.rs"), "rb") as f:
        driver = f.read()

    lo, hi = POINT_VEC_LINES
    lines = sources["fs_geom"].split(b"\n")
    excerpt = b"\n".join(lines[lo - 1 : hi]) + b"\n"
    if not (lines[lo - 1].startswith(b"/// A point in") and lines[hi - 1] == b"}"):
        sys.exit("fs-geom lib.rs: the Point3/Vec3 line range no longer brackets the two types")
    header = f"//! fs-geom src/lib.rs lines {lo}-{hi} @ {COMMIT} (Point3 and Vec3 only).\n\n".encode()

    write(os.path.join(out, "Cargo.toml"), WORKSPACE_TOML.encode())
    write(os.path.join(out, "fs-geom", "Cargo.toml"), FS_GEOM_TOML.encode())
    write(os.path.join(out, "fs-geom", "src", "lib.rs"), header + excerpt)
    write(os.path.join(out, "oracle", "Cargo.toml"), ORACLE_TOML.encode())
    write(os.path.join(out, "oracle", "src", "main.rs"), driver)
    write(os.path.join(out, "oracle", "src", "upstream", "winding.rs"), sources["winding"])
    write(os.path.join(out, "oracle", "src", "upstream", "shapes.rs"), sources["shapes"])

    digests = [(sha256(sources[n]), UPSTREAM[n][0]) for n in UPSTREAM] + [(sha256(driver), "driver.rs")]
    listing = "".join(f"sha256:{h} {p}\n" for h, p in digests)
    print(listing, end="", file=sys.stderr)
    print(f"inputs_digest sha256:{sha256(listing.encode())}", file=sys.stderr)


if __name__ == "__main__":
    main()
