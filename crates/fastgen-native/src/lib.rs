//! Fast Native World Generation Math Engine for Paper & Folia
//! 1:1 Math and Architectural Parity with SteelMC (steel-worldgen).

#![allow(unexpected_cfgs)]
#![allow(warnings)]

pub mod math;
pub mod simd;
pub mod block;
pub mod random;
pub mod noise;
pub mod density;
pub mod aquifer;
pub mod noise_chunk;
pub mod surface;
pub mod climate;
pub mod biomes;
pub mod carver;
pub mod context;
pub mod threads;

#[path = "generated/vanilla_noise_parameters.rs"]
pub mod noise_parameters;

#[path = "generated/vanilla_density_functions/mod.rs"]
pub mod density_functions;

// Global module aliases for transpiled density functions
pub use crate as steel_worldgen;
pub use crate as steel_utils;
pub use crate::math as steel_math;

pub mod steel_registry {
    pub use crate::block::vanilla_blocks;
    pub use crate::block::vanilla_biomes;
    pub use crate::block::BlockStateExt;
    pub use crate::block::RegistryEntry;
    pub use crate::block::RegistryExt;
    pub use crate::block::REGISTRY;
    pub mod blocks {
        pub mod block_state_ext {
            pub use crate::block::BlockStateExt;
        }
    }
}

use std::cell::Cell;
use std::sync::Arc;
use crate::block::{BlockStateId, BLOCK_AIR, BLOCK_STONE, BLOCK_DEEPSLATE};
use crate::density::DimensionNoises;
use crate::density_functions::overworld::{OverworldColumnCache, OverworldNoises};
use crate::noise_parameters::get_noise_parameters;
use crate::random::{Random, Xoroshiro, XoroshiroSplitter};

#[unsafe(no_mangle)]
pub extern "C" fn fastgen_has_avx2() -> i32 {
    if simd::has_avx2() { 1 } else { 0 }
}

/// Set target Minecraft worldgen version (1 = 26.1, 2 = 26.2).
#[unsafe(no_mangle)]
pub extern "C" fn fastgen_set_version(version: i32) {
    biomes::set_version(version);
}

/// Get current active Minecraft worldgen version (1 = 26.1, 2 = 26.2).
#[unsafe(no_mangle)]
pub extern "C" fn fastgen_get_version() -> i32 {
    biomes::get_version()
}

/// Set target worker threads count for native parallelism and Rayon thread pool.
#[unsafe(no_mangle)]
pub extern "C" fn fastgen_set_worker_threads(threads: i32) -> i32 {
    threads::set_worker_threads(threads)
}

/// Get currently active worker threads count.
#[unsafe(no_mangle)]
pub extern "C" fn fastgen_get_worker_threads() -> i32 {
    threads::get_worker_threads()
}

/// Compute 16x16 exact vanilla biomes for chunk (256 bytes, values 0..54).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fastgen_compute_biomes(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    out_biomes: *mut u8,
) {
    if out_biomes.is_null() {
        return;
    }

    let ctx = context::get_or_create_context(seed);
    let world_base_x = chunk_x * 16;
    let world_base_z = chunk_z * 16;

    for local_x in 0..16 {
        for local_z in 0..16 {
            let wx = (world_base_x + local_x) as f64;
            let wz = (world_base_z + local_z) as f64;

            let c = ctx.climate_sampler.sample(wx, wz);
            let b_id = biomes::lookup_surface_biome(&c);
            let idx = (local_x * 16 + local_z) as usize;
            unsafe {
                *out_biomes.add(idx) = b_id;
            }
        }
    }
}

/// Get exact vanilla 1.21/26.x biome at any block position (x, y, z).
#[unsafe(no_mangle)]
pub extern "C" fn fastgen_get_biome(seed: i64, x: i32, y: i32, z: i32) -> u8 {
    let ctx = context::get_or_create_context(seed);
    let mut cache = OverworldColumnCache::new();
    let (c, depth) = ctx.climate_sampler.sample_3d(x as f64, y as f64, z as f64, &mut cache);
    biomes::lookup_biome_3d(&c, depth)
}

