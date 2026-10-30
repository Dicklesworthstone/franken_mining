//! Provenance: frankensim crates/fs-blake3/src/lib.rs @ 20463525d9775a18c1b237d2676f364f367175ab; licence MIT; Changes: added SHA-256 and MMR parent helper for FMT-LEDGER-1 chains; Left out: fs-blake3::identity submodule
//! Determinism: BitwisePortable

use core::fmt;

/// 32-byte cryptographic digest (BLAKE3 or SHA-256) (`BD-018`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Digest32(pub [u8; 32]);

impl Digest32 {
    /// Borrow the underlying 32 bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Encode as 64 lowercase hexadecimal characters.
    #[must_use]
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(64);
        for b in &self.0 {
            use fmt::Write as _;
            let _ = write!(out, "{b:02x}");
        }
        out
    }
}

impl fmt::Display for Digest32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for b in &self.0 {
            write!(f, "{b:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest32({self})")
    }
}

const IV: [u32; 8] = [
    0x6A09_E667,
    0xBB67_AE85,
    0x3C6E_F372,
    0xA54F_F53A,
    0x510E_527F,
    0x9B05_688C,
    0x1F83_D9AB,
    0x5BE0_CD19,
];

const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

const BLOCK_LEN: usize = 64;
const CHUNK_LEN: usize = 1024;

const CHUNK_START: u32 = 1 << 0;
const CHUNK_END: u32 = 1 << 1;
const PARENT: u32 = 1 << 2;
const ROOT: u32 = 1 << 3;

fn g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

fn round(state: &mut [u32; 16], m: &[u32; 16]) {
    g(state, 0, 4, 8, 12, m[0], m[1]);
    g(state, 1, 5, 9, 13, m[2], m[3]);
    g(state, 2, 6, 10, 14, m[4], m[5]);
    g(state, 3, 7, 11, 15, m[6], m[7]);
    g(state, 0, 5, 10, 15, m[8], m[9]);
    g(state, 1, 6, 11, 12, m[10], m[11]);
    g(state, 2, 7, 8, 13, m[12], m[13]);
    g(state, 3, 4, 9, 14, m[14], m[15]);
}

fn permute(m: &mut [u32; 16]) {
    let mut permuted = [0u32; 16];
    for (dst, &src) in permuted.iter_mut().zip(MSG_PERMUTATION.iter()) {
        *dst = m[src];
    }
    *m = permuted;
}

