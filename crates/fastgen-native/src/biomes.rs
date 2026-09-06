// 1:1 Vanilla 1.21 & SteelMC Multi-Noise Overworld Biome Lookup Engine
// Uses exact flattened R-Tree nearest-neighbor search over 7,594 vanilla climate parameter entries.

use std::cmp::Ordering;
use std::sync::LazyLock;
use crate::climate::Climate5D;

pub const PARAMETER_COUNT: usize = 7;
const CHILDREN_PER_NODE: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Parameter {
    pub min: i64,
    pub max: i64,
}

impl Parameter {
    #[inline(always)]
    pub const fn new(min: i64, max: i64) -> Self {
        Self { min, max }
    }

    #[inline(always)]
    pub const fn span_with(&self, other: Option<&Parameter>) -> Self {
        match other {
            Some(o) => Self {
                min: if self.min < o.min { self.min } else { o.min },
                max: if self.max > o.max { self.max } else { o.max },
            },
            None => *self,
        }
    }
}

#[derive(Clone)]
struct BuildEntry {
    parameter_space: [Parameter; PARAMETER_COUNT],
    biome_id: u8,
}

enum RTreeNode {
    Leaf {
        parameter_space: [Parameter; PARAMETER_COUNT],
        biome_id: u8,
    },
    SubTree {
        parameter_space: [Parameter; PARAMETER_COUNT],
        children: Vec<RTreeNode>,
    },
}