/// Compute 16x16x5 multi-noise climate points for chunk (256 columns * 5 floats).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fastgen_compute_climate(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    _c_scale: f64,
    _e_scale: f64,
    _t_scale: f64,
    _h_scale: f64,
    _w_scale: f64,
    out_climate: *mut f32,
) {
    if out_climate.is_null() {
        return;
    }

    let ctx = context::get_or_create_context(seed);
    let world_base_x = chunk_x * 16;
    let world_base_z = chunk_z * 16;

    for local_x in 0..16 {
        for local_z in 0..16 {
            let wx = (world_base_x + local_x) as f64;
            let wz = (world_base_z + local_z) as f64;

            let c = ctx.climate_sampler.sample(wx, wz);
            let base_idx = ((local_x * 16 + local_z) * 5) as usize;

            unsafe {
                *out_climate.add(base_idx) = c.continentalness;
                *out_climate.add(base_idx + 1) = c.erosion;
                *out_climate.add(base_idx + 2) = c.temperature;
                *out_climate.add(base_idx + 3) = c.humidity;
                *out_climate.add(base_idx + 4) = c.weirdness;
            }
        }
    }
}

/// Fast single-column base height lookup used by Paper spawn selector and probes.
/// Computes exact preliminary surface level in < 1 microsecond without generating full chunk blocks.
#[unsafe(no_mangle)]
pub extern "C" fn fastgen_get_base_height(
    seed: i64,
    x: i32,
    z: i32,
    min_y: i32,
    max_y: i32,
) -> i32 {
    let ctx = context::get_or_create_context(seed);
    let mut cache = OverworldColumnCache::new();
    let est = aquifer::preliminary_surface_level::<OverworldNoises>(&ctx.noises, &mut cache, x, z);
    est.clamp(min_y, max_y - 1)
}

/// Fast 16x16 chunk heightmap lookup without allocating 3D chunk blocks.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fastgen_compute_heightmap(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    max_y: i32,
    out_heights: *mut i16,
) {
    if out_heights.is_null() {
        return;
    }
    let ctx = context::get_or_create_context(seed);
    let mut cache = OverworldColumnCache::new();
    let base_x = chunk_x * 16;
    let base_z = chunk_z * 16;

    for local_x in 0..16 {
        for local_z in 0..16 {
            let wx = base_x + local_x;
            let wz = base_z + local_z;
            let est = aquifer::preliminary_surface_level::<OverworldNoises>(&ctx.noises, &mut cache, wx, wz);
            let h = est.clamp(min_y, max_y - 1);
            let idx = (local_x * 16 + local_z) as usize;
            unsafe {
                *out_heights.add(idx) = h as i16;
            }
        }
    }
}

