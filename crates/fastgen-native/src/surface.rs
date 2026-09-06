//! Surface rule evaluation and surface system.

use std::cell::Cell;
use crate::block::{BlockStateId, BLOCK_TERRACOTTA, BLOCK_ORANGE_TERRACOTTA, BLOCK_WHITE_TERRACOTTA, BLOCK_YELLOW_TERRACOTTA, BLOCK_BROWN_TERRACOTTA, BLOCK_RED_TERRACOTTA, BLOCK_LIGHT_GRAY_TERRACOTTA};
use crate::density::NoiseParameters;
use crate::noise::NormalNoise;
use crate::random::{NameHash, PositionalRandom, Random, RandomSplitter, RandomSource};
use rustc_hash::FxHashMap;

pub const CLAY_BAND_LENGTH: usize = 192;

pub trait SurfaceNoiseProvider {
    fn condition_noise(&self, noise_index: usize, x: i32, z: i32) -> f64;
    fn condition_noise_3d(&self, noise_index: usize, x: i32, y: i32, z: i32) -> f64;
    fn get_band(&self, x: i32, y: i32, z: i32) -> BlockStateId;
    fn vertical_gradient(
        &self,
        gradient_index: usize,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        true_at_and_below: i32,
        false_at_and_above: i32,
    ) -> bool;
    fn cold_enough_to_snow(&self, biome_id: u16, block_x: i32, block_y: i32, block_z: i32) -> bool;
}

pub struct SurfaceConditionNoiseCache<'a> {
    values: &'a [Cell<f64>],
    initialized: &'a [Cell<bool>],
}

impl<'a> SurfaceConditionNoiseCache<'a> {
    #[must_use]
    pub const fn new(values: &'a [Cell<f64>], initialized: &'a [Cell<bool>]) -> Self {
        Self {
            values,
            initialized,
        }
    }

    pub fn reset(&self) {
        for init in self.initialized {
            init.set(false);
        }
    }

    pub fn get(
        &self,
        noise_index: usize,
        system: &dyn SurfaceNoiseProvider,
        x: i32,
        z: i32,
    ) -> f64 {
        if !self.initialized[noise_index].get() {
            let val = system.condition_noise(noise_index, x, z);
            self.values[noise_index].set(val);
            self.initialized[noise_index].set(true);
            val
        } else {
            self.values[noise_index].get()
        }
    }
}

pub struct SurfaceRuleContext<'a> {
    pub block_x: i32,
    pub block_z: i32,
    pub surface_depth: i32,
    pub surface_secondary: f64,
    pub min_surface_level: i32,
    pub steep: bool,
    pub block_y: i32,
    pub stone_depth_above: i32,
    pub stone_depth_below: i32,
    pub water_height: i32,
    pub biome_id: Option<u16>,
    pub system: &'a dyn SurfaceNoiseProvider,
    pub condition_noises: &'a SurfaceConditionNoiseCache<'a>,
    pub block_states: &'a [BlockStateId],
}

impl<'a> SurfaceRuleContext<'a> {
    #[inline]
    pub fn condition_noise(&self, noise_index: usize) -> f64 {
        self.condition_noises
            .get(noise_index, self.system, self.block_x, self.block_z)
    }

    #[inline]
    pub fn condition_noise_3d(&self, noise_index: usize) -> f64 {
        self.system
            .condition_noise_3d(noise_index, self.block_x, self.block_y, self.block_z)
    }

    #[inline]
    pub fn block_state(&self, index: usize) -> BlockStateId {
        self.block_states[index]
    }

    #[inline]
    pub fn biome_id(&mut self) -> Option<u16> {
        self.biome_id
    }

    #[inline]
    pub fn min_surface_level(&self) -> i32 {
        self.min_surface_level
    }

    #[inline]
    pub fn steep(&self) -> bool {
        self.steep
    }

    #[inline]
    pub fn surface_secondary(&self) -> f64 {
        self.surface_secondary
    }

    #[inline]
    pub fn cold_enough_to_snow(&mut self) -> bool {
        if let Some(biome) = self.biome_id {
            self.system.cold_enough_to_snow(biome, self.block_x, self.block_y, self.block_z)
        } else {
            false
        }
    }
}

pub struct SurfaceSystem {
    surface_noise: NormalNoise,
    surface_secondary_noise: NormalNoise,
    clay_bands_offset_noise: NormalNoise,
    clay_bands: [BlockStateId; CLAY_BAND_LENGTH],
    noise_random: RandomSplitter,
    condition_noises: Vec<NormalNoise>,
    vertical_gradient_randoms: Vec<RandomSplitter>,
}