fn compress(
    chaining_value: &[u32; 8],
    block_words: &[u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
) -> [u32; 16] {
    let mut state = [
        chaining_value[0],
        chaining_value[1],
        chaining_value[2],
        chaining_value[3],
        chaining_value[4],
        chaining_value[5],
        chaining_value[6],
        chaining_value[7],
        IV[0],
        IV[1],
        IV[2],
        IV[3],
        counter as u32,
        (counter >> 32) as u32,
        block_len,
        flags,
    ];
    let mut block = *block_words;
    for r in 0..7 {
        if r > 0 {
            permute(&mut block);
        }
        round(&mut state, &block);
    }
    for i in 0..8 {
        state[i] ^= state[i + 8];
        state[i + 8] ^= chaining_value[i];
    }
    state
}

fn words_from_block(block: &[u8; BLOCK_LEN]) -> [u32; 16] {
    let mut words = [0u32; 16];
    for (i, word) in words.iter_mut().enumerate() {
        let off = i * 4;
        *word = u32::from_le_bytes([block[off], block[off + 1], block[off + 2], block[off + 3]]);
    }
    words
}

fn first_8_words(compression_output: [u32; 16]) -> [u32; 8] {
    let mut cv = [0u32; 8];
    cv.copy_from_slice(&compression_output[..8]);
    cv
}

struct Output {
    input_chaining_value: [u32; 8],
    block_words: [u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
}

impl Output {
    fn chaining_value(&self) -> [u32; 8] {
        first_8_words(compress(
            &self.input_chaining_value,
            &self.block_words,
            self.counter,
            self.block_len,
            self.flags,
        ))
    }

    fn root_hash(&self) -> [u8; 32] {
        let words = compress(
            &self.input_chaining_value,
            &self.block_words,
            0,
            self.block_len,
            self.flags | ROOT,
        );
        let mut bytes = [0u8; 32];
        for (i, word) in words[..8].iter().enumerate() {
            let off = i * 4;
            bytes[off..off + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
}

fn parent_output(
    key_words: [u32; 8],
    flags: u32,
    left_child_cv: [u32; 8],
    right_child_cv: [u32; 8],
) -> Output {
    let mut block_words = [0u32; 16];
    block_words[..8].copy_from_slice(&left_child_cv);
    block_words[8..].copy_from_slice(&right_child_cv);
    Output {
        input_chaining_value: key_words,
        block_words,
        counter: 0,
        block_len: BLOCK_LEN as u32,
        flags: flags | PARENT,
    }
}

struct ChunkState {
    chaining_value: [u32; 8],
    chunk_counter: u64,
    block: [u8; BLOCK_LEN],
    block_len: u8,
    blocks_compressed: u8,
    flags: u32,
}

impl ChunkState {
    fn new(key_words: [u32; 8], chunk_counter: u64, flags: u32) -> Self {
        Self {
            chaining_value: key_words,
            chunk_counter,
            block: [0; BLOCK_LEN],
            block_len: 0,
            blocks_compressed: 0,
            flags,
        }
    }

    fn len(&self) -> usize {
        BLOCK_LEN * self.blocks_compressed as usize + self.block_len as usize
    }

    fn start_flag(&self) -> u32 {
        if self.blocks_compressed == 0 {
            self.flags | CHUNK_START
        } else {
            self.flags
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.block_len as usize == BLOCK_LEN {
                let block_words = words_from_block(&self.block);
                self.chaining_value = first_8_words(compress(
                    &self.chaining_value,
                    &block_words,
                    self.chunk_counter,
                    BLOCK_LEN as u32,
                    self.start_flag(),
                ));
                self.blocks_compressed += 1;
                self.block = [0; BLOCK_LEN];
                self.block_len = 0;
            }
            let want = BLOCK_LEN - self.block_len as usize;
            let take = want.min(input.len());
            self.block[self.block_len as usize..self.block_len as usize + take]
                .copy_from_slice(&input[..take]);
            self.block_len += take as u8;
            input = &input[take..];
        }
    }

    fn output(&self) -> Output {
        Output {
            input_chaining_value: self.chaining_value,
            block_words: words_from_block(&self.block),
            counter: self.chunk_counter,
            block_len: u32::from(self.block_len),
            flags: self.start_flag() | CHUNK_END,
        }
    }
}

const MAX_DEPTH: usize = 54;

/// Streaming BLAKE3 hasher (`BD-018`).
pub struct Blake3 {
    key_words: [u32; 8],
    flags: u32,
    chunk_state: ChunkState,
    cv_stack: [[u32; 8]; MAX_DEPTH],
    cv_stack_len: u8,
}

impl Default for Blake3 {
    fn default() -> Self {
        Self::new()
    }
}

impl Blake3 {
    /// Construct a new BLAKE3 hasher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            key_words: IV,
            flags: 0,
            chunk_state: ChunkState::new(IV, 0, 0),
            cv_stack: [[0; 8]; MAX_DEPTH],
            cv_stack_len: 0,
        }
    }

    fn push_cv(&mut self, cv: [u32; 8]) {
        self.cv_stack[self.cv_stack_len as usize] = cv;
        self.cv_stack_len += 1;
    }

    fn pop_cv(&mut self) -> [u32; 8] {
        self.cv_stack_len -= 1;
        self.cv_stack[self.cv_stack_len as usize]
    }

    fn add_chunk_chaining_value(&mut self, mut new_cv: [u32; 8], mut total_chunks: u64) {
        while total_chunks & 1 == 0 {
            new_cv =
                parent_output(self.key_words, self.flags, self.pop_cv(), new_cv).chaining_value();
            total_chunks >>= 1;
        }
        self.push_cv(new_cv);
    }

    /// Feed input bytes into the streaming hasher.
    pub fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            if self.chunk_state.len() == CHUNK_LEN {
                let chunk_cv = self.chunk_state.output().chaining_value();
                let total_chunks = self.chunk_state.chunk_counter + 1;
                self.add_chunk_chaining_value(chunk_cv, total_chunks);
                self.chunk_state = ChunkState::new(self.key_words, total_chunks, self.flags);
            }
            let want = CHUNK_LEN - self.chunk_state.len();
            let take = want.min(input.len());
            self.chunk_state.update(&input[..take]);
            input = &input[take..];
        }
    }

    /// Finalize and return the 32-byte BLAKE3 root digest.
    #[must_use]
    pub fn finalize(&self) -> Digest32 {
        let mut output = self.chunk_state.output();
        let mut parent_nodes_remaining = self.cv_stack_len as usize;
        while parent_nodes_remaining > 0 {
            parent_nodes_remaining -= 1;
            output = parent_output(
                self.key_words,
                self.flags,
                self.cv_stack[parent_nodes_remaining],
                output.chaining_value(),
            );
        }
        Digest32(output.root_hash())
    }
}

/// Compute one-shot BLAKE3 digest (`BD-018`).
#[must_use]
pub fn blake3_hash(input: &[u8]) -> Digest32 {
    let mut h = Blake3::new();
    h.update(input);
    h.finalize()
}

/// Pure safe-Rust SHA-256 (FIPS 180-4) for `FMT-LEDGER-1` hash chains and MMR peaks (`BD-018`, `§26.5`).
#[must_use]
pub fn sha256(input: &[u8]) -> Digest32 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state = IV;
    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = input.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    // `padded` is a whole number of 64-byte blocks, so the remainder of `as_chunks` is empty.
    for chunk in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (word, bytes) in w.iter_mut().zip(chunk.as_chunks::<4>().0) {
            *word = u32::from_be_bytes(*bytes);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g_w = state[6];
        let mut h = state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g_w);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g_w;
            g_w = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g_w);
        state[7] = state[7].wrapping_add(h);
    }

    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        let off = i * 4;
        out[off..off + 4].copy_from_slice(&word.to_be_bytes());
    }
    Digest32(out)
}