/// 1:1 Complete SteelMC Vanilla Chunk Generation Pipeline.
/// Runs cell-based multi-channel 3D density interpolation, aquifer fluid/barrier simulation,
/// surface rule application, and cave/canyon carvers.
///
/// Output blocks layout: `16 * 16 * height` bytes indexed by `((local_x * 16 + local_z) * height) + (world_y - min_y)`.
/// Output heights: 256 `i16` values with highest non-air block Y per column.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fastgen_generate_chunk(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: i32,
    out_blocks: *mut u8,
    out_heights: *mut i16,
) {
    if out_blocks.is_null() || height <= 0 {
        return;
    }

    let ctx = context::get_or_create_context(seed);
    let noises = &*ctx.noises;
    let surface_system = &*ctx.surface_system;
    let splitter = XoroshiroSplitter::from_seed(seed as u64);

    let chunk_min_x = chunk_x * 16;
    let chunk_min_z = chunk_z * 16;
    let height_usize = height as usize;
    let total_blocks = 16 * 16 * height_usize;

    let blocks_slice = unsafe { std::slice::from_raw_parts_mut(out_blocks, total_blocks) };
    blocks_slice.fill(BLOCK_AIR);

    // Thread-local caches & buffers to eliminate all hot-path heap allocations
    thread_local! {
        static NOISE_CHUNK: std::cell::RefCell<noise_chunk::NoiseChunk<OverworldNoises>> =
            std::cell::RefCell::new(noise_chunk::NoiseChunk::<OverworldNoises>::new(0, 0));
        static COLUMN_CACHE: std::cell::RefCell<OverworldColumnCache> =
            std::cell::RefCell::new(OverworldColumnCache::new());
        static CORNER_CACHE: std::cell::RefCell<OverworldColumnCache> =
            std::cell::RefCell::new(OverworldColumnCache::new());
    }

    COLUMN_CACHE.with(|cache_cell| {
        let mut column_cache = cache_cell.borrow_mut();
        column_cache.init_grid(chunk_min_x, chunk_min_z, noises);

        let mut aquifer = aquifer::Aquifer::<OverworldNoises>::new(
            chunk_min_x,
            chunk_min_z,
            min_y,
            height,
            &splitter,
            noises,
            column_cache.clone(),
        );

        let skip_sampling_above_y = aquifer.skip_sampling_above_y();

        // 2. 3D Noise Chunk Fill
        NOISE_CHUNK.with(|nc_cell| {
            let mut noise_chunk = nc_cell.borrow_mut();
            noise_chunk.reset(chunk_min_x, chunk_min_z);

            noise_chunk.fill(
                noises,
                &mut *column_cache,
                |local_x, world_y, local_z, density, _interpolated, _cache| {
                    let rel_y = (world_y - min_y) as usize;
                    if rel_y >= height_usize {
                        return;
                    }
                    let idx = ((local_x * 16 + local_z) * height_usize) + rel_y;

                    // Fast-path 1: density > 0.0 is ALWAYS solid stone across all dimensions
                    if density > 0.0 {
                        blocks_slice[idx] = BLOCK_STONE;
                        return;
                    }

                    // Fast-path 2: above surface and sea level, negative density is 100% air
                    if world_y > skip_sampling_above_y && world_y >= 63 {
                        return;
                    }

                    // Complex underground cave & aquifer math
                    let world_x = chunk_min_x + local_x as i32;
                    let world_z = chunk_min_z + local_z as i32;
                    match aquifer.compute_substance(noises, world_x, world_y, world_z, density) {
                        aquifer::AquiferResult::Solid => {
                            blocks_slice[idx] = BLOCK_STONE;
                        }
                        aquifer::AquiferResult::Fluid(fluid_id) => {
                            blocks_slice[idx] = fluid_id;
                        }
                        aquifer::AquiferResult::Air => {}
                    }
                },
            );
        });

        // 3. Build Surface Rules
        let condition_noise_values = [
            Cell::new(0.0), Cell::new(0.0), Cell::new(0.0), Cell::new(0.0),
            Cell::new(0.0), Cell::new(0.0), Cell::new(0.0), Cell::new(0.0),
        ];
        let condition_noise_initialized = [
            Cell::new(false), Cell::new(false), Cell::new(false), Cell::new(false),
            Cell::new(false), Cell::new(false), Cell::new(false), Cell::new(false),
        ];
        let condition_cache = surface::SurfaceConditionNoiseCache::new(&condition_noise_values, &condition_noise_initialized);
        let surface_rule_block_states = OverworldNoises::surface_rule_block_states();

        let (nw, ne, sw, se) = CORNER_CACHE.with(|cc_cell| {
            let mut corner_cache = cc_cell.borrow_mut();
            let nw = aquifer::preliminary_surface_level::<OverworldNoises>(noises, &mut *corner_cache, chunk_min_x, chunk_min_z);
            let ne = aquifer::preliminary_surface_level::<OverworldNoises>(noises, &mut *corner_cache, chunk_min_x + 16, chunk_min_z);
            let sw = aquifer::preliminary_surface_level::<OverworldNoises>(noises, &mut *corner_cache, chunk_min_x, chunk_min_z + 16);
            let se = aquifer::preliminary_surface_level::<OverworldNoises>(noises, &mut *corner_cache, chunk_min_x + 16, chunk_min_z + 16);
            (nw, ne, sw, se)
        });

        for local_x in 0..16usize {
            for local_z in 0..16usize {
                let block_x = chunk_min_x + local_x as i32;
                let block_z = chunk_min_z + local_z as i32;
                let col_base = (local_x * 16 + local_z) * height_usize;

                let mut start_height = min_y - 1;
                for rel_y in (0..height_usize).rev() {
                    if blocks_slice[col_base + rel_y] != BLOCK_AIR {
                        start_height = min_y + rel_y as i32;
                        break;
                    }
                }

                if start_height < min_y {
                    continue;
                }

                let surface_depth = surface_system.get_surface_depth(block_x, block_z);
                let surface_secondary = surface_system.get_surface_secondary(block_x, block_z);
                condition_cache.reset();

                let t_x = local_x as f64 / 16.0;
                let t_z = local_z as f64 / 16.0;
                let interp = math::lerp2(t_x, t_z, nw as f64, ne as f64, sw as f64, se as f64);
                let min_surface_level = interp.floor() as i32 + surface_depth - 8;

                let c = ctx.climate_sampler.sample(block_x as f64, block_z as f64);
                let col_biome = biomes::lookup_surface_biome(&c) as u16;

                let mut stone_depth_above: i32 = 0;
                let mut water_height: i32 = i32::MIN;
                let mut next_ceiling_stone_y: i32 = i32::MAX;

                for world_y in (min_y..=start_height).rev() {
                    let rel_y = (world_y - min_y) as usize;
                    let state = blocks_slice[col_base + rel_y];

                    if state == BLOCK_AIR {
                        stone_depth_above = 0;
                        water_height = i32::MIN;
                        continue;
                    }

                    if block::is_liquid(state) {
                        if water_height == i32::MIN {
                            water_height = world_y + 1;
                        }
                        continue;
                    }

                    if next_ceiling_stone_y >= world_y {
                        next_ceiling_stone_y = i32::MIN;
                        for la_y in (min_y - 1..world_y).rev() {
                            if la_y < min_y {
                                next_ceiling_stone_y = la_y + 1;
                                break;
                            }
                            let la_rel = (la_y - min_y) as usize;
                            let la_state = blocks_slice[col_base + la_rel];
                            if la_state == BLOCK_AIR || block::is_liquid(la_state) {
                                next_ceiling_stone_y = la_y + 1;
                                break;
                            }
                        }
                    }

                    stone_depth_above += 1;
                    let stone_depth_below = world_y - next_ceiling_stone_y + 1;

                    if state == BLOCK_STONE {
                        // Deep underground fast-paths:
                        // 1. Below y = -59: bedrock floor transition
                        // 2. Between y = -59 and y <= 0: guaranteed 100% deepslate (surface rules never reach here)
                        // 3. Above y = 8 and deep below surface (depth > 40): surface rules never reach here, stays stone
                        if world_y > 0 && world_y > 8 && stone_depth_above > 40 {
                            continue;
                        }

                        if world_y > -59 && world_y <= 0 {
                            blocks_slice[col_base + rel_y] = BLOCK_DEEPSLATE;
                            continue;
                        }

                        let mut ctx_s = surface::SurfaceRuleContext {
                            block_x,
                            block_z,
                            surface_depth,
                            surface_secondary,
                            min_surface_level,
                            steep: false,
                            block_y: world_y,
                            stone_depth_above,
                            stone_depth_below,
                            water_height,
                            biome_id: Some(col_biome),
                            system: surface_system,
                            condition_noises: &condition_cache,
                            block_states: surface_rule_block_states,
                        };

                        if let Some(new_block) = OverworldNoises::try_apply_surface_rule(&mut ctx_s) {
                            blocks_slice[col_base + rel_y] = new_block;
                        }
                    }
                }
            }
        }

        // 4. Carvers: 1:1 Overworld caves, extra underground caves, and ravines/canyons
        carver::apply_carvers(
            seed,
            chunk_x,
            chunk_z,
            min_y,
            height,
            blocks_slice,
            &mut aquifer,
            noises,
        );
    });

    // 5. Update out_heights after carving surface openings
    if !out_heights.is_null() {
        for local_x in 0..16usize {
            for local_z in 0..16usize {
                let col_base = (local_x * 16 + local_z) * height_usize;
                let mut start_height = min_y - 1;
                for rel_y in (0..height_usize).rev() {
                    if blocks_slice[col_base + rel_y] != BLOCK_AIR {
                        start_height = min_y + rel_y as i32;
                        break;
                    }
                }
                let h_idx = local_x * 16 + local_z;
                unsafe {
                    *out_heights.add(h_idx) = (start_height + 1) as i16;
                }
            }
        }
    }
}

