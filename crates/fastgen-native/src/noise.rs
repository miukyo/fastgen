//! Noise implementations: Improved Perlin, Multi-octave Perlin, Double-Perlin Normal, and BlendedNoise.

use crate::math::{clamped_lerp, fast_floor, lerp, lerp2, lerp3, smoothstep, wrap};
use crate::random::{NameHash, PositionalRandom, Random, RandomSplitter, Xoroshiro};
use crate::simd::f64x4;

pub const GRADIENT: [[f64; 3]; 16] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
    [1.0, 1.0, 0.0],
    [0.0, -1.0, 1.0],
    [-1.0, 1.0, 0.0],
    [0.0, -1.0, -1.0],
];

#[inline(always)]
fn grad_dot(p: &[u8; 256], x: i32, y: i32, z: i32, xr: f64, yr: f64, zr: f64) -> f64 {
    let x_idx = (x & 255) as usize;
    let y_idx = (y & 255) as usize;
    let z_idx = (z & 255) as usize;
    let h = p[(p[(p[x_idx] as usize + y_idx) & 255] as usize + z_idx) & 255] as usize & 15;
    let g = GRADIENT[h];
    g[0] * xr + g[1] * yr + g[2] * zr
}

#[derive(Debug, Clone)]
pub struct ImprovedNoise {
    p: [u8; 256],
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    yo_floor: i32,
    yo_fraction: f64,
    zo_floor: i32,
    zo_fraction: f64,
}

impl ImprovedNoise {
    pub fn new<R: Random>(random: &mut R) -> Self {
        let xo = random.next_f64() * 256.0;
        let yo = random.next_f64() * 256.0;
        let zo = random.next_f64() * 256.0;

        let mut p = [0u8; 256];
        for i in 0..256 {
            p[i] = i as u8;
        }

        for i in 0..256 {
            let offset = random.next_i32_bounded((256 - i) as i32) as usize;
            p.swap(i, i + offset);
        }

        let yo_floor = fast_floor(yo);
        let yo_fraction = yo - f64::from(yo_floor);
        let zo_floor = fast_floor(zo);
        let zo_fraction = zo - f64::from(zo_floor);

        Self {
            p,
            xo,
            yo,
            zo,
            yo_floor,
            yo_fraction,
            zo_floor,
            zo_fraction,
        }
    }

    #[inline]
    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;

        let xf = fast_floor(x);
        let yf = fast_floor(y);
        let zf = fast_floor(z);

        let xr = x - f64::from(xf);
        let yr = y - f64::from(yf);
        let zr = z - f64::from(zf);

        self.sample_and_lerp(xf, yf, zf, xr, yr, zr, yr)
    }

    #[inline]
    pub fn noise_xz(&self, x: f64, z: f64) -> f64 {
        let x = x + self.xo;
        let z = z + self.zo;

        let xf = fast_floor(x);
        let zf = fast_floor(z);

        let xr = x - f64::from(xf);
        let zr = z - f64::from(zf);

        self.sample_and_lerp(
            xf,
            self.yo_floor,
            zf,
            xr,
            self.yo_fraction,
            zr,
            self.yo_fraction,
        )
    }

    #[inline]
    pub fn noise_xy(&self, x: f64, y: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;

        let xf = fast_floor(x);
        let yf = fast_floor(y);

        let xr = x - f64::from(xf);
        let yr = y - f64::from(yf);

        self.sample_and_lerp(
            xf,
            yf,
            self.zo_floor,
            xr,
            yr,
            self.zo_fraction,
            yr,
        )
    }

    #[inline]
    pub fn noise_with_y_scale(&self, x: f64, y: f64, z: f64, y_scale: f64, y_fudge: f64) -> f64 {
        let x = x + self.xo;
        let y = y + self.yo;
        let z = z + self.zo;

        let xf = fast_floor(x);
        let yf = fast_floor(y);
        let zf = fast_floor(z);

        let xr = x - f64::from(xf);
        let yr = y - f64::from(yf);
        let zr = z - f64::from(zf);

        let yr_fudge = if y_scale != 0.0 {
            let fudge_limit = if y_fudge >= 0.0 && y_fudge < yr {
                y_fudge
            } else {
                yr
            };
            (fudge_limit / y_scale + f64::from(1.0e-7_f32)).floor() * y_scale
        } else {
            0.0
        };
        self.sample_and_lerp(xf, yf, zf, xr, yr - yr_fudge, zr, yr)
    }

    #[inline]
    fn sample_and_lerp(
        &self,
        x: i32,
        y: i32,
        z: i32,
        xr: f64,
        yr: f64,
        zr: f64,
        fade_y: f64,
    ) -> f64 {
        let u = smoothstep(xr);
        let v = smoothstep(fade_y);
        let w = smoothstep(zr);

        let d000 = grad_dot(&self.p, x, y, z, xr, yr, zr);
        let d100 = grad_dot(&self.p, x + 1, y, z, xr - 1.0, yr, zr);
        let d010 = grad_dot(&self.p, x, y + 1, z, xr, yr - 1.0, zr);
        let d110 = grad_dot(&self.p, x + 1, y + 1, z, xr - 1.0, yr - 1.0, zr);
        let d001 = grad_dot(&self.p, x, y, z + 1, xr, yr, zr - 1.0);
        let d101 = grad_dot(&self.p, x + 1, y, z + 1, xr - 1.0, yr, zr - 1.0);
        let d011 = grad_dot(&self.p, x, y + 1, z + 1, xr, yr - 1.0, zr - 1.0);
        let d111 = grad_dot(&self.p, x + 1, y + 1, z + 1, xr - 1.0, yr - 1.0, zr - 1.0);

        lerp3(u, v, w, d000, d100, d010, d110, d001, d101, d011, d111)
    }
}

