//! Pure Rust Xoroshiro128++ & MD5 NameHash matching SteelMC / vanilla Minecraft 1.21

const GOLDEN_RATIO_64: u64 = 0x9E37_79B9_7F4A_7C15;
const SILVER_RATIO_64: u64 = 0x6A09_E667_F3BC_C909;

pub trait Random {
    fn next_f64(&mut self) -> f64;
    fn next_f32(&mut self) -> f32;
    fn next_i32_bounded(&mut self, bound: i32) -> i32;
    fn next_random(&mut self) -> u64;
    fn next_i64(&mut self) -> i64;
}

pub trait PositionalRandom {
    type Source: Random;
    fn at(&self, x: i32, y: i32, z: i32) -> Self::Source;
    fn from_hash_of(&self, name: &str) -> Self::Source;
    fn with_hash_of(&self, hash: &NameHash) -> Self::Source;
}

pub type RandomSplitter = XoroshiroSplitter;

#[derive(Clone, Debug)]
pub enum RandomSource {
    Xoroshiro(Xoroshiro),
    Legacy(LegacyRandom),
}

impl Random for RandomSource {
    #[inline]
    fn next_f64(&mut self) -> f64 {
        match self {
            Self::Xoroshiro(x) => x.next_f64(),
            Self::Legacy(l) => l.next_f64(),
        }
    }
    #[inline]
    fn next_f32(&mut self) -> f32 {
        match self {
            Self::Xoroshiro(x) => x.next_f32(),
            Self::Legacy(l) => l.next_f32(),
        }
    }
    #[inline]
    fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        match self {
            Self::Xoroshiro(x) => x.next_i32_bounded(bound),
            Self::Legacy(l) => l.next_i32_bounded(bound),
        }
    }
    #[inline]
    fn next_random(&mut self) -> u64 {
        match self {
            Self::Xoroshiro(x) => x.next_random(),
            Self::Legacy(l) => l.next_random(),
        }
    }
    #[inline]
    fn next_i64(&mut self) -> i64 {
        match self {
            Self::Xoroshiro(x) => x.next_i64(),
            Self::Legacy(l) => l.next_i64(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Xoroshiro {
    pub seed_lo: u64,
    pub seed_hi: u64,
}

#[derive(Clone, Debug)]
pub struct XoroshiroSplitter {
    pub seed_lo: u64,
    pub seed_hi: u64,
}

impl Xoroshiro {
    #[inline]
    pub const fn from_seed(seed: u64) -> Self {
        let (lo, hi) = Self::upgrade_seed_to_128_bit(seed);
        let lo = mix_stafford_13(lo);
        let hi = mix_stafford_13(hi);
        Self::new(lo, hi)
    }

    #[inline]
    pub const fn new(lo: u64, hi: u64) -> Self {
        let (lo, hi) = if (lo | hi) == 0 {
            (GOLDEN_RATIO_64, SILVER_RATIO_64)
        } else {
            (lo, hi)
        };
        Self {
            seed_lo: lo,
            seed_hi: hi,
        }
    }

    #[inline]
    const fn upgrade_seed_to_128_bit(seed: u64) -> (u64, u64) {
        let lo = seed ^ SILVER_RATIO_64;
        let hi = lo.wrapping_add(GOLDEN_RATIO_64);
        (lo, hi)
    }

    #[inline]
    pub fn next(&mut self, bits: u64) -> u64 {
        self.next_random() >> (64 - bits)
    }

    #[inline]
    pub fn next_random(&mut self) -> u64 {
        let l = self.seed_lo;
        let m = self.seed_hi;
        let n = l.wrapping_add(m).rotate_left(17).wrapping_add(l);
        let m = m ^ l;
        self.seed_lo = l.rotate_left(49) ^ m ^ (m << 21);
        self.seed_hi = m.rotate_left(28);
        n
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        self.next(53) as f64 * (1.1102230246251565e-16_f64)
    }

    #[inline]
    pub fn next_i64(&mut self) -> i64 {
        self.next_random() as i64
    }

    #[inline]
    pub fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }
        let bound = bound as u64;
        let mut r = self.next(31);
        let mut m = r % bound;
        while r.wrapping_sub(m).wrapping_add(bound - 1) < r {
            r = self.next(31);
            m = r % bound;
        }
        m as i32
    }

    #[inline]
    pub fn consume_count(&mut self, count: usize) {
        for _ in 0..count {
            self.next_random();
        }
    }

    #[inline]
    pub fn next_positional(&mut self) -> XoroshiroSplitter {
        XoroshiroSplitter {
            seed_lo: self.next_random(),
            seed_hi: self.next_random(),
        }
    }
}

impl Random for Xoroshiro {
    #[inline]
    fn next_f64(&mut self) -> f64 {
        self.next_f64()
    }
    #[inline]
    fn next_f32(&mut self) -> f32 {
        self.next_f64() as f32
    }
    #[inline]
    fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        self.next_i32_bounded(bound)
    }
    #[inline]
    fn next_random(&mut self) -> u64 {
        self.next_random()
    }
    #[inline]
    fn next_i64(&mut self) -> i64 {
        self.next_i64()
    }
}