#[derive(Copy, Clone)]
struct SendPtr<T>(pub *mut T);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

/// Batch generate multiple chunks concurrently using native Rayon worker threads.
/// coords: array of `count * 2` i32: [chunk_x0, chunk_z0, chunk_x1, chunk_z1, ...]
/// out_blocks: array of `count` pointers to `16 * 16 * height` byte buffers
/// out_heights: array of `count` pointers to 256 i16 buffers (or null)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fastgen_generate_chunks_batch(
    seed: i64,
    coords: *const i32,
    count: i32,
    min_y: i32,
    height: i32,
    out_blocks: *const *mut u8,
    out_heights: *const *mut i16,
) {
    if coords.is_null() || out_blocks.is_null() || count <= 0 {
        return;
    }

    let coords_slice = unsafe { std::slice::from_raw_parts(coords, (count * 2) as usize) };
    let blocks_ptrs: Vec<SendPtr<u8>> = (0..count as usize)
        .map(|i| SendPtr(unsafe { *out_blocks.add(i) }))
        .collect();
    let heights_ptrs: Option<Vec<SendPtr<i16>>> = if out_heights.is_null() {
        None
    } else {
        Some((0..count as usize).map(|i| SendPtr(unsafe { *out_heights.add(i) })).collect())
    };

    let generate_task = || {
        use rayon::prelude::*;
        (0..count as usize).into_par_iter().for_each(|i| {
            let cx = coords_slice[i * 2];
            let cz = coords_slice[i * 2 + 1];
            let b_ptr = blocks_ptrs[i].0;
            let h_ptr = heights_ptrs.as_ref().map_or(std::ptr::null_mut(), |h| h[i].0);
            unsafe {
                fastgen_generate_chunk(seed, cx, cz, min_y, height, b_ptr, h_ptr);
            }
        });
    };

    if let Some(pool) = threads::get_pool() {
        pool.install(generate_task);
    } else {
        generate_task();
    }
}

