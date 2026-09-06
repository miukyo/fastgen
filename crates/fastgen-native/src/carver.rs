//! 1:1 SteelMC Cave & Canyon Carver Engine
//!
//! Mirrors vanilla's `CaveWorldCarver`, `CanyonWorldCarver`, and `apply_carvers`.
//! Operates with full 1:1 Aquifer substance protection so oceans and lakes
//! are never breached or replaced with air.

use crate::aquifer::{Aquifer, AquiferResult};
use crate::block::{
    is_carver_replaceable, BlockStateId, BLOCK_AIR, BLOCK_DIRT, BLOCK_GRASS_BLOCK,
    BLOCK_LAVA, BLOCK_MYCELIUM, BLOCK_PODZOL,
};
use crate::density_functions::overworld::OverworldNoises;
use crate::math::trig;
use crate::random::{LegacyRandom, Random};

/// 16x16xH bitmask tracking carved blocks in chunk to prevent duplicate work.
pub struct CarvingMask {
    words: Vec<u64>,
    height: usize,
}

impl CarvingMask {
    pub fn new(height: usize) -> Self {
        let total_bits = 16 * 16 * height;
        let total_words = (total_bits + 63) / 64;
        Self {
            words: vec![0u64; total_words],
            height,
        }
    }

    #[inline]
    pub fn clear(&mut self, height: usize) {
        self.height = height;
        let total_bits = 16 * 16 * height;
        let total_words = (total_bits + 63) / 64;
        if self.words.len() < total_words {
            self.words.resize(total_words, 0);
        } else {
            self.words[..total_words].fill(0);
        }
    }

    #[inline]
    pub fn set_if_unset(&mut self, local_x: usize, rel_y: usize, local_z: usize) -> bool {
        let bit_idx = (local_x * 16 + local_z) * self.height + rel_y;
        let word_idx = bit_idx / 64;
        let mask = 1u64 << (bit_idx % 64);
        if (self.words[word_idx] & mask) != 0 {
            return false;
        }
        self.words[word_idx] |= mask;
        true
    }
}

/// Vanilla cave/canyon tunnel radius calculation.
#[inline]
pub fn horizontal_tunnel_radius(progress_arg: f32, thickness: f32) -> f64 {
    let radius_offset = trig::sin(f64::from(progress_arg)) * thickness;
    1.5 + f64::from(radius_offset)
}

/// Vanilla's `WorldCarver.canReach` — prunes carver steps that can't touch
/// any block in the given chunk.
#[inline]
pub fn can_reach(
    chunk_min_x: i32,
    chunk_min_z: i32,
    x: f64,
    z: f64,
    current_step: i32,
    total_steps: i32,
    thickness: f32,
) -> bool {
    let x_mid = f64::from(chunk_min_x) + 8.0;
    let z_mid = f64::from(chunk_min_z) + 8.0;
    let xd = x - x_mid;
    let zd = z - z_mid;
    let remaining = f64::from(total_steps - current_step);
    let rr = f64::from(thickness + 2.0_f32 + 16.0_f32);
    xd * xd + zd * zd - remaining * remaining <= rr * rr
}

