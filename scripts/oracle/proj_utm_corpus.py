"""PROJ's UTM coordinates for fmine-crs's PO-CRS-1 corpus (bd-035).

For each projected CRS in registries/crs.toml, 10,000 WGS 84 points over the zone's area of use (its 6-degree
strip, latitudes 0 to 84 north or -80 to 0 south) go through PROJ (pyproj's Transformer from EPSG:4326 to the
EPSG code, always_xy) and the output is printed as `easting northing` in metres to 0.1 mm.

The points are not stored. Each comes from a 64-bit LCG (Knuth's MMIX constants) seeded with the EPSG code:
latitude and longitude are whole microdegrees, `lo + (state >> 11) % (hi - lo + 1)` per draw, latitude first,
divided by 1e6 to the nearest double. fmine-crs's test draws the same points. The inputs digest is the sha256
of every point as `<lat bits> <lon bits>`, 16 hex digits each, one line per point, all CRSs in registry order;
the test recomputes it.

Usage: <python with pyproj> -I scripts/oracle/proj_utm_corpus.py <registries/crs.toml>
stdout is the fixture; stderr gets the inputs digest and the PROJ pipeline of each CRS.
"""

import hashlib
import struct
import sys
import tomllib

import pyproj

POINTS = 10_000
MASK = (1 << 64) - 1


class Lcg:
    def __init__(self, seed):
        self.state = seed & MASK

    def next(self):
        self.state = (self.state * 6364136223846793005 + 1442695040888963407) & MASK
        return self.state

    def micro(self, lo, hi):
        return lo + (self.next() >> 11) % (hi - lo + 1)


def points(epsg, zone, north):
    rng = Lcg(epsg)
    cm = (zone - 1) * 6 - 180 + 3
    lat_lo, lat_hi = (0, 84_000_000) if north else (-80_000_000, 0)
    lon_lo, lon_hi = (cm - 3) * 1_000_000, (cm + 3) * 1_000_000
    out = []
    for _ in range(POINTS):
        lat = rng.micro(lat_lo, lat_hi) / 1_000_000
        lon = rng.micro(lon_lo, lon_hi) / 1_000_000
        out.append((lat, lon))
    return out


def bits(x):
    return f"{struct.unpack('<Q', struct.pack('<d', x))[0]:016x}"


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    with open(sys.argv[1], "rb") as f:
        registry = tomllib.load(f)
    digest = hashlib.sha256()
    print("# PROJ's UTM coordinates for fmine-crs's PO-CRS-1 corpus (bd-035); see the capture record.")
    print("# crs <EPSG code> <zone> <N|S> <points>, then one `easting northing` line per point, in metres.")
    print(f"# pyproj {pyproj.__version__}, PROJ {pyproj.proj_version_str}")
    for row in registry["crs"]:
        if row["kind"] != "ProjectedUTM":
            continue
        epsg = int(row["code"].removeprefix("EPSG:"))
        zone, north = row["utm_zone"], row["northern"]
        t = pyproj.Transformer.from_crs("EPSG:4326", f"EPSG:{epsg}", always_xy=True)
        print(f"EPSG:{epsg} {t.definition}", file=sys.stderr)
        print(f"crs EPSG:{epsg} {zone} {'N' if north else 'S'} {POINTS}")
        for lat, lon in points(epsg, zone, north):
            digest.update(f"{bits(lat)} {bits(lon)}\n".encode())
            e, n = t.transform(lon, lat, errcheck=True)
            print(f"{e:.4f} {n:.4f}")
    print(f"inputs_digest sha256:{digest.hexdigest()}", file=sys.stderr)


if __name__ == "__main__":
    main()