impl SurfaceSystem {
    pub fn new(
        splitter: &RandomSplitter,
        noise_params: &FxHashMap<String, NoiseParameters>,
        condition_noise_ids: &[&str],
        vertical_gradient_ids: &[&str],
    ) -> Self {
        const CLAY_BANDS_HASH: NameHash = NameHash::new("minecraft:clay_bands");
        let noise_random = splitter.clone();
        let mut band_random = noise_random.with_hash_of(&CLAY_BANDS_HASH);
        let clay_bands = Self::generate_bands(&mut band_random);

        let condition_noises: Vec<NormalNoise> = condition_noise_ids
            .iter()
            .map(|&id| {
                let p = noise_params.get(id).unwrap_or_else(|| panic!("missing noise: {id}"));
                NormalNoise::create(splitter, id, p.first_octave, &p.amplitudes)
            })
            .collect();

        let vertical_gradient_randoms: Vec<RandomSplitter> = vertical_gradient_ids
            .iter()
            .map(|&id| {
                let hash = NameHash::new(id);
                let mut random = splitter.with_hash_of(&hash);
                random.next_positional()
            })
            .collect();

        let p_surface = noise_params.get("minecraft:surface").expect("surface params");
        let p_surface_sec = noise_params.get("minecraft:surface_secondary").expect("surface_secondary params");
        let p_clay = noise_params.get("minecraft:clay_bands_offset").expect("clay params");

        Self {
            surface_noise: NormalNoise::create(splitter, "minecraft:surface", p_surface.first_octave, &p_surface.amplitudes),
            surface_secondary_noise: NormalNoise::create(splitter, "minecraft:surface_secondary", p_surface_sec.first_octave, &p_surface_sec.amplitudes),
            clay_bands_offset_noise: NormalNoise::create(splitter, "minecraft:clay_bands_offset", p_clay.first_octave, &p_clay.amplitudes),
            clay_bands,
            noise_random,
            condition_noises,
            vertical_gradient_randoms,
        }
    }

    #[inline]
    pub fn get_surface_depth(&self, x: i32, z: i32) -> i32 {
        let noise_value = self.surface_noise.get_value(f64::from(x), 0.0, f64::from(z));
        let jitter = self.noise_random.at(x, 0, z).next_f64() * 0.25;
        (noise_value * 2.75 + 3.0 + jitter) as i32
    }

    #[inline]
    pub fn get_surface_secondary(&self, x: i32, z: i32) -> f64 {
        self.surface_secondary_noise.get_value(f64::from(x), 0.0, f64::from(z))
    }

    fn generate_bands<R: Random>(random: &mut R) -> [BlockStateId; CLAY_BAND_LENGTH] {
        let mut bands = [BLOCK_TERRACOTTA; CLAY_BAND_LENGTH];
        let mut i = 0usize;
        while i < CLAY_BAND_LENGTH {
            i += random.next_i32_bounded(5) as usize + 1;
            if i < CLAY_BAND_LENGTH {
                bands[i] = BLOCK_ORANGE_TERRACOTTA;
            }
            i += 1;
        }

        Self::make_bands(random, &mut bands, 1, BLOCK_YELLOW_TERRACOTTA);
        Self::make_bands(random, &mut bands, 2, BLOCK_BROWN_TERRACOTTA);
        Self::make_bands(random, &mut bands, 1, BLOCK_RED_TERRACOTTA);
        bands
    }

    fn make_bands<R: Random>(
        random: &mut R,
        bands: &mut [BlockStateId; CLAY_BAND_LENGTH],
        base_width: i32,
        state: BlockStateId,
    ) {
        let band_count = random.next_i32_bounded(10) + 6;
        for _ in 0..band_count {
            let width = (base_width + random.next_i32_bounded(3)) as usize;
            let start = random.next_i32_bounded(CLAY_BAND_LENGTH as i32) as usize;
            for p in 0..width {
                if start + p >= CLAY_BAND_LENGTH {
                    break;
                }
                bands[start + p] = state;
            }
        }
    }
}

impl SurfaceNoiseProvider for SurfaceSystem {
    #[inline]
    fn condition_noise(&self, noise_index: usize, x: i32, z: i32) -> f64 {
        self.condition_noises[noise_index].get_value(f64::from(x), 0.0, f64::from(z))
    }

    #[inline]
    fn condition_noise_3d(&self, noise_index: usize, x: i32, y: i32, z: i32) -> f64 {
        self.condition_noises[noise_index].get_value(f64::from(x), f64::from(y), f64::from(z))
    }

    #[inline]
    fn get_band(&self, x: i32, y: i32, z: i32) -> BlockStateId {
        let offset = (self.clay_bands_offset_noise.get_value(f64::from(x), 0.0, f64::from(z)) * 4.0 + 0.5).floor() as i32;
        let index = ((y + offset) % CLAY_BAND_LENGTH as i32 + CLAY_BAND_LENGTH as i32) as usize % CLAY_BAND_LENGTH;
        self.clay_bands[index]
    }

    #[inline]
    fn vertical_gradient(
        &self,
        gradient_index: usize,
        block_x: i32,
        block_y: i32,
        block_z: i32,
        true_at_and_below: i32,
        false_at_and_above: i32,
    ) -> bool {
        if block_y <= true_at_and_below {
            return true;
        }
        if block_y >= false_at_and_above {
            return false;
        }
        let probability = f64::from(false_at_and_above - block_y) / f64::from(false_at_and_above - true_at_and_below);
        let factory = &self.vertical_gradient_randoms[gradient_index];
        let random_value = factory.at(block_x, block_y, block_z).next_f64();
        random_value < probability
    }

    #[inline]
    fn cold_enough_to_snow(&self, _biome_id: u16, _block_x: i32, block_y: i32, _block_z: i32) -> bool {
        block_y > 100
    }
}
