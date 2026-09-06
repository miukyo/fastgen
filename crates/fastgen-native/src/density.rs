//! Density function traits, noise parameters, and spline evaluation.

use crate::block::BlockStateId;
use crate::random::RandomSplitter;
use crate::simd::f64x4;
use crate::surface::SurfaceRuleContext;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone)]
pub struct NoiseParameters {
    pub first_octave: i32,
    pub amplitudes: Vec<f64>,
}

impl NoiseParameters {
    #[must_use]
    pub const fn new(first_octave: i32, amplitudes: Vec<f64>) -> Self {
        Self {
            first_octave,
            amplitudes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RarityValueMapper {
    Tunnels,
    Caves,
}

impl RarityValueMapper {
    #[must_use]
    pub fn get_values(self, rarity: f64) -> f64 {
        match self {
            Self::Tunnels => {
                if rarity < -0.5 {
                    0.75
                } else if rarity < 0.0 {
                    1.0
                } else if rarity < 0.5 {
                    1.5
                } else {
                    2.0
                }
            }
            Self::Caves => {
                if rarity < -0.75 {
                    0.5
                } else if rarity < -0.5 {
                    0.75
                } else if rarity < 0.5 {
                    1.0
                } else if rarity < 0.75 {
                    2.0
                } else {
                    3.0
                }
            }
        }
    }
}

pub mod spline_eval {
    #[inline]
    #[must_use]
    pub fn find_interval(locations: &[f32], input: f32) -> i32 {
        let mut lo = 0i32;
        let mut hi = locations.len() as i32;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if input < locations[mid as usize] {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        lo - 1
    }

    #[inline]
    #[must_use]
    pub fn hermite_interpolate(
        x1: f32,
        x2: f32,
        y1: f32,
        y2: f32,
        d1: f32,
        d2: f32,
        input: f32,
    ) -> f32 {
        let t = (input - x1) / (x2 - x1);
        let h = x2 - x1;
        let a = d1 * h - (y2 - y1);
        let b = -d2 * h + (y2 - y1);
        let lerp_y = y1 + t * (y2 - y1);
        let lerp_ab = a + t * (b - a);
        lerp_y + t * (1.0 - t) * lerp_ab
    }

    #[inline]
    pub fn evaluate_spline(
        locations: &[f32],
        derivatives: &[f32],
        input: f32,
        value_at: impl Fn(usize) -> f32,
    ) -> f32 {
        if locations.is_empty() {
            return 0.0;
        }

        let last = locations.len() - 1;
        let start = find_interval(locations, input);

        if start < 0 {
            let value = value_at(0);
            return value + derivatives[0] * (input - locations[0]);
        }

        let start = start as usize;
        if start == last {
            let value = value_at(last);
            return value + derivatives[last] * (input - locations[last]);
        }

        let y1 = value_at(start);
        let y2 = value_at(start + 1);
        hermite_interpolate(
            locations[start],
            locations[start + 1],
            y1,
            y2,
            derivatives[start],
            derivatives[start + 1],
            input,
        )
    }
}

pub trait NoiseSettings: Send + Sync {
    const MIN_Y: i32;
    const HEIGHT: i32;
    const SEA_LEVEL: i32;
    const CELL_WIDTH: i32;
    const CELL_HEIGHT: i32;
    const AQUIFERS_ENABLED: bool;
    const ORE_VEINS_ENABLED: bool;
    const LEGACY_RANDOM_SOURCE: bool;

    fn default_block_id() -> BlockStateId;
    fn default_fluid_id() -> BlockStateId;
}

pub trait ColumnCache: Clone + Default + Send + Sync {
    type Noises: DimensionNoises<ColumnCache = Self>;
    fn ensure(&mut self, x: i32, z: i32, noises: &Self::Noises);
    fn init_grid(&mut self, chunk_block_x: i32, chunk_block_z: i32, noises: &Self::Noises);
}

pub trait DimensionNoises: Sized + Send + Sync {
    type ColumnCache: ColumnCache<Noises = Self>;
    type Settings: NoiseSettings;

    fn create(
        seed: u64,
        splitter: &RandomSplitter,
        params: &FxHashMap<String, NoiseParameters>,
    ) -> Self;

    fn router_final_density(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_depth(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_barrier(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_fluid_level_floodedness(
        &self,
        cache: &mut Self::ColumnCache,
        x: i32,
        y: i32,
        z: i32,
    ) -> f64;
    fn router_fluid_level_spread(
        &self,
        cache: &mut Self::ColumnCache,
        x: i32,
        y: i32,
        z: i32,
    ) -> f64;
    fn router_lava(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_vein_toggle(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_vein_ridged(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_vein_gap(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_erosion(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_continentalness(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_temperature(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_vegetation(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_ridges(&self, cache: &mut Self::ColumnCache, x: i32, y: i32, z: i32) -> f64;
    fn router_preliminary_surface_level(
        &self,
        cache: &mut Self::ColumnCache,
        x: i32,
        y: i32,
        z: i32,
    ) -> f64;

    fn interpolated_count() -> usize;
    fn vein_interp_enabled() -> bool;

    fn compute_noise_column(&self, x: i32, block_ys: &[i32], z: i32, out: &mut [f64]);

    fn fill_cell_corner_densities(
        &self,
        cache: &mut Self::ColumnCache,
        x: i32,
        y: i32,
        z: i32,
        blended_noise_value: f64,
        out: &mut [f64],
    );

    fn fill_cell_corner_densities_4x(
        &self,
        cache: &mut Self::ColumnCache,
        x: i32,
        ys: f64x4,
        z: i32,
        blended_noise_values: f64x4,
        out: &mut [f64],
    );

    fn combine_interpolated(
        &self,
        cache: &mut Self::ColumnCache,
        interpolated: &[f64],
        x: i32,
        y: i32,
        z: i32,
    ) -> f64;

    fn combine_vein_toggle(
        &self,
        cache: &mut Self::ColumnCache,
        interpolated: &[f64],
        x: i32,
        y: i32,
        z: i32,
    ) -> f64;

    fn combine_vein_ridged(
        &self,
        cache: &mut Self::ColumnCache,
        interpolated: &[f64],
        x: i32,
        y: i32,
        z: i32,
    ) -> f64;

    fn surface_noise_ids() -> &'static [&'static str];
    fn surface_gradient_ids() -> &'static [&'static str];
    fn surface_rule_block_states() -> &'static [BlockStateId];

    fn surface_rule_uses_biome() -> bool;
    fn surface_rule_uses_preliminary_surface() -> bool;
    fn surface_rule_uses_surface_secondary() -> bool;
    fn surface_rule_uses_steep() -> bool;

    fn try_apply_surface_rule(ctx: &mut SurfaceRuleContext<'_>) -> Option<BlockStateId>;
}