/// Batch compute 16x16 biomes across multiple chunks using native Rayon worker threads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fastgen_compute_biomes_batch(
    seed: i64,
    coords: *const i32,
    count: i32,
    out_biomes: *const *mut u8,
) {
    if coords.is_null() || out_biomes.is_null() || count <= 0 {
        return;
    }

    let coords_slice = unsafe { std::slice::from_raw_parts(coords, (count * 2) as usize) };
    let biomes_ptrs: Vec<SendPtr<u8>> = (0..count as usize)
        .map(|i| SendPtr(unsafe { *out_biomes.add(i) }))
        .collect();

    let task = || {
        use rayon::prelude::*;
        (0..count as usize).into_par_iter().for_each(|i| {
            let cx = coords_slice[i * 2];
            let cz = coords_slice[i * 2 + 1];
            let b_ptr = biomes_ptrs[i].0;
            unsafe {
                fastgen_compute_biomes(seed, cx, cz, b_ptr);
            }
        });
    };

    if let Some(pool) = threads::get_pool() {
        pool.install(task);
    } else {
        task();
    }
}

/// Batch compute heightmaps across multiple chunks using native Rayon worker threads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fastgen_compute_heightmaps_batch(
    seed: i64,
    coords: *const i32,
    count: i32,
    min_y: i32,
    max_y: i32,
    out_heights: *const *mut i16,
) {
    if coords.is_null() || out_heights.is_null() || count <= 0 {
        return;
    }

    let coords_slice = unsafe { std::slice::from_raw_parts(coords, (count * 2) as usize) };
    let heights_ptrs: Vec<SendPtr<i16>> = (0..count as usize)
        .map(|i| SendPtr(unsafe { *out_heights.add(i) }))
        .collect();

    let task = || {
        use rayon::prelude::*;
        (0..count as usize).into_par_iter().for_each(|i| {
            let cx = coords_slice[i * 2];
            let cz = coords_slice[i * 2 + 1];
            let h_ptr = heights_ptrs[i].0;
            unsafe {
                fastgen_compute_heightmap(seed, cx, cz, min_y, max_y, h_ptr);
            }
        });
    };

    if let Some(pool) = threads::get_pool() {
        pool.install(task);
    } else {
        task();
    }
}