#[derive(Debug, Clone)]
pub struct PerlinNoise {
    noise_levels: Vec<Option<ImprovedNoise>>,
    amplitudes: Vec<f64>,
    lowest_freq_value_factor: f64,
    lowest_freq_input_factor: f64,
    max_value: f64,
}

impl PerlinNoise {
    pub fn create(
        splitter: &RandomSplitter,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let octaves = amplitudes.len();
        let zero_octave_index = (-first_octave) as usize;

        let mut noise_levels = Vec::with_capacity(octaves);
        for i in 0..octaves {
            if amplitudes[i] != 0.0 {
                let octave = first_octave + i as i32;
                let name = format!("octave_{octave}");
                let mut octave_random = splitter.with_hash_of(&NameHash::new(&name));
                noise_levels.push(Some(ImprovedNoise::new(&mut octave_random)));
            } else {
                noise_levels.push(None);
            }
        }

        Self::from_parts(noise_levels, amplitudes, zero_octave_index)
    }

    pub fn create_legacy_for_nether<R: Random>(
        random: &mut R,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let octaves = amplitudes.len();
        let zero_octave_index = (-first_octave) as usize;

        let mut noise_levels = vec![None; octaves];

        if zero_octave_index < octaves && amplitudes[zero_octave_index] != 0.0 {
            noise_levels[zero_octave_index] = Some(ImprovedNoise::new(random));
        } else {
            for _ in 0..262 {
                random.next_random();
            }
        }

        for ix in (0..zero_octave_index).rev() {
            if ix < octaves && amplitudes[ix] != 0.0 {
                noise_levels[ix] = Some(ImprovedNoise::new(random));
            } else {
                for _ in 0..262 {
                    random.next_random();
                }
            }
        }

        for ix in (zero_octave_index + 1)..octaves {
            if amplitudes[ix] != 0.0 {
                noise_levels[ix] = Some(ImprovedNoise::new(random));
            } else {
                for _ in 0..262 {
                    random.next_random();
                }
            }
        }

        Self::from_parts(noise_levels, amplitudes, zero_octave_index)
    }

    fn from_parts(
        noise_levels: Vec<Option<ImprovedNoise>>,
        amplitudes: &[f64],
        zero_octave_index: usize,
    ) -> Self {
        let octaves = amplitudes.len();
        let lowest_freq_input_factor = 2.0_f64.powi(-(zero_octave_index as i32));
        let lowest_freq_value_factor =
            2.0_f64.powi((octaves - 1) as i32) / (2.0_f64.powi(octaves as i32) - 1.0);

        let mut max_value = 0.0;
        let mut v_factor = lowest_freq_value_factor;
        for &amp in amplitudes {
            if amp != 0.0 {
                max_value += amp * 2.0 * v_factor;
            }
            v_factor /= 2.0;
        }

        Self {
            noise_levels,
            amplitudes: amplitudes.to_vec(),
            lowest_freq_value_factor,
            lowest_freq_input_factor,
            max_value,
        }
    }

    #[inline]
    pub fn get_octave_noise(&self, octave: usize) -> Option<&ImprovedNoise> {
        self.noise_levels.get(octave).and_then(|o| o.as_ref())
    }

