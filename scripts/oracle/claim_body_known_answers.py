"""Known answers for `Claim::canonical_body` and its `clm_` id (bd-040), built from the specs, not from fmine code.

The body is the `CLM1` frame, schema version 2, of fmine-core's canonical codec (BD-017):
  "FMNE" | tag "CLM1" | version u16 LE | fields | CRC32C (Castagnoli, reflected 0x82F63B78) of all prior bytes, u32 LE.
A string is a u32 LE byte length, then its UTF-8 bytes. The fields, in order:
  metric_key str | value tag str | evidence [variant, payload] | epistemic 18 bytes | knowledge state u8 |
  label (public 0, internal 1, else 5, u64 venture count, each venture str, u64 wall count, each wall str) |
  u64 parent count, each parent's 16 id bytes | u64 anchor count, each: path str, locator str, sha256 32 bytes.
The id is "clm_" followed by the first 16 bytes of BLAKE3("clm_" + body), in lower-case hex.

BLAKE3 here is a plain port of the reference implementation's hash mode. It is checked against the official
vectors (input[i] = i % 251) before any answer is printed. Run: python3 -I scripts/oracle/claim_body_known_answers.py
"""

import struct

IV = [0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19]
PERM = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
CHUNK_START, CHUNK_END, PARENT, ROOT = 1, 2, 4, 8
M32 = 0xFFFFFFFF


def rotr(x, n):
    return ((x >> n) | (x << (32 - n))) & M32


def g(s, a, b, c, d, x, y):
    s[a] = (s[a] + s[b] + x) & M32
    s[d] = rotr(s[d] ^ s[a], 16)
    s[c] = (s[c] + s[d]) & M32
    s[b] = rotr(s[b] ^ s[c], 12)
    s[a] = (s[a] + s[b] + y) & M32
    s[d] = rotr(s[d] ^ s[a], 8)
    s[c] = (s[c] + s[d]) & M32
    s[b] = rotr(s[b] ^ s[c], 7)


def compress(cv, block_words, counter, block_len, flags):
    s = list(cv) + IV[:4] + [counter & M32, (counter >> 32) & M32, block_len, flags]
    m = list(block_words)
    for r in range(7):
        g(s, 0, 4, 8, 12, m[0], m[1])
        g(s, 1, 5, 9, 13, m[2], m[3])
        g(s, 2, 6, 10, 14, m[4], m[5])
        g(s, 3, 7, 11, 15, m[6], m[7])
        g(s, 0, 5, 10, 15, m[8], m[9])
        g(s, 1, 6, 11, 12, m[10], m[11])
        g(s, 2, 7, 8, 13, m[12], m[13])
        g(s, 3, 4, 9, 14, m[14], m[15])
        if r < 6:
            m = [m[i] for i in PERM]
    for i in range(8):
        s[i] ^= s[i + 8]
        s[i + 8] ^= cv[i]
    return s


def words(block):
    return list(struct.unpack("<16I", block.ljust(64, b"\0")))


def chunk_output(cv, chunk, counter):
    """The last block's compression inputs, for a chunk of at most 1024 bytes (one block for an empty chunk)."""
    blocks = [chunk[i:i + 64] for i in range(0, len(chunk), 64)] or [b""]
    for i, block in enumerate(blocks[:-1]):
        flags = CHUNK_START if i == 0 else 0
        cv = compress(cv, words(block), counter, 64, flags)[:8]
    last = blocks[-1]
    flags = CHUNK_END | (CHUNK_START if len(blocks) == 1 else 0)
    return cv, words(last), counter, len(last), flags


def blake3(data):
    chunks = [data[i:i + 1024] for i in range(0, len(data), 1024)] or [b""]
    stack = []
    for n, chunk in enumerate(chunks[:-1]):
        out = chunk_output(IV, chunk, n)
        cv = compress(*out)[:8]
        total = n + 1
        while total % 2 == 0:
            left = stack.pop()
            cv = compress(IV, left + cv, 0, 64, PARENT)[:8]
            total //= 2
        stack.append(cv)
    out = chunk_output(IV, chunks[-1], len(chunks) - 1)
    while stack:
        cv = compress(*out)[:8]
        out = (IV, stack.pop() + cv, 0, 64, PARENT)
    cv, block, counter, block_len, flags = out
    return struct.pack("<8I", *compress(cv, block, counter, block_len, flags | ROOT)[:8])


def crc32c(data):
    crc = 0xFFFFFFFF
    for b in data:
        crc ^= b
        for _ in range(8):
            crc = (crc >> 1) ^ (0x82F63B78 & -(crc & 1))
    return crc ^ 0xFFFFFFFF


def s(text):
    raw = text.encode("utf-8")
    return struct.pack("<I", len(raw)) + raw


def u64(n):
    return struct.pack("<Q", n)


def body(metric_key, tag, evidence, epistemic, state, label, parents, anchors):
    out = b"FMNE" + b"CLM1" + struct.pack("<H", 2)
    out += s(metric_key) + s(tag) + bytes(evidence) + bytes(epistemic) + bytes([state]) + label
    out += u64(len(parents)) + b"".join(bytes.fromhex(p) for p in parents)
    out += u64(len(anchors)) + b"".join(s(path) + s(loc) + sha for path, loc, sha in anchors)
    return out + struct.pack("<I", crc32c(out))


def claim_id(frame):
    return "clm_" + blake3(b"clm_" + frame)[:16].hex()


OFFICIAL = {
    0: "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
    1: "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213",
    64: "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98",
    1025: "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444",
}


def main():
    for n, want in OFFICIAL.items():
        got = blake3(bytes(i % 251 for i in range(n))).hex()
        if got != want:
            raise SystemExit(f"BLAKE3 port fails the official vector for length {n}: {got}")
    if crc32c(b"123456789") != 0xE3069283:
        raise SystemExit("CRC32C fails its check value")

    # A: an issuer's figure, public, one anchor. Assertion{Issuer} = [7, 2]; KnownKnown = 18 zero bytes; Known = 0.
    a = body("aisc_usd_per_oz", "1234.5", [7, 2], [0] * 18, 0, bytes([0]), [],
             [("synthetic/aisc.xlsx", "Synthetic AISC!L14", bytes([0x11]) * 32)])
    # B: derived, venture ven_a joined with wall W-1, two parents, no anchor. Derived = [1, 0];
    # KnownUnknown(Parametric) = [2, 0 x 17]; Estimated = 1.
    label = bytes([5]) + u64(1) + s("ven_a") + u64(1) + s("W-1")
    b = body("net_aisc_usd_m", "-12.25", [1, 0], [2] + [0] * 17, 1, label,
             ["00112233445566778899aabbccddeeff", "ffeeddccbbaa99887766554433221100"], [])
    for name, frame in (("A", a), ("B", b)):
        print(f"{name} body {frame.hex()}")
        print(f"{name} id {claim_id(frame)}")


if __name__ == "__main__":
    main()