/// Domain-separated SHA-256 Merkle Mountain Range (`MMR`) parent hash (`§26.9 A2`).
#[must_use]
pub fn mmr_parent(left: &Digest32, right: &Digest32) -> Digest32 {
    let mut buf = [0u8; 65];
    buf[0] = 0x01; // internal node domain separator
    buf[1..33].copy_from_slice(&left.0);
    buf[33..65].copy_from_slice(&right.0);
    sha256(&buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_official_blake3_vectors() {
        // Official BLAKE3 test vectors for lengths 0, 1, 64, 65, 1024, 1025
        // where input[i] = (i % 251) as u8.
        let make_input = |len: usize| -> Vec<u8> { (0..len).map(|i| (i % 251) as u8).collect() };
        assert_eq!(
            blake3_hash(&make_input(0)).to_hex(),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(
            blake3_hash(&make_input(1)).to_hex(),
            "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213"
        );
        assert_eq!(
            blake3_hash(&make_input(64)).to_hex(),
            "4eed7141ea4a5cd4b788606bd23f46e212af9cacebacdc7d1f4c6dc7f2511b98"
        );
        assert_eq!(
            blake3_hash(&make_input(1025)).to_hex(),
            "d00278ae47eb27b34faecf67b4fe263f82d5412916c1ffd97c8cb7fb814b8444"
        );
    }

    #[test]
    fn test_official_sha256_vectors() {
        assert_eq!(
            sha256(b"").to_hex(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc").to_hex(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