    #[inline]
    pub fn max_broken_value(&self, _y_scale: f64) -> f64 {
        let mut v = 0.0;
        let mut factor = self.lowest_freq_value_factor;
        for &amp in &self.amplitudes {
            if amp != 0.0 {
                v += amp * factor;
            }
            factor /= 2.0;
        }
        v
    }

    #[inline]
    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut input_factor = self.lowest_freq_input_factor;
        let mut value_factor = self.lowest_freq_value_factor;
        for (i, noise_opt) in self.noise_levels.iter().enumerate() {
            if let Some(noise) = noise_opt {
                let n = noise.noise(wrap(x * input_factor), wrap(y * input_factor), wrap(z * input_factor));
                value += self.amplitudes[i] * value_factor * n;
            }
            input_factor *= 2.0;
            value_factor /= 2.0;
        }
        value
    }

    #[inline]
    pub fn get_value_xz(&self, x: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut input_factor = self.lowest_freq_input_factor;
        let mut value_factor = self.lowest_freq_value_factor;
        for (i, noise_opt) in self.noise_levels.iter().enumerate() {
            if let Some(noise) = noise_opt {
                let n = noise.noise_xz(wrap(x * input_factor), wrap(z * input_factor));
                value += self.amplitudes[i] * value_factor * n;
            }
            input_factor *= 2.0;
            value_factor /= 2.0;
        }
        value
    }

    #[inline]
    pub fn get_value_xy(&self, x: f64, y: f64) -> f64 {
        let mut value = 0.0;
        let mut input_factor = self.lowest_freq_input_factor;
        let mut value_factor = self.lowest_freq_value_factor;
        for (i, noise_opt) in self.noise_levels.iter().enumerate() {
            if let Some(noise) = noise_opt {
                let n = noise.noise_xy(wrap(x * input_factor), wrap(y * input_factor));
                value += self.amplitudes[i] * value_factor * n;
            }
            input_factor *= 2.0;
            value_factor /= 2.0;
        }
        value
    }

    #[inline]
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
}

pub const INPUT_FACTOR: f64 = 1.0181268882175227;
pub const VALUE_FACTOR_NUMERATOR: f64 = 0.16666666666666666;

#[inline]
fn expected_deviation(octave_span: i32) -> f64 {
    0.1 * (1.0 + 1.0 / f64::from(octave_span + 1))
}

#[derive(Debug, Clone)]
pub struct NormalNoise {
    first: PerlinNoise,
    second: PerlinNoise,
    value_factor: f64,
    max_value: f64,
}

impl NormalNoise {
    pub fn create(
        splitter: &RandomSplitter,
        name: &str,
        first_octave: i32,
        amplitudes: &[f64],
    ) -> Self {
        let mut random = splitter.from_hash_of(name);
        let first_splitter = random.next_positional();
        let second_splitter = random.next_positional();

        let first = PerlinNoise::create(&first_splitter, first_octave, amplitudes);
        let second = PerlinNoise::create(&second_splitter, first_octave, amplitudes);

        let min_octave = first_octave;
        let max_octave = first_octave + amplitudes.len() as i32 - 1;
        let octave_span = max_octave - min_octave;
        let value_factor = VALUE_FACTOR_NUMERATOR / expected_deviation(octave_span);
        let max_value = (first.max_value() + second.max_value()) * value_factor;

        Self {
            first,
            second,
            value_factor,
            max_value,
        }
    }

    #[inline]
    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let x2 = x * INPUT_FACTOR;
        let y2 = y * INPUT_FACTOR;
        let z2 = z * INPUT_FACTOR;
        (self.first.get_value(x, y, z) + self.second.get_value(x2, y2, z2)) * self.value_factor
    }

    #[inline]
    pub fn get_value_xz(&self, x: f64, z: f64) -> f64 {
        let x2 = x * INPUT_FACTOR;
        let z2 = z * INPUT_FACTOR;
        (self.first.get_value_xz(x, z) + self.second.get_value_xz(x2, z2)) * self.value_factor
    }

    #[inline]
    pub fn get_value_xy(&self, x: f64, y: f64) -> f64 {
        let x2 = x * INPUT_FACTOR;
        let y2 = y * INPUT_FACTOR;
        (self.first.get_value_xy(x, y) + self.second.get_value_xy(x2, y2)) * self.value_factor
    }

    #[inline]
    pub fn get_value_y_simd(&self, x: f64, ys: f64x4, z: f64) -> f64x4 {
        f64x4([
            self.get_value(x, ys.0[0], z),
            self.get_value(x, ys.0[1], z),
            self.get_value(x, ys.0[2], z),
            self.get_value(x, ys.0[3], z),
        ])
    }

    #[inline]
    pub fn max_value(&self) -> f64 {
        self.max_value
    }
}