/// Carve every block inside the given ellipsoid that falls in this chunk.
pub fn carve_ellipsoid<F>(
    chunk_min_x: i32,
    chunk_min_z: i32,
    min_y: i32,
    height: i32,
    blocks: &mut [BlockStateId],
    mask: &mut CarvingMask,
    aquifer: &mut Aquifer<OverworldNoises>,
    noises: &OverworldNoises,
    x: f64,
    y: f64,
    z: f64,
    horizontal_radius: f64,
    vertical_radius: f64,
    mut should_skip: F,
) -> bool
where
    F: FnMut(f64, f64, f64, i32) -> bool,
{
    let middle_x = f64::from(chunk_min_x) + 8.0;
    let middle_z = f64::from(chunk_min_z) + 8.0;
    let max_delta = 16.0 + horizontal_radius * 2.0;
    if (x - middle_x).abs() > max_delta || (z - middle_z).abs() > max_delta {
        return false;
    }

    let min_x_idx = ((x - horizontal_radius).floor() as i32 - chunk_min_x - 1).max(0);
    let max_x_idx = ((x + horizontal_radius).floor() as i32 - chunk_min_x).min(15);
    let min_y_bound = ((y - vertical_radius).floor() as i32 - 1).max(min_y + 1);
    let protected_blocks_on_top = 7;
    let max_y_bound =
        ((y + vertical_radius).floor() as i32 + 1).min(min_y + height - 1 - protected_blocks_on_top);
    let min_z_idx = ((z - horizontal_radius).floor() as i32 - chunk_min_z - 1).max(0);
    let max_z_idx = ((z + horizontal_radius).floor() as i32 - chunk_min_z).min(15);

    if min_x_idx > max_x_idx || min_y_bound > max_y_bound || min_z_idx > max_z_idx {
        return false;
    }

    let height_usize = height as usize;
    let lava_level_y = min_y + 8; // -56 in overworld
    let mut carved = false;

    for x_idx in min_x_idx..=max_x_idx {
        let world_x = chunk_min_x + x_idx;
        let xd = (f64::from(world_x) + 0.5 - x) / horizontal_radius;

        for z_idx in min_z_idx..=max_z_idx {
            let world_z = chunk_min_z + z_idx;
            let zd = (f64::from(world_z) + 0.5 - z) / horizontal_radius;
            if xd * xd + zd * zd >= 1.0 {
                continue;
            }

            let col_base = (x_idx as usize * 16 + z_idx as usize) * height_usize;
            let mut has_grass = false;

            for world_y in (min_y_bound + 1..=max_y_bound).rev() {
                let yd = (f64::from(world_y) - 0.5 - y) / vertical_radius;
                if should_skip(xd, yd, zd, world_y) {
                    continue;
                }

                let rel_y = (world_y - min_y) as usize;
                if rel_y >= height_usize {
                    continue;
                }

                if !mask.set_if_unset(x_idx as usize, rel_y, z_idx as usize) {
                    continue;
                }

                let idx = col_base + rel_y;
                let existing = blocks[idx];

                if existing == BLOCK_GRASS_BLOCK
                    || existing == BLOCK_MYCELIUM
                    || existing == BLOCK_PODZOL
                {
                    has_grass = true;
                }

                if !is_carver_replaceable(existing) {
                    continue;
                }

                let new_state = if world_y <= lava_level_y {
                    BLOCK_LAVA
                } else {
                    match aquifer.compute_substance(noises, world_x, world_y, world_z, 0.0) {
                        AquiferResult::Solid => continue,
                        AquiferResult::Fluid(fluid_id) => fluid_id,
                        AquiferResult::Air => BLOCK_AIR,
                    }
                };

                blocks[idx] = new_state;
                carved = true;

                // Top-material rewrite: when grass is carved and block below is dirt, turn below block into grass
                if has_grass && rel_y > 0 {
                    let below_idx = col_base + rel_y - 1;
                    if blocks[below_idx] == BLOCK_DIRT && new_state == BLOCK_AIR {
                        blocks[below_idx] = BLOCK_GRASS_BLOCK;
                    }
                }
            }
        }
    }

    carved
}

#[derive(Debug, Clone, Copy)]
struct TunnelState {
    x: f64,
    y: f64,
    z: f64,
    horizontal_rotation: f32,
    vertical_rotation: f32,
}

#[derive(Debug, Clone, Copy)]
struct TunnelParams {
    tunnel_seed: i64,
    horizontal_radius_multiplier: f64,
    vertical_radius_multiplier: f64,
    thickness: f32,
    step: i32,
    dist: i32,
    y_scale: f64,
}

