//! Overworld Climate Sampler matching SteelMC (`steel-worldgen/src/biomes/climate_sampler.rs`).
//! Evaluates the overworld noise router (temperature, vegetation, continentalness,
//! erosion, depth, ridges) to produce Climate5D points for biome lookup.

use crate::density_functions::overworld::{self, OverworldColumnCache, OverworldNoises};
use crate::noise_parameters::get_noise_parameters;
use crate::random::{Random, Xoroshiro};

/// 5D Climate multi-noise parameter point
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Climate5D {
    pub continentalness: f32,
    pub erosion: f32,
    pub temperature: f32,
    pub humidity: f32,
    pub weirdness: f32,
}

impl Climate5D {
    #[inline]
    pub const fn new(c: f32, e: f32, t: f32, h: f32, w: f32) -> Self {
        Self {
            continentalness: c,
            erosion: e,
            temperature: t,
            humidity: h,
            weirdness: w,
        }
    }
}

use std::sync::Arc;

/// Climate sampler for the overworld using exact vanilla noise router functions.
pub struct OverworldClimateSampler {
    pub noises: Arc<OverworldNoises>,
}

impl OverworldClimateSampler {
    #[inline]
    pub fn new(seed: u64) -> Self {
        let ctx = crate::context::get_or_create_context(seed as i64);
        Self {
            noises: Arc::clone(&ctx.noises),
        }
    }

    #[inline]
    pub fn from_arc(noises: Arc<OverworldNoises>) -> Self {
        Self { noises }
    }

    /// Sample climate at surface world coordinates (wx, wz).
    #[inline]
    pub fn sample(&self, wx: f64, wz: f64) -> Climate5D {
        let mut cache = OverworldColumnCache::new();
        let block_x = wx as i32;
        let block_z = wz as i32;
        cache.ensure(block_x, block_z, &self.noises);

        let t = overworld::router_temperature(&self.noises, &cache, wx, 0.0, wz) as f32;
        let h = overworld::router_vegetation(&self.noises, &cache, wx, 0.0, wz) as f32;
        let c = overworld::router_continentalness(&self.noises, &cache, wx, 0.0, wz) as f32;
        let e = overworld::router_erosion(&self.noises, &cache, wx, 0.0, wz) as f32;
        let w = overworld::router_ridges(&self.noises, &cache, wx, 0.0, wz) as f32;

        Climate5D {
            continentalness: c.clamp(-1.0, 1.0),
            erosion: e.clamp(-1.0, 1.0),
            temperature: t.clamp(-1.0, 1.0),
            humidity: h.clamp(-1.0, 1.0),
            weirdness: w.clamp(-1.0, 1.0),
        }
    }

    /// Sample climate at 3D world coordinates (wx, wy, wz).
    #[inline]
    pub fn sample_3d(&self, wx: f64, wy: f64, wz: f64, cache: &mut OverworldColumnCache) -> (Climate5D, f32) {
        let block_x = wx as i32;
        let block_z = wz as i32;
        cache.ensure(block_x, block_z, &self.noises);

        let t = overworld::router_temperature(&self.noises, cache, wx, wy, wz) as f32;
        let h = overworld::router_vegetation(&self.noises, cache, wx, wy, wz) as f32;
        let c = overworld::router_continentalness(&self.noises, cache, wx, wy, wz) as f32;
        let e = overworld::router_erosion(&self.noises, cache, wx, wy, wz) as f32;
        let d = overworld::router_depth(&self.noises, cache, wx, wy, wz) as f32;
        let w = overworld::router_ridges(&self.noises, cache, wx, wy, wz) as f32;

        (
            Climate5D {
                continentalness: c.clamp(-1.0, 1.0),
                erosion: e.clamp(-1.0, 1.0),
                temperature: t.clamp(-1.0, 1.0),
                humidity: h.clamp(-1.0, 1.0),
                weirdness: w.clamp(-1.0, 1.0),
            },
            d,
        )
    }
}
