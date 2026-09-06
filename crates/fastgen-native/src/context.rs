//! Global cached Overworld world generation context.
//! Reuses 1:1 noises, surface systems, and climate samplers across chunks to eliminate
//! multi-millisecond octave allocation & PRNG re-seeding bottlenecks.

use std::sync::{Arc, RwLock};
use crate::climate::OverworldClimateSampler;
use crate::density::{DimensionNoises, NoiseParameters};
use crate::density_functions::overworld::OverworldNoises;
use crate::noise_parameters::get_noise_parameters;
use crate::random::XoroshiroSplitter;
use crate::surface::SurfaceSystem;
use rustc_hash::FxHashMap;

pub struct OverworldContext {
    pub seed: i64,
    pub noises: Arc<OverworldNoises>,
    pub surface_system: Arc<SurfaceSystem>,
    pub noise_params: Arc<FxHashMap<String, NoiseParameters>>,
    pub climate_sampler: Arc<OverworldClimateSampler>,
}

static CONTEXT_CACHE: RwLock<Option<Arc<OverworldContext>>> = RwLock::new(None);

thread_local! {
    static LOCAL_CTX: std::cell::RefCell<Option<Arc<OverworldContext>>> = const { std::cell::RefCell::new(None) };
}

pub fn get_or_create_context(seed: i64) -> Arc<OverworldContext> {
    if let Some(ctx) = LOCAL_CTX.with(|c| {
        let b = c.borrow();
        if let Some(ref ctx) = *b {
            if ctx.seed == seed {
                return Some(Arc::clone(ctx));
            }
        }
        None
    }) {
        return ctx;
    }

    // Scoped read guard: MUST drop before acquiring write lock in get_or_create_context_slow
    {
        if let Ok(guard) = CONTEXT_CACHE.read() {
            if let Some(ref ctx) = *guard {
                if ctx.seed == seed {
                    let res = Arc::clone(ctx);
                    LOCAL_CTX.with(|c| {
                        *c.borrow_mut() = Some(Arc::clone(&res));
                    });
                    return res;
                }
            }
        }
    }

    let ctx = get_or_create_context_slow(seed);
    LOCAL_CTX.with(|c| {
        *c.borrow_mut() = Some(Arc::clone(&ctx));
    });
    ctx
}

fn get_or_create_context_slow(seed: i64) -> Arc<OverworldContext> {

    let mut guard = CONTEXT_CACHE.write().unwrap();
    if let Some(ref ctx) = *guard {
        if ctx.seed == seed {
            return Arc::clone(ctx);
        }
    }

    let splitter = XoroshiroSplitter::from_seed(seed as u64);
    let noise_params = Arc::new(get_noise_parameters());
    let noises = Arc::new(OverworldNoises::create(seed as u64, &splitter, &noise_params));
    let surface_system = Arc::new(SurfaceSystem::new(
        &splitter,
        &noise_params,
        OverworldNoises::surface_noise_ids(),
        OverworldNoises::surface_gradient_ids(),
    ));
    let climate_sampler = Arc::new(OverworldClimateSampler::from_arc(Arc::clone(&noises)));

    let ctx = Arc::new(OverworldContext {
        seed,
        noises,
        surface_system,
        noise_params,
        climate_sampler,
    });
    *guard = Some(Arc::clone(&ctx));
    ctx
}