#[cfg(test)]
mod benches {
    use super::*;

    #[test]
    fn bench_chunk_generation() {
        let seed = 123456789i64;
        let min_y = -64;
        let height = 384;
        let mut blocks = vec![0u8; 16 * 16 * height as usize];
        let mut heights = vec![0i16; 256];

        // Warmup
        unsafe {
            fastgen_generate_chunk(seed, 0, 0, min_y, height, blocks.as_mut_ptr(), heights.as_mut_ptr());
        }

        let runs = 20;
        let start = std::time::Instant::now();
        for i in 0..runs {
            unsafe {
                fastgen_generate_chunk(seed, i, 0, min_y, height, blocks.as_mut_ptr(), heights.as_mut_ptr());
            }
        }
        let elapsed = start.elapsed();
        let per_chunk = elapsed.as_secs_f64() * 1000.0 / runs as f64;
        println!("Rust Native Chunk Gen Speed: {:.3} ms/chunk ({:.1} chunks/sec single-threaded)", per_chunk, 1000.0 / per_chunk);
    }

    #[test]
    fn bench_multithreaded_batch_generation() {
        let threads = threads::set_worker_threads(4);
        println!("Native Rayon worker threads: {}", threads);
        assert!(threads > 0);

        let seed = 123456789i64;
        let min_y = -64;
        let height = 384;
        let count = 16;

        let mut coords = Vec::with_capacity(count * 2);
        let mut block_buffers = Vec::with_capacity(count);
        let mut height_buffers = Vec::with_capacity(count);
        let mut block_ptrs = Vec::with_capacity(count);
        let mut height_ptrs = Vec::with_capacity(count);

        for i in 0..count {
            coords.push(i as i32);
            coords.push(i as i32);
            let mut b = vec![0u8; 16 * 16 * height as usize];
            let mut h = vec![0i16; 256];
            block_ptrs.push(b.as_mut_ptr());
            height_ptrs.push(h.as_mut_ptr());
            block_buffers.push(b);
            height_buffers.push(h);
        }

        let start = std::time::Instant::now();
        unsafe {
            fastgen_generate_chunks_batch(
                seed,
                coords.as_ptr(),
                count as i32,
                min_y,
                height,
                block_ptrs.as_ptr() as *const *mut u8,
                height_ptrs.as_ptr() as *const *mut i16,
            );
        }
        let elapsed = start.elapsed();
        let total_ms = elapsed.as_secs_f64() * 1000.0;
        let per_chunk = total_ms / count as f64;
        println!("Multithreaded Batch ({} chunks on {} threads): {:.2} ms total ({:.2} ms/chunk, {:.1} chunks/sec)",
            count, threads, total_ms, per_chunk, 1000.0 / per_chunk);
    }
}