fn create_tunnel<S>(
    chunk_min_x: i32,
    chunk_min_z: i32,
    min_y: i32,
    height: i32,
    blocks: &mut [BlockStateId],
    mask: &mut CarvingMask,
    aquifer: &mut Aquifer<OverworldNoises>,
    noises: &OverworldNoises,
    mut state: TunnelState,
    tunnel: TunnelParams,
    skip_checker: S,
) where
    S: Fn(f64, f64, f64, i32) -> bool + Copy,
{
    let mut random = LegacyRandom::from_seed(tunnel.tunnel_seed as u64);
    let split_point = random.next_i32_bounded(tunnel.dist / 2) + tunnel.dist / 4;
    let steep = random.next_i32_bounded(6) == 0;
    let mut y_rota: f32 = 0.0;
    let mut x_rota: f32 = 0.0;

    for current_step in tunnel.step..tunnel.dist {
        let progress_arg = std::f32::consts::PI * current_step as f32 / tunnel.dist as f32;
        let horizontal_radius = horizontal_tunnel_radius(progress_arg, tunnel.thickness);
        let vertical_radius = horizontal_radius * tunnel.y_scale;
        let cos_x = trig::cos(f64::from(state.vertical_rotation));
        state.x += f64::from(trig::cos(f64::from(state.horizontal_rotation)) * cos_x);
        state.y += f64::from(trig::sin(f64::from(state.vertical_rotation)));
        state.z += f64::from(trig::sin(f64::from(state.horizontal_rotation)) * cos_x);
        state.vertical_rotation *= if steep { 0.92 } else { 0.7 };
        state.vertical_rotation += x_rota * 0.1;
        state.horizontal_rotation += y_rota * 0.1;
        x_rota *= 0.9;
        y_rota *= 0.75;
        x_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 2.0;
        y_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 4.0;

        if current_step == split_point && tunnel.thickness > 1.0 {
            let sub_seed_a = random.next_i64();
            let sub_thickness_a = random.next_f32() * 0.5 + 0.5;
            let sub_state_a = TunnelState {
                horizontal_rotation: state.horizontal_rotation - std::f32::consts::FRAC_PI_2,
                vertical_rotation: state.vertical_rotation / 3.0,
                ..state
            };
            let sub_seed_b = random.next_i64();
            let sub_thickness_b = random.next_f32() * 0.5 + 0.5;
            let sub_state_b = TunnelState {
                horizontal_rotation: state.horizontal_rotation + std::f32::consts::FRAC_PI_2,
                vertical_rotation: state.vertical_rotation / 3.0,
                ..state
            };
            let sub_tunnel_a = TunnelParams {
                tunnel_seed: sub_seed_a,
                thickness: sub_thickness_a,
                step: current_step,
                y_scale: 1.0,
                ..tunnel
            };
            let sub_tunnel_b = TunnelParams {
                tunnel_seed: sub_seed_b,
                thickness: sub_thickness_b,
                step: current_step,
                y_scale: 1.0,
                ..tunnel
            };
            create_tunnel(
                chunk_min_x,
                chunk_min_z,
                min_y,
                height,
                blocks,
                mask,
                aquifer,
                noises,
                sub_state_a,
                sub_tunnel_a,
                skip_checker,
            );
            create_tunnel(
                chunk_min_x,
                chunk_min_z,
                min_y,
                height,
                blocks,
                mask,
                aquifer,
                noises,
                sub_state_b,
                sub_tunnel_b,
                skip_checker,
            );
            return;
        }

        if random.next_i32_bounded(4) == 0 {
            continue;
        }

        if !can_reach(
            chunk_min_x,
            chunk_min_z,
            state.x,
            state.z,
            current_step,
            tunnel.dist,
            tunnel.thickness,
        ) {
            return;
        }

        carve_ellipsoid(
            chunk_min_x,
            chunk_min_z,
            min_y,
            height,
            blocks,
            mask,
            aquifer,
            noises,
            state.x,
            state.y,
            state.z,
            horizontal_radius * tunnel.horizontal_radius_multiplier,
            vertical_radius * tunnel.vertical_radius_multiplier,
            skip_checker,
        );
    }
}

