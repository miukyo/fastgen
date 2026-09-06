//! Core math utilities for world generation.

#[inline(always)]
pub fn clamp<T: PartialOrd>(val: T, min: T, max: T) -> T {
    if val < min {
        min
    } else if val > max {
        max
    } else {
        val
    }
}

#[inline(always)]
pub fn map(val: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    to_min + (val - from_min) * (to_max - to_min) / (from_max - from_min)
}

#[inline(always)]
pub fn map_clamped(val: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    if val <= from_min {
        to_min
    } else if val >= from_max {
        to_max
    } else {
        map(val, from_min, from_max, to_min, to_max)
    }
}

#[inline(always)]
pub fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[inline(always)]
pub fn clamped_lerp(a: f64, b: f64, t: f64) -> f64 {
    if t <= 0.0 {
        a
    } else if t >= 1.0 {
        b
    } else {
        lerp(t, a, b)
    }
}

#[inline(always)]
pub fn lerp2(u: f64, v: f64, x00: f64, x10: f64, x01: f64, x11: f64) -> f64 {
    lerp(v, lerp(u, x00, x10), lerp(u, x01, x11))
}

#[inline(always)]
pub fn lerp3(
    a1: f64,
    a2: f64,
    a3: f64,
    x000: f64,
    x100: f64,
    x010: f64,
    x110: f64,
    x001: f64,
    x101: f64,
    x011: f64,
    x111: f64,
) -> f64 {
    lerp(
        a3,
        lerp2(a1, a2, x000, x100, x010, x110),
        lerp2(a1, a2, x001, x101, x011, x111),
    )
}

const ROUND_OFF: f64 = 33_554_432.0;
const HALF_ROUND_OFF: f64 = ROUND_OFF / 2.0;

#[inline(always)]
pub fn wrap(x: f64) -> f64 {
    if (-HALF_ROUND_OFF..HALF_ROUND_OFF).contains(&x) {
        return x;
    }
    x - (x / ROUND_OFF + 0.5).floor() * ROUND_OFF
}

#[inline(always)]
pub fn fast_floor(v: f64) -> i32 {
    let i = v as i32;
    if v < f64::from(i) {
        i - 1
    } else {
        i
    }
}

#[inline(always)]
pub fn smoothstep(x: f64) -> f64 {
    x * x * x * (x * (x * 6.0 - 15.0) + 10.0)
}

pub mod trig {
    use std::sync::LazyLock;

    const INDEX_SCALE: f64 = 10_430.378_350_470_453;
    const TABLE_LEN: usize = 65_536;
    const TABLE_MASK: i64 = 0xFFFF;

    static SIN_TABLE: LazyLock<Box<[f32; TABLE_LEN]>> = LazyLock::new(|| {
        let mut table: Box<[f32; TABLE_LEN]> = vec![0.0_f32; TABLE_LEN]
            .into_boxed_slice()
            .try_into()
            .expect("65536-element vec");
        for (i, slot) in table.iter_mut().enumerate() {
            *slot = (i as f64 / INDEX_SCALE).sin() as f32;
        }
        table
    });

    #[inline]
    pub fn sin(angle: f64) -> f32 {
        let idx = (((angle * INDEX_SCALE) as i64) & TABLE_MASK) as usize;
        SIN_TABLE[idx]
    }

    #[inline]
    pub fn cos(angle: f64) -> f32 {
        let idx = (((angle * INDEX_SCALE + 16_384.0) as i64) & TABLE_MASK) as usize;
        SIN_TABLE[idx]
    }
}