impl RTreeNode {
    fn parameter_space(&self) -> &[Parameter; PARAMETER_COUNT] {
        match self {
            Self::Leaf { parameter_space, .. } | Self::SubTree { parameter_space, .. } => parameter_space,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FlatNode {
    mins: [i64; PARAMETER_COUNT],
    maxs: [i64; PARAMETER_COUNT],
    biome_id: u8,
    children_count: u8,
    _pad: u16,
    children_start: u32,
}

impl FlatNode {
    #[inline(always)]
    fn distance(&self, target: &[i64; PARAMETER_COUNT]) -> i64 {
        let mut d = 0i64;
        for i in 0..PARAMETER_COUNT {
            let di = (target[i] - self.maxs[i])
                .max(self.mins[i] - target[i])
                .max(0);
            d += di * di;
        }
        d
    }

    #[inline(always)]
    const fn is_leaf(&self) -> bool {
        self.children_count == 0
    }
}

fn build_parameter_space(children: &[RTreeNode]) -> [Parameter; PARAMETER_COUNT] {
    let mut bounds: [Option<Parameter>; PARAMETER_COUNT] = [None; PARAMETER_COUNT];
    for child in children {
        let ps = child.parameter_space();
        for d in 0..PARAMETER_COUNT {
            bounds[d] = Some(ps[d].span_with(bounds[d].as_ref()));
        }
    }
    bounds.map(|b| b.expect("bounds initialized"))
}

fn cost(parameter_space: &[Parameter; PARAMETER_COUNT]) -> i64 {
    let mut result = 0i64;
    for p in parameter_space {
        result += (p.max - p.min).abs();
    }
    result
}

fn sort_entries(entries: &mut [BuildEntry], dimension: usize) {
    entries.sort_by(|a, b| {
        for offset in 0..PARAMETER_COUNT {
            let d = (dimension + offset) % PARAMETER_COUNT;
            let center_a = (a.parameter_space[d].min + a.parameter_space[d].max) / 2;
            let center_b = (b.parameter_space[d].min + b.parameter_space[d].max) / 2;
            let cmp = center_a.cmp(&center_b);
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        Ordering::Equal
    });
}

fn sort_bucket_subtrees(
    subtrees: &mut [([Parameter; PARAMETER_COUNT], Vec<BuildEntry>)],
    dimension: usize,
) {
    subtrees.sort_by(|a, b| {
        for offset in 0..PARAMETER_COUNT {
            let d = (dimension + offset) % PARAMETER_COUNT;
            let center_a = (a.0[d].min + a.0[d].max) / 2;
            let center_b = (b.0[d].min + b.0[d].max) / 2;
            let cmp = center_a.abs().cmp(&center_b.abs());
            if cmp != Ordering::Equal {
                return cmp;
            }
        }
        Ordering::Equal
    });
}

fn expected_children_count(total: usize) -> usize {
    let log_base_6 = ((total as f64) - 0.01).ln() / (CHILDREN_PER_NODE as f64).ln();
    (CHILDREN_PER_NODE as f64).powf(log_base_6.floor()) as usize
}

fn snapshot_buckets(entries: &[BuildEntry]) -> (i64, Vec<Vec<BuildEntry>>) {
    let expected = expected_children_count(entries.len());
    let mut buckets = Vec::new();
    let mut total_cost = 0i64;
    let mut start = 0;
    while start < entries.len() {
        let end = (start + expected).min(entries.len());
        let bucket = entries[start..end].to_vec();
        let mut bounds: [Option<Parameter>; PARAMETER_COUNT] = [None; PARAMETER_COUNT];
        for e in &bucket {
            for d in 0..PARAMETER_COUNT {
                bounds[d] = Some(e.parameter_space[d].span_with(bounds[d].as_ref()));
            }
        }
        let ps = bounds.map(|b| b.expect("bounds initialized"));
        total_cost += cost(&ps);
        buckets.push(bucket);
        start = end;
    }
    (total_cost, buckets)
}

fn build_tree(entries: &mut [BuildEntry]) -> RTreeNode {
    assert!(!entries.is_empty());

    if entries.len() == 1 {
        return RTreeNode::Leaf {
            parameter_space: entries[0].parameter_space,
            biome_id: entries[0].biome_id,
        };
    }

    if entries.len() <= CHILDREN_PER_NODE {
        entries.sort_by_key(|e| {
            let mut total: i64 = 0;
            for d in 0..PARAMETER_COUNT {
                let p = &e.parameter_space[d];
                total += ((p.min + p.max) / 2).abs();
            }
            total
        });

        let children: Vec<RTreeNode> = entries
            .iter()
            .map(|e| RTreeNode::Leaf {
                parameter_space: e.parameter_space,
                biome_id: e.biome_id,
            })
            .collect();
        let ps = build_parameter_space(&children);
        return RTreeNode::SubTree {
            parameter_space: ps,
            children,
        };
    }

    let mut min_cost = i64::MAX;
    let mut best_dim = 0;
    let mut best_buckets: Option<Vec<Vec<BuildEntry>>> = None;

    for d in 0..PARAMETER_COUNT {
        sort_entries(entries, d);
        let (bucket_cost, buckets) = snapshot_buckets(entries);
        if min_cost > bucket_cost {
            min_cost = bucket_cost;
            best_dim = d;
            best_buckets = Some(buckets);
        }
    }

    let buckets = best_buckets.expect("at least one dimension");
    let mut bucket_subtrees: Vec<([Parameter; PARAMETER_COUNT], Vec<BuildEntry>)> = buckets
        .into_iter()
        .map(|bucket_entries| {
            let mut bounds: [Option<Parameter>; PARAMETER_COUNT] = [None; PARAMETER_COUNT];
            for e in &bucket_entries {
                for dim in 0..PARAMETER_COUNT {
                    bounds[dim] = Some(e.parameter_space[dim].span_with(bounds[dim].as_ref()));
                }
            }
            let ps = bounds.map(|b| b.expect("bounds initialized"));
            (ps, bucket_entries)
        })
        .collect();

    sort_bucket_subtrees(&mut bucket_subtrees, best_dim);
    let final_children: Vec<RTreeNode> = bucket_subtrees
        .into_iter()
        .map(|(_, mut child_entries)| build_tree(&mut child_entries))
        .collect();

    let ps = build_parameter_space(&final_children);
    RTreeNode::SubTree {
        parameter_space: ps,
        children: final_children,
    }
}

fn flatten_tree(root: RTreeNode) -> Vec<FlatNode> {
    use std::collections::VecDeque;

    let mut nodes: Vec<FlatNode> = Vec::new();
    let mut queue: VecDeque<(Vec<RTreeNode>, Option<u32>)> = VecDeque::new();
    queue.push_back((vec![root], None));

    while let Some((batch, parent_idx)) = queue.pop_front() {
        let batch_start = nodes.len() as u32;
        if let Some(pidx) = parent_idx {
            nodes[pidx as usize].children_start = batch_start;
        }

        for node in batch {
            let flat_idx = nodes.len() as u32;
            match node {
                RTreeNode::Leaf {
                    parameter_space,
                    biome_id,
                } => {
                    nodes.push(FlatNode {
                        mins: parameter_space.map(|p| p.min),
                        maxs: parameter_space.map(|p| p.max),
                        biome_id,
                        children_count: 0,
                        _pad: 0,
                        children_start: 0,
                    });
                }
                RTreeNode::SubTree {
                    parameter_space,
                    children,
                } => {
                    let children_count = children.len() as u8;
                    nodes.push(FlatNode {
                        mins: parameter_space.map(|p| p.min),
                        maxs: parameter_space.map(|p| p.max),
                        biome_id: 255,
                        children_count,
                        _pad: 0,
                        children_start: 0,
                    });
                    queue.push_back((children, Some(flat_idx)));
                }
            }
        }
    }

    nodes
}

#[inline(always)]
fn search_nearest(
    nodes: &[FlatNode],
    node: &FlatNode,
    target: &[i64; PARAMETER_COUNT],
    best_dist: &mut i64,
    best_biome: &mut u8,
) {
    let start = node.children_start as usize;
    let end = start + node.children_count as usize;
    let children = &nodes[start..end];

    for child in children {
        let child_dist = child.distance(target);
        if *best_dist > child_dist {
            if child.is_leaf() {
                *best_dist = child_dist;
                *best_biome = child.biome_id;
            } else {
                search_nearest(nodes, child, target, best_dist, best_biome);
            }
        }
    }
}

pub const VERSION_26_1: i32 = 1;
pub const VERSION_26_2: i32 = 2;

static CURRENT_VERSION: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(VERSION_26_2);

#[inline(always)]
pub fn set_version(version: i32) {
    CURRENT_VERSION.store(version, std::sync::atomic::Ordering::Relaxed);
}

#[inline(always)]
pub fn get_version() -> i32 {
    CURRENT_VERSION.load(std::sync::atomic::Ordering::Relaxed)
}

pub struct OverworldBiomeTree {
    nodes: Vec<FlatNode>,
}

impl OverworldBiomeTree {
    fn new(include_sulfur_caves: bool) -> Self {
        const RAW: &[u8] = include_bytes!("overworld_biomes.bin");
        const ENTRY_SIZE: usize = 32;
        let count = RAW.len() / ENTRY_SIZE;
        let mut entries = Vec::with_capacity(count);

        for i in 0..count {
            let off = i * ENTRY_SIZE;
            let slice = &RAW[off..off + ENTRY_SIZE];
            let biome_id = slice[28];

            if !include_sulfur_caves && biome_id == 45 {
                continue;
            }

            let t_min = i16::from_le_bytes([slice[0], slice[1]]) as i64;
            let t_max = i16::from_le_bytes([slice[2], slice[3]]) as i64;
            let h_min = i16::from_le_bytes([slice[4], slice[5]]) as i64;
            let h_max = i16::from_le_bytes([slice[6], slice[7]]) as i64;
            let c_min = i16::from_le_bytes([slice[8], slice[9]]) as i64;
            let c_max = i16::from_le_bytes([slice[10], slice[11]]) as i64;
            let e_min = i16::from_le_bytes([slice[12], slice[13]]) as i64;
            let e_max = i16::from_le_bytes([slice[14], slice[15]]) as i64;
            let d_min = i16::from_le_bytes([slice[16], slice[17]]) as i64;
            let d_max = i16::from_le_bytes([slice[18], slice[19]]) as i64;
            let w_min = i16::from_le_bytes([slice[20], slice[21]]) as i64;
            let w_max = i16::from_le_bytes([slice[22], slice[23]]) as i64;
            let offset = i32::from_le_bytes([slice[24], slice[25], slice[26], slice[27]]) as i64;

            entries.push(BuildEntry {
                parameter_space: [
                    Parameter::new(t_min, t_max),
                    Parameter::new(h_min, h_max),
                    Parameter::new(c_min, c_max),
                    Parameter::new(e_min, e_max),
                    Parameter::new(d_min, d_max),
                    Parameter::new(w_min, w_max),
                    Parameter::new(offset, offset),
                ],
                biome_id,
            });
        }

        let root = build_tree(&mut entries);
        let nodes = flatten_tree(root);
        Self { nodes }
    }

    #[inline(always)]
    pub fn lookup(&self, target: &[i64; PARAMETER_COUNT]) -> u8 {
        if self.nodes.is_empty() {
            return 34; // Plains default
        }
        let mut best_dist = i64::MAX;
        let mut best_biome = 34;
        search_nearest(&self.nodes, &self.nodes[0], target, &mut best_dist, &mut best_biome);
        best_biome
    }
}

pub static OVERWORLD_TREE_26_1: LazyLock<OverworldBiomeTree> = LazyLock::new(|| OverworldBiomeTree::new(false));
pub static OVERWORLD_TREE_26_2: LazyLock<OverworldBiomeTree> = LazyLock::new(|| OverworldBiomeTree::new(true));

#[inline(always)]
pub fn get_active_tree() -> &'static OverworldBiomeTree {
    if CURRENT_VERSION.load(std::sync::atomic::Ordering::Relaxed) == VERSION_26_1 {
        &OVERWORLD_TREE_26_1
    } else {
        &OVERWORLD_TREE_26_2
    }
}

#[inline(always)]
fn quantize(v: f32) -> i64 {
    (v * 10000.0f32) as i64
}

/// Lookup exact vanilla biome matching climate point at surface (depth = 0).
#[inline]
pub fn lookup_surface_biome(climate: &Climate5D) -> u8 {
    let target = [
        quantize(climate.temperature),
        quantize(climate.humidity),
        quantize(climate.continentalness),
        quantize(climate.erosion),
        0, // Surface depth is 0
        quantize(climate.weirdness),
        0, // Target offset is always 0
    ];
    get_active_tree().lookup(&target)
}

/// Lookup exact vanilla biome matching climate point with 3D depth.
#[inline]
pub fn lookup_biome_3d(climate: &Climate5D, depth: f32) -> u8 {
    let target = [
        quantize(climate.temperature),
        quantize(climate.humidity),
        quantize(climate.continentalness),
        quantize(climate.erosion),
        quantize(depth),
        quantize(climate.weirdness),
        0,
    ];
    get_active_tree().lookup(&target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_26_1_and_26_2_trees() {
        // Sample point inside sulfur caves parameter space:
        // temp: 0.0, hum: 0.0, cont: 0.2, ero: 0.7, depth: 0.5, weird: -1.0, offset: 0.0
        let target = [
            quantize(0.0),
            quantize(0.0),
            quantize(0.2),
            quantize(0.7),
            quantize(0.5),
            quantize(-1.0),
            0,
        ];

        // 26.2 must resolve to 45 (sulfur_caves)
        let biome_26_2 = OVERWORLD_TREE_26_2.lookup(&target);
        assert_eq!(biome_26_2, 45, "26.2 should return sulfur_caves (45)");

        // 26.1 must never return 45 (sulfur_caves was introduced in 26.2)
        let biome_26_1 = OVERWORLD_TREE_26_1.lookup(&target);
        assert_ne!(biome_26_1, 45, "26.1 should never return sulfur_caves (45)");

        // Dynamic version switching check
        set_version(VERSION_26_1);
        assert_eq!(get_version(), VERSION_26_1);
        let climate = Climate5D {
            temperature: 0.0,
            humidity: 0.0,
            continentalness: 0.2,
            erosion: 0.7,
            weirdness: -1.0,
        };
        assert_ne!(lookup_biome_3d(&climate, 0.5), 45);

        set_version(VERSION_26_2);
        assert_eq!(get_version(), VERSION_26_2);
        assert_eq!(lookup_biome_3d(&climate, 0.5), 45);
    }
}