pub fn carve_cave(
    source_pos_x: i32,
    source_pos_z: i32,
    chunk_min_x: i32,
    chunk_min_z: i32,
    min_y: i32,
    height: i32,
    blocks: &mut [BlockStateId],
    mask: &mut CarvingMask,
    aquifer: &mut Aquifer<OverworldNoises>,
    noises: &OverworldNoises,
    random: &mut LegacyRandom,
    extra_underground: bool,
) {
    let bound = 15;
    let inner = random.next_i32_bounded(bound);
    let mid = random.next_i32_bounded(inner + 1);
    let cave_count = random.next_i32_bounded(mid + 1);

    let source_min_x = source_pos_x * 16;
    let source_min_z = source_pos_z * 16;

    for _ in 0..cave_count {
        let x = f64::from(source_min_x + random.next_i32_bounded(16));
        let y = if extra_underground {
            let min_c = min_y + 8;
            let max_c = 47;
            let span = (max_c - min_c + 1).max(1);
            f64::from(min_c + random.next_i32_bounded(span))
        } else {
            let min_c = min_y + 8;
            let max_c = 180;
            let span = (max_c - min_c + 1).max(1);
            f64::from(min_c + random.next_i32_bounded(span))
        };
        let z = f64::from(source_min_z + random.next_i32_bounded(16));

        let horizontal_radius_multiplier = f64::from(0.7 + random.next_f32() * 0.7);
        let vertical_radius_multiplier = f64::from(0.8 + random.next_f32() * 0.5);
        let floor_level = f64::from(-1.0 + random.next_f32() * 0.6);

        let skip_checker = move |xd: f64, yd: f64, zd: f64, _world_y: i32| {
            yd <= floor_level || xd * xd + yd * yd + zd * zd >= 1.0
        };

        let mut tunnels = 1i32;
        if random.next_i32_bounded(4) == 0 {
            let y_scale = f64::from(0.1 + random.next_f32() * 0.8);
            let thickness = 1.0 + random.next_f32() * 6.0;
            let horizontal_radius =
                1.5 + f64::from(trig::sin(f64::from(std::f32::consts::FRAC_PI_2))) * f64::from(thickness);
            let vertical_radius = horizontal_radius * y_scale;
            carve_ellipsoid(
                chunk_min_x,
                chunk_min_z,
                min_y,
                height,
                blocks,
                mask,
                aquifer,
                noises,
                x + 1.0,
                y,
                z,
                horizontal_radius,
                vertical_radius,
                skip_checker,
            );
            tunnels += random.next_i32_bounded(4);
        }

        for _ in 0..tunnels {
            let state = TunnelState {
                x,
                y,
                z,
                horizontal_rotation: random.next_f32() * std::f32::consts::TAU,
                vertical_rotation: (random.next_f32() - 0.5) / 4.0,
            };
            let thickness = {
                let mut t = random.next_f32() * 2.0 + random.next_f32();
                if random.next_i32_bounded(10) == 0 {
                    t *= random.next_f32() * random.next_f32() * 3.0 + 1.0;
                }
                t
            };
            let dist = 112 - random.next_i32_bounded(112 / 4);
            let tunnel_seed = random.next_i64();
            let tunnel = TunnelParams {
                tunnel_seed,
                horizontal_radius_multiplier,
                vertical_radius_multiplier,
                thickness,
                step: 0,
                dist,
                y_scale: 1.0,
            };
            create_tunnel(
                chunk_min_x,
                chunk_min_z,
                min_y,
                height,
                blocks,
                mask,
                aquifer,
                noises,
                state,
                tunnel,
                skip_checker,
            );
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct CanyonState {
    x: f64,
    y: f64,
    z: f64,
    horizontal_rotation: f32,
    vertical_rotation: f32,
}

#[derive(Debug, Clone, Copy)]
struct CanyonTunnel {
    tunnel_seed: i64,
    thickness: f32,
    distance: i32,
    y_scale: f64,
}

fn do_carve_canyon(
    chunk_min_x: i32,
    chunk_min_z: i32,
    min_y: i32,
    height: i32,
    blocks: &mut [BlockStateId],
    mask: &mut CarvingMask,
    aquifer: &mut Aquifer<OverworldNoises>,
    noises: &OverworldNoises,
    mut state: CanyonState,
    tunnel: CanyonTunnel,
) {
    let mut random = LegacyRandom::from_seed(tunnel.tunnel_seed as u64);
    let depth = height as usize;
    let mut width_factors = vec![0.0_f32; depth];
    let mut current = 1.0_f32;
    for (y_index, slot) in width_factors.iter_mut().enumerate() {
        if y_index == 0 || random.next_i32_bounded(3) == 0 {
            current = 1.0 + random.next_f32() * random.next_f32();
        }
        *slot = current * current;
    }

    let mut y_rota: f32 = 0.0;
    let mut x_rota: f32 = 0.0;

    for current_step in 0..tunnel.distance {
        let progress = std::f32::consts::PI * current_step as f32 / tunnel.distance as f32;
        let mut horizontal_radius = horizontal_tunnel_radius(progress, tunnel.thickness);
        let mut vertical_radius = horizontal_radius * tunnel.y_scale;
        horizontal_radius *= f64::from(0.75 + random.next_f32() * 0.25);

        let vertical_multiplier =
            1.0_f32 - (0.5 - current_step as f32 / tunnel.distance as f32).abs() * 2.0;
        let factor = 1.0_f32 + 0.0_f32 * vertical_multiplier;
        let jitter = 0.75 + random.next_f32() * 0.25;
        vertical_radius = f64::from(factor) * vertical_radius * f64::from(jitter);

        let xc = trig::cos(f64::from(state.vertical_rotation));
        let xs = trig::sin(f64::from(state.vertical_rotation));
        state.x += f64::from(trig::cos(f64::from(state.horizontal_rotation)) * xc);
        state.y += f64::from(xs);
        state.z += f64::from(trig::sin(f64::from(state.horizontal_rotation)) * xc);
        state.vertical_rotation *= 0.7;
        state.vertical_rotation += x_rota * 0.05;
        state.horizontal_rotation += y_rota * 0.05;
        x_rota *= 0.8;
        y_rota *= 0.5;
        x_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 2.0;
        y_rota += (random.next_f32() - random.next_f32()) * random.next_f32() * 4.0;

        if random.next_i32_bounded(4) == 0 {
            continue;
        }

        if !can_reach(
            chunk_min_x,
            chunk_min_z,
            state.x,
            state.z,
            current_step,
            tunnel.distance,
            tunnel.thickness,
        ) {
            return;
        }

        let skip_checker = |xd: f64, yd: f64, zd: f64, world_y: i32| {
            let y_index = (world_y - min_y - 1).clamp(0, height - 1) as usize;
            let factor = width_factors[y_index];
            (xd * xd + zd * zd) * f64::from(factor) + yd * yd / 6.0 >= 1.0
        };

        carve_ellipsoid(
            chunk_min_x,
            chunk_min_z,
            min_y,
            height,
            blocks,
            mask,
            aquifer,
            noises,
            state.x,
            state.y,
            state.z,
            horizontal_radius,
            vertical_radius,
            skip_checker,
        );
    }
}

pub fn carve_canyon(
    source_pos_x: i32,
    source_pos_z: i32,
    chunk_min_x: i32,
    chunk_min_z: i32,
    min_y: i32,
    height: i32,
    blocks: &mut [BlockStateId],
    mask: &mut CarvingMask,
    aquifer: &mut Aquifer<OverworldNoises>,
    noises: &OverworldNoises,
    random: &mut LegacyRandom,
) {
    let source_min_x = source_pos_x * 16;
    let source_min_z = source_pos_z * 16;

    let state = CanyonState {
        x: f64::from(source_min_x + random.next_i32_bounded(16)),
        y: f64::from(10 + random.next_i32_bounded(67 - 10 + 1)),
        z: f64::from(source_min_z + random.next_i32_bounded(16)),
        horizontal_rotation: random.next_f32() * std::f32::consts::TAU,
        vertical_rotation: -0.125 + random.next_f32() * 0.25,
    };

    let y_scale = 3.0f64;
    // Trapezoid thickness: min 0.0, max 6.0, plateau 2.0
    let thickness = {
        let min = 0.0f32;
        let max = 6.0f32;
        let plateau = 2.0f32;
        let diff = max - min;
        let flat = (diff - plateau) / 2.0;
        let r1 = random.next_f32();
        let r2 = random.next_f32();
        min + r1 * flat + r2 * flat + plateau
    };
    let distance_factor = 0.75 + random.next_f32() * 0.25;
    let distance = (112.0f32 * distance_factor) as i32;
    let tunnel_seed = random.next_i64();

    let tunnel = CanyonTunnel {
        tunnel_seed,
        thickness,
        distance,
        y_scale,
    };

    do_carve_canyon(
        chunk_min_x,
        chunk_min_z,
        min_y,
        height,
        blocks,
        mask,
        aquifer,
        noises,
        state,
        tunnel,
    );
}

thread_local! {
    static CARVING_MASK: std::cell::RefCell<CarvingMask> = std::cell::RefCell::new(CarvingMask::new(384));
}

/// Applies all overworld carvers across the 17x17 source chunk grid.
pub fn apply_carvers(
    seed: i64,
    chunk_x: i32,
    chunk_z: i32,
    min_y: i32,
    height: i32,
    blocks: &mut [BlockStateId],
    aquifer: &mut Aquifer<OverworldNoises>,
    noises: &OverworldNoises,
) {
    let chunk_min_x = chunk_x * 16;
    let chunk_min_z = chunk_z * 16;
    let mut random = LegacyRandom::from_seed(0);

    CARVING_MASK.with(|mask_cell| {
        let mut mask = mask_cell.borrow_mut();
        mask.clear(height as usize);

        for dx in -8..=8 {
            for dz in -8..=8 {
                let sx = chunk_x + dx;
                let sz = chunk_z + dz;

            // Carver index 0: minecraft:cave (probability 0.15)
            random.set_large_feature_seed(seed, sx, sz);
            if random.next_f32() <= 0.15 {
                carve_cave(
                    sx,
                    sz,
                    chunk_min_x,
                    chunk_min_z,
                    min_y,
                    height,
                    blocks,
                    &mut mask,
                    aquifer,
                    noises,
                    &mut random,
                    false,
                );
            }

            // Carver index 1: minecraft:cave_extra_underground (probability 0.07)
            random.set_large_feature_seed(seed.wrapping_add(1), sx, sz);
            if random.next_f32() <= 0.07 {
                carve_cave(
                    sx,
                    sz,
                    chunk_min_x,
                    chunk_min_z,
                    min_y,
                    height,
                    blocks,
                    &mut mask,
                    aquifer,
                    noises,
                    &mut random,
                    true,
                );
            }

            // Carver index 2: minecraft:canyon (probability 0.01)
            random.set_large_feature_seed(seed.wrapping_add(2), sx, sz);
            if random.next_f32() <= 0.01 {
                carve_canyon(
                    sx,
                    sz,
                    chunk_min_x,
                    chunk_min_z,
                    min_y,
                    height,
                    blocks,
                    &mut mask,
                    aquifer,
                    noises,
                    &mut random,
                );
            }
        }
    }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{BLOCK_AIR, BLOCK_STONE, BLOCK_WATER};
    use crate::density_functions::overworld::{OverworldColumnCache, OverworldNoises};
    use crate::noise_parameters::get_noise_parameters;
    use crate::random::Xoroshiro;

    #[test]
    fn test_carver_replaceable_blocks() {
        assert!(is_carver_replaceable(BLOCK_STONE));
        assert!(!is_carver_replaceable(BLOCK_WATER), "Water must NOT be replaceable by carver");
        assert!(!is_carver_replaceable(BLOCK_AIR), "Air must NOT be replaceable by carver");
    }

    #[test]
    fn test_carver_never_replaces_water() {
        let seed = 12345i64;
        let mut rng = Xoroshiro::from_seed(seed as u64);
        let splitter = rng.next_positional();
        let noise_params = get_noise_parameters();
        let noises = OverworldNoises::create(seed as u64, &splitter, &noise_params);

        let min_y = -64;
        let height = 384;
        let mut column_cache = OverworldColumnCache::new();
        column_cache.init_grid(0, 0, &noises);

        let mut aquifer = Aquifer::<OverworldNoises>::new(
            0,
            0,
            min_y,
            height,
            &splitter,
            &noises,
            column_cache,
        );

        let mut blocks = vec![BLOCK_WATER; 16 * 16 * height as usize];
        apply_carvers(seed, 0, 0, min_y, height, &mut blocks, &mut aquifer, &noises);

        // Every block must still be WATER! Carver must never touch water.
        for &b in &blocks {
            assert_eq!(b, BLOCK_WATER);
        }
    }

    #[test]
    fn test_carver_carves_caves_into_stone() {
        let seed = 12345i64;
        let mut rng = Xoroshiro::from_seed(seed as u64);
        let splitter = rng.next_positional();
        let noise_params = get_noise_parameters();
        let noises = OverworldNoises::create(seed as u64, &splitter, &noise_params);

        let min_y = -64;
        let height = 384;
        let mut column_cache = OverworldColumnCache::new();
        column_cache.init_grid(0, 0, &noises);

        let mut aquifer = Aquifer::<OverworldNoises>::new(
            0,
            0,
            min_y,
            height,
            &splitter,
            &noises,
            column_cache,
        );

        let mut blocks = vec![BLOCK_STONE; 16 * 16 * height as usize];
        apply_carvers(seed, 0, 0, min_y, height, &mut blocks, &mut aquifer, &noises);

        let mut air_count = 0;
        let mut stone_count = 0;
        let mut lava_count = 0;
        for &b in &blocks {
            if b == BLOCK_AIR {
                air_count += 1;
            } else if b == BLOCK_STONE {
                stone_count += 1;
            } else if b == BLOCK_LAVA {
                lava_count += 1;
            }
        }

        assert!(air_count > 0, "Carver must carve air tunnels in chunk (found {})", air_count);
        assert!(stone_count > 0, "Chunk must retain stone walls (found {})", stone_count);
    }
}
