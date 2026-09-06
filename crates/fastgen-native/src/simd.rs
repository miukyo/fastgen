//! Portable 4-lane SIMD vector implementation for stable Rust.

use std::ops::{Add, BitAnd, BitOr, Div, Mul, Neg, Not, Sub};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct f64x4(pub [f64; 4]);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Mask4(pub [bool; 4]);

pub trait Select<T> {
    fn select(self, true_val: T, false_val: T) -> T;
}

pub trait SimdPartialOrd {
    type Mask;
    fn simd_lt(self, other: Self) -> Self::Mask;
    fn simd_le(self, other: Self) -> Self::Mask;
    fn simd_gt(self, other: Self) -> Self::Mask;
    fn simd_ge(self, other: Self) -> Self::Mask;
}

pub trait SimdFloat {
    fn simd_max(self, other: Self) -> Self;
    fn simd_min(self, other: Self) -> Self;
    fn abs(self) -> Self;
}

impl f64x4 {
    #[inline(always)]
    pub const fn splat(val: f64) -> Self {
        Self([val, val, val, val])
    }

    #[inline(always)]
    pub const fn from_array(arr: [f64; 4]) -> Self {
        Self(arr)
    }

    #[inline(always)]
    pub const fn to_array(self) -> [f64; 4] {
        self.0
    }

    #[inline(always)]
    pub fn from_slice(slice: &[f64]) -> Self {
        Self([slice[0], slice[1], slice[2], slice[3]])
    }
}

impl std::ops::Index<usize> for f64x4 {
    type Output = f64;
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl std::ops::IndexMut<usize> for f64x4 {
    #[inline(always)]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}

impl SimdPartialOrd for f64x4 {
    type Mask = Mask4;

    #[inline(always)]
    fn simd_lt(self, other: Self) -> Mask4 {
        Mask4([
            self.0[0] < other.0[0],
            self.0[1] < other.0[1],
            self.0[2] < other.0[2],
            self.0[3] < other.0[3],
        ])
    }

    #[inline(always)]
    fn simd_le(self, other: Self) -> Mask4 {
        Mask4([
            self.0[0] <= other.0[0],
            self.0[1] <= other.0[1],
            self.0[2] <= other.0[2],
            self.0[3] <= other.0[3],
        ])
    }

    #[inline(always)]
    fn simd_gt(self, other: Self) -> Mask4 {
        Mask4([
            self.0[0] > other.0[0],
            self.0[1] > other.0[1],
            self.0[2] > other.0[2],
            self.0[3] > other.0[3],
        ])
    }

    #[inline(always)]
    fn simd_ge(self, other: Self) -> Mask4 {
        Mask4([
            self.0[0] >= other.0[0],
            self.0[1] >= other.0[1],
            self.0[2] >= other.0[2],
            self.0[3] >= other.0[3],
        ])
    }
}

impl SimdFloat for f64x4 {
    #[inline(always)]
    fn simd_max(self, other: Self) -> Self {
        Self([
            self.0[0].max(other.0[0]),
            self.0[1].max(other.0[1]),
            self.0[2].max(other.0[2]),
            self.0[3].max(other.0[3]),
        ])
    }

    #[inline(always)]
    fn simd_min(self, other: Self) -> Self {
        Self([
            self.0[0].min(other.0[0]),
            self.0[1].min(other.0[1]),
            self.0[2].min(other.0[2]),
            self.0[3].min(other.0[3]),
        ])
    }

    #[inline(always)]
    fn abs(self) -> Self {
        Self([
            self.0[0].abs(),
            self.0[1].abs(),
            self.0[2].abs(),
            self.0[3].abs(),
        ])
    }
}

impl Mask4 {
    #[inline(always)]
    pub fn all(self) -> bool {
        self.0[0] && self.0[1] && self.0[2] && self.0[3]
    }

    #[inline(always)]
    pub fn any(self) -> bool {
        self.0[0] || self.0[1] || self.0[2] || self.0[3]
    }
}

impl Select<f64x4> for Mask4 {
    #[inline(always)]
    fn select(self, true_val: f64x4, false_val: f64x4) -> f64x4 {
        f64x4([
            if self.0[0] { true_val.0[0] } else { false_val.0[0] },
            if self.0[1] { true_val.0[1] } else { false_val.0[1] },
            if self.0[2] { true_val.0[2] } else { false_val.0[2] },
            if self.0[3] { true_val.0[3] } else { false_val.0[3] },
        ])
    }
}

impl Add for f64x4 {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self([
            self.0[0] + rhs.0[0],
            self.0[1] + rhs.0[1],
            self.0[2] + rhs.0[2],
            self.0[3] + rhs.0[3],
        ])
    }
}

impl Sub for f64x4 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self([
            self.0[0] - rhs.0[0],
            self.0[1] - rhs.0[1],
            self.0[2] - rhs.0[2],
            self.0[3] - rhs.0[3],
        ])
    }
}

impl Mul for f64x4 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        Self([
            self.0[0] * rhs.0[0],
            self.0[1] * rhs.0[1],
            self.0[2] * rhs.0[2],
            self.0[3] * rhs.0[3],
        ])
    }
}

impl Div for f64x4 {
    type Output = Self;
    #[inline(always)]
    fn div(self, rhs: Self) -> Self {
        Self([
            self.0[0] / rhs.0[0],
            self.0[1] / rhs.0[1],
            self.0[2] / rhs.0[2],
            self.0[3] / rhs.0[3],
        ])
    }
}

impl Neg for f64x4 {
    type Output = Self;
    #[inline(always)]
    fn neg(self) -> Self {
        Self([-self.0[0], -self.0[1], -self.0[2], -self.0[3]])
    }
}

impl BitAnd for Mask4 {
    type Output = Self;
    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self {
        Self([
            self.0[0] && rhs.0[0],
            self.0[1] && rhs.0[1],
            self.0[2] && rhs.0[2],
            self.0[3] && rhs.0[3],
        ])
    }
}

impl BitOr for Mask4 {
    type Output = Self;
    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self {
        Self([
            self.0[0] || rhs.0[0],
            self.0[1] || rhs.0[1],
            self.0[2] || rhs.0[2],
            self.0[3] || rhs.0[3],
        ])
    }
}

impl Not for Mask4 {
    type Output = Self;
    #[inline(always)]
    fn not(self) -> Self {
        Self([!self.0[0], !self.0[1], !self.0[2], !self.0[3]])
    }
}

#[inline]
pub fn has_avx2() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        is_x86_feature_detected!("avx2")
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        false
    }
}