const COORDINATE_SCALE: f64 = 684.412;

#[derive(Debug, Clone)]
pub struct BlendedNoise {
    min_limit_noise: PerlinNoise,
    max_limit_noise: PerlinNoise,
    main_noise: PerlinNoise,
    xz_multiplier: f64,
    y_multiplier: f64,
    xz_factor: f64,
    y_factor: f64,
    smear_scale_multiplier: f64,
    max_value: f64,
}

impl BlendedNoise {
    pub fn new<R: Random>(
        random: &mut R,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        let min_limit_noise = PerlinNoise::create_legacy_for_nether(random, -15, &[1.0; 16]);
        let max_limit_noise = PerlinNoise::create_legacy_for_nether(random, -15, &[1.0; 16]);
        let main_noise = PerlinNoise::create_legacy_for_nether(random, -7, &[1.0; 8]);

        let xz_multiplier = COORDINATE_SCALE * xz_scale;
        let y_multiplier = COORDINATE_SCALE * y_scale;
        let max_value = min_limit_noise.max_broken_value(y_multiplier);

        Self {
            min_limit_noise,
            max_limit_noise,
            main_noise,
            xz_multiplier,
            y_multiplier,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
            max_value,
        }
    }

    #[inline]
    pub fn compute(&self, block_x: f64, block_y: f64, block_z: f64) -> f64 {
        let limit_x = block_x * self.xz_multiplier;
        let limit_y = block_y * self.y_multiplier;
        let limit_z = block_z * self.xz_multiplier;
        let main_x = limit_x / self.xz_factor;
        let main_y = limit_y / self.y_factor;
        let main_z = limit_z / self.xz_factor;
        let limit_smear = self.y_multiplier * self.smear_scale_multiplier;
        let main_smear = limit_smear / self.y_factor;

        let mut main_noise_value = 0.0;
        let mut pow = 1.0;
        for i in 0..8 {
            if let Some(noise) = self.main_noise.get_octave_noise(i) {
                main_noise_value += noise.noise_with_y_scale(
                    wrap(main_x * pow),
                    wrap(main_y * pow),
                    wrap(main_z * pow),
                    main_smear * pow,
                    main_y * pow,
                ) / pow;
            }
            pow /= 2.0;
        }

        let factor = (main_noise_value / 10.0 + 1.0) / 2.0;
        let is_max = factor >= 1.0;
        let is_min = factor <= 0.0;

        let mut blend_min = 0.0;
        let mut blend_max = 0.0;
        pow = 1.0;
        for i in 0..16 {
            let wx = wrap(limit_x * pow);
            let wy = wrap(limit_y * pow);
            let wz = wrap(limit_z * pow);
            let y_scale_pow = limit_smear * pow;

            if !is_max {
                if let Some(noise) = self.min_limit_noise.get_octave_noise(i) {
                    blend_min += noise.noise_with_y_scale(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
                }
            }

            if !is_min {
                if let Some(noise) = self.max_limit_noise.get_octave_noise(i) {
                    blend_max += noise.noise_with_y_scale(wx, wy, wz, y_scale_pow, limit_y * pow) / pow;
                }
            }

            pow /= 2.0;
        }

        clamped_lerp(blend_min / 512.0, blend_max / 512.0, factor) / 128.0
    }

    #[inline]
    pub fn compute_column(&self, block_x: i32, block_ys: &[i32], block_z: i32, out: &mut [f64]) {
        let count = block_ys.len().min(out.len());
        let bx = block_x as f64;
        let bz = block_z as f64;
        for i in 0..count {
            out[i] = self.compute(bx, block_ys[i] as f64, bz);
        }
    }
}

#[derive(Debug, Clone)]
pub struct EndIslands;

impl EndIslands {
    #[must_use]
    pub fn new(_seed: u64) -> Self {
        Self
    }

    #[inline]
    #[must_use]
    pub fn get_height_value(&self, _x: i32, _y: i32, _z: i32) -> f64 {
        0.0
    }

    #[inline]
    #[must_use]
    pub fn sample(&self, _x: f64, _y: f64, _z: f64) -> f64 {
        0.0
    }
}

