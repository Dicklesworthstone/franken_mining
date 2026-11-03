"""The rows where upstream's winding answers differ between two hosts (bd-192).

Both inputs are the driver's output (see assemble.py), run on different hosts. The soups and query points must be
identical; only the answers, which go through each host's std `atan2`, may differ. Prints one line per differing
query, `q <soup> <query index within the soup> <exact> <beta1> <beta2> <beta4>`, with the second host's answers.

Usage: python3 -I scripts/oracle/winding_corpus/host_delta.py <base corpus> <other host's corpus>
"""

import sys


def rows(path):
    out, soup, k = [], None, 0
    with open(path, encoding="ascii") as f:
        for line in f:
            line = line.rstrip("\n")
            if line.startswith("soup "):
                soup, k = line.split(" ")[1], 0
                out.append(("soup", line))
            elif line.startswith(("p ", "t ")):
                out.append(("geom", line))
            elif line.startswith("q "):
                fields = line.split(" ")[1:]
                out.append(("q", (soup, k, fields[:3], fields[3:])))
                k += 1
    return out


def main():
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    base, other = rows(sys.argv[1]), rows(sys.argv[2])
    if len(base) != len(other):
        sys.exit("the corpora have different lengths")
    print("# Upstream winding answers on a second host, where they differ from winding_corpus.txt (bd-192).")
    print("# q <soup> <query index> <exact> <beta1> <beta2> <beta4>; floats are 16 hex digits of IEEE-754 bits.")
    for (kind, a), (kind2, b) in zip(base, other):
        if kind != kind2:
            sys.exit("the corpora have different structure")
        if kind != "q":
            if a != b:
                sys.exit(f"the corpora differ outside the answers: {a!r} / {b!r}")
            continue
        (soup, k, point, answers), (soup2, k2, point2, answers2) = a, b
        if (soup, k, point) != (soup2, k2, point2):
            sys.exit(f"query {soup} {k} differs between the corpora")
        if answers != answers2:
            print(f"q {soup} {k} {' '.join(answers2)}")


if __name__ == "__main__":
    main()