impl XoroshiroSplitter {
    #[inline]
    pub fn from_seed(seed: u64) -> Self {
        let mut rng = Xoroshiro::from_seed(seed);
        rng.next_positional()
    }

    #[inline]
    pub fn at(&self, x: i32, y: i32, z: i32) -> Xoroshiro {
        let h = get_seed(x, y, z);
        let lo = h ^ self.seed_lo;
        Xoroshiro::new(lo, self.seed_hi)
    }

    #[inline]
    pub fn from_hash_of(&self, name: &str) -> Xoroshiro {
        let md5 = md5_digest(name.as_bytes());
        let lo = u64::from_le_bytes(md5[0..8].try_into().unwrap()) ^ self.seed_lo;
        let hi = u64::from_le_bytes(md5[8..16].try_into().unwrap()) ^ self.seed_hi;
        Xoroshiro::new(lo, hi)
    }

    #[inline]
    pub fn with_hash_of(&self, hash: &NameHash) -> Xoroshiro {
        let lo = hash.hash_lo ^ self.seed_lo;
        let hi = hash.hash_hi ^ self.seed_hi;
        Xoroshiro::new(lo, hi)
    }

    #[inline]
    pub fn next_positional(&mut self) -> Self {
        let mut rng = Xoroshiro::new(self.seed_lo, self.seed_hi);
        let next = rng.next_positional();
        self.seed_lo = rng.seed_lo;
        self.seed_hi = rng.seed_hi;
        next
    }
}

impl PositionalRandom for XoroshiroSplitter {
    type Source = Xoroshiro;

    #[inline]
    fn at(&self, x: i32, y: i32, z: i32) -> Self::Source {
        self.at(x, y, z)
    }

    #[inline]
    fn from_hash_of(&self, name: &str) -> Self::Source {
        self.from_hash_of(name)
    }

    #[inline]
    fn with_hash_of(&self, hash: &NameHash) -> Self::Source {
        self.with_hash_of(hash)
    }
}

const LCG_A: u64 = 0x0005_DEEC_E66D;
const LCG_MASK: u64 = 0xFFFF_FFFF_FFFF;

#[derive(Clone, Debug)]
pub struct LegacyRandom {
    pub seed: i64,
}

impl LegacyRandom {
    #[inline]
    pub const fn from_seed(seed: u64) -> Self {
        Self {
            seed: (seed as i64 ^ LCG_A as i64) & LCG_MASK as i64,
        }
    }

    #[inline]
    pub const fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ 0x0005_DEEC_E66D) & 0xFFFF_FFFF_FFFF;
    }

    #[inline]
    pub fn set_large_feature_seed(&mut self, seed: i64, chunk_x: i32, chunk_z: i32) {
        self.set_seed(seed);
        let x_mul = self.next_i64();
        let z_mul = self.next_i64();
        self.set_seed(
            i64::from(chunk_x).wrapping_mul(x_mul) ^ i64::from(chunk_z).wrapping_mul(z_mul) ^ seed,
        );
    }

    #[inline]
    pub fn next(&mut self, bits: u64) -> i32 {
        self.seed = (self.seed.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & LCG_MASK as i64;
        ((self.seed as u64) >> (48 - bits)) as i32
    }

    #[inline]
    pub fn next_i32(&mut self) -> i32 {
        self.next(32)
    }

    #[inline]
    pub fn next_f32(&mut self) -> f32 {
        self.next(24) as f32 * 5.960_464_5e-8_f32
    }

    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        let combined = (i64::from(self.next(26)) << 27) + i64::from(self.next(27));
        combined as f64 * (1.0 / (1_i64 << 53) as f64)
    }

    #[inline]
    pub fn next_i64(&mut self) -> i64 {
        let i = self.next_i32();
        let j = self.next_i32();
        (i64::from(i) << 32).wrapping_add(i64::from(j))
    }

    #[inline]
    pub fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }
        if bound & bound.wrapping_sub(1) == 0 {
            (i64::from(bound).wrapping_mul(i64::from(self.next(31))) >> 31) as i32
        } else {
            loop {
                let i = self.next(31);
                let j = i % bound;
                if i.wrapping_sub(j).wrapping_add(bound.wrapping_sub(1)) >= 0 {
                    return j;
                }
            }
        }
    }

    #[inline]
    pub fn next_random(&mut self) -> u64 {
        self.next(32) as u32 as u64
    }

    #[inline]
    pub fn next_bool(&mut self) -> bool {
        self.next(1) != 0
    }
}

impl Random for LegacyRandom {
    #[inline]
    fn next_f64(&mut self) -> f64 {
        self.next_f64()
    }
    #[inline]
    fn next_f32(&mut self) -> f32 {
        self.next_f32()
    }
    #[inline]
    fn next_i32_bounded(&mut self, bound: i32) -> i32 {
        self.next_i32_bounded(bound)
    }
    #[inline]
    fn next_random(&mut self) -> u64 {
        self.next_random()
    }
    #[inline]
    fn next_i64(&mut self) -> i64 {
        self.next_i64()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NameHash {
    pub hash_lo: u64,
    pub hash_hi: u64,
}

impl NameHash {
    pub const fn new(name: &str) -> Self {
        let d = md5_digest(name.as_bytes());
        let lo = u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6], d[7]]);
        let hi = u64::from_le_bytes([d[8], d[9], d[10], d[11], d[12], d[13], d[14], d[15]]);
        Self {
            hash_lo: lo,
            hash_hi: hi,
        }
    }
}

pub mod name_hash {
    pub use super::NameHash;
}

pub mod legacy_random {
    pub use super::LegacyRandom;
}

#[inline(always)]
const fn mix_stafford_13(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[inline(always)]
fn get_seed(x: i32, y: i32, z: i32) -> u64 {
    let mut l = (x as i64) as u64;
    l = l.wrapping_mul(3129871);
    let mut m = (z as i64) as u64;
    l = m.wrapping_mul(l).wrapping_add(m);
    m = (y as i64) as u64;
    l = m.wrapping_mul(l).wrapping_add(m);
    l = m.wrapping_mul(l).wrapping_add(m);
    l
}

const fn md5_digest(data: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    const T: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    let mut block = [0u8; 64];
    let mut i = 0;
    while i < data.len() && i < 55 {
        block[i] = data[i];
        i += 1;
    }
    block[i] = 0x80;
    let bit_len = (data.len() as u64) * 8;
    let len_bytes = bit_len.to_le_bytes();
    i = 0;
    while i < 8 {
        block[56 + i] = len_bytes[i];
        i += 1;
    }

    let mut m = [0u32; 16];
    i = 0;
    while i < 16 {
        m[i] = u32::from_le_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
        i += 1;
    }

    let mut a: u32 = 0x67452301;
    let mut b: u32 = 0xefcdab89;
    let mut c: u32 = 0x98badcfe;
    let mut d: u32 = 0x10325476;

    i = 0;
    while i < 64 {
        let (f, g) = if i < 16 {
            ((b & c) | (!b & d), i)
        } else if i < 32 {
            ((d & b) | (!d & c), (5 * i + 1) % 16)
        } else if i < 48 {
            (b ^ c ^ d, (3 * i + 5) % 16)
        } else {
            (c ^ (b | !d), (7 * i) % 16)
        };

        let temp = d;
        d = c;
        c = b;
        let x = a.wrapping_add(f).wrapping_add(T[i]).wrapping_add(m[g]);
        b = b.wrapping_add(x.rotate_left(S[i]));
        a = temp;

        i += 1;
    }

    a = a.wrapping_add(0x67452301);
    b = b.wrapping_add(0xefcdab89);
    c = c.wrapping_add(0x98badcfe);
    d = d.wrapping_add(0x10325476);

    let ab = a.to_le_bytes();
    let bb = b.to_le_bytes();
    let cb = c.to_le_bytes();
    let db = d.to_le_bytes();
    [
        ab[0], ab[1], ab[2], ab[3], bb[0], bb[1], bb[2], bb[3],
        cb[0], cb[1], cb[2], cb[3], db[0], db[1], db[2], db[3],
    ]
}
