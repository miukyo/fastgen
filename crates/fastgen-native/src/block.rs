//! Block state and vanilla registry representations matching vanilla 1.21 & SteelMC.

pub type BlockStateId = u8;

pub const BLOCK_BEDROCK: BlockStateId = 0;
pub const BLOCK_COARSE_DIRT: BlockStateId = 1;
pub const BLOCK_GRASS_BLOCK: BlockStateId = 2;
pub const BLOCK_DIRT: BlockStateId = 3;
pub const BLOCK_WATER: BlockStateId = 4;
pub const BLOCK_ORANGE_TERRACOTTA: BlockStateId = 5;
pub const BLOCK_TERRACOTTA: BlockStateId = 6;
pub const BLOCK_RED_SANDSTONE: BlockStateId = 7;
pub const BLOCK_RED_SAND: BlockStateId = 8;
pub const BLOCK_WHITE_TERRACOTTA: BlockStateId = 9;
pub const BLOCK_STONE: BlockStateId = 10;
pub const BLOCK_GRAVEL: BlockStateId = 11;
pub const BLOCK_AIR: BlockStateId = 12;
pub const BLOCK_ICE: BlockStateId = 13;
pub const BLOCK_PACKED_ICE: BlockStateId = 14;
pub const BLOCK_SNOW_BLOCK: BlockStateId = 15;
pub const BLOCK_POWDER_SNOW: BlockStateId = 16;
pub const BLOCK_CALCITE: BlockStateId = 17;
pub const BLOCK_SANDSTONE: BlockStateId = 18;
pub const BLOCK_SAND: BlockStateId = 19;
pub const BLOCK_PODZOL: BlockStateId = 20;
pub const BLOCK_MUD: BlockStateId = 21;
pub const BLOCK_MYCELIUM: BlockStateId = 22;
pub const BLOCK_DEEPSLATE: BlockStateId = 23;
pub const BLOCK_LAVA: BlockStateId = 24;
pub const BLOCK_YELLOW_TERRACOTTA: BlockStateId = 25;
pub const BLOCK_BROWN_TERRACOTTA: BlockStateId = 26;
pub const BLOCK_RED_TERRACOTTA: BlockStateId = 27;
pub const BLOCK_LIGHT_GRAY_TERRACOTTA: BlockStateId = 28;

#[inline(always)]
pub fn is_air(id: BlockStateId) -> bool {
    id == BLOCK_AIR
}

#[inline(always)]
pub fn is_liquid(id: BlockStateId) -> bool {
    id == BLOCK_WATER || id == BLOCK_LAVA
}

#[inline(always)]
pub fn is_carver_replaceable(id: BlockStateId) -> bool {
    matches!(
        id,
        BLOCK_STONE
            | BLOCK_DEEPSLATE
            | BLOCK_DIRT
            | BLOCK_GRASS_BLOCK
            | BLOCK_COARSE_DIRT
            | BLOCK_GRAVEL
            | BLOCK_SANDSTONE
            | BLOCK_RED_SANDSTONE
            | BLOCK_CALCITE
            | BLOCK_PODZOL
            | BLOCK_MUD
            | BLOCK_MYCELIUM
            | BLOCK_TERRACOTTA
            | BLOCK_ORANGE_TERRACOTTA
            | BLOCK_WHITE_TERRACOTTA
            | BLOCK_YELLOW_TERRACOTTA
            | BLOCK_BROWN_TERRACOTTA
            | BLOCK_RED_TERRACOTTA
            | BLOCK_LIGHT_GRAY_TERRACOTTA
            | BLOCK_SAND
            | BLOCK_RED_SAND
            | BLOCK_PACKED_ICE
            | BLOCK_SNOW_BLOCK
    )
}

pub mod vanilla_blocks {
    use super::*;

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub struct Block(pub BlockStateId);

    impl Block {
        #[inline(always)]
        pub const fn default_state(&self) -> BlockStateId {
            self.0
        }
    }

    pub const BEDROCK: Block = Block(BLOCK_BEDROCK);
    pub const COARSE_DIRT: Block = Block(BLOCK_COARSE_DIRT);
    pub const GRASS_BLOCK: Block = Block(BLOCK_GRASS_BLOCK);
    pub const DIRT: Block = Block(BLOCK_DIRT);
    pub const WATER: Block = Block(BLOCK_WATER);
    pub const ORANGE_TERRACOTTA: Block = Block(BLOCK_ORANGE_TERRACOTTA);
    pub const TERRACOTTA: Block = Block(BLOCK_TERRACOTTA);
    pub const RED_SANDSTONE: Block = Block(BLOCK_RED_SANDSTONE);
    pub const RED_SAND: Block = Block(BLOCK_RED_SAND);
    pub const WHITE_TERRACOTTA: Block = Block(BLOCK_WHITE_TERRACOTTA);
    pub const STONE: Block = Block(BLOCK_STONE);
    pub const GRAVEL: Block = Block(BLOCK_GRAVEL);
    pub const AIR: Block = Block(BLOCK_AIR);
    pub const ICE: Block = Block(BLOCK_ICE);
    pub const PACKED_ICE: Block = Block(BLOCK_PACKED_ICE);
    pub const SNOW_BLOCK: Block = Block(BLOCK_SNOW_BLOCK);
    pub const POWDER_SNOW: Block = Block(BLOCK_POWDER_SNOW);
    pub const CALCITE: Block = Block(BLOCK_CALCITE);
    pub const SANDSTONE: Block = Block(BLOCK_SANDSTONE);
    pub const SAND: Block = Block(BLOCK_SAND);
    pub const PODZOL: Block = Block(BLOCK_PODZOL);
    pub const MUD: Block = Block(BLOCK_MUD);
    pub const MYCELIUM: Block = Block(BLOCK_MYCELIUM);
    pub const DEEPSLATE: Block = Block(BLOCK_DEEPSLATE);
    pub const LAVA: Block = Block(BLOCK_LAVA);
    pub const YELLOW_TERRACOTTA: Block = Block(BLOCK_YELLOW_TERRACOTTA);
    pub const BROWN_TERRACOTTA: Block = Block(BLOCK_BROWN_TERRACOTTA);
    pub const RED_TERRACOTTA: Block = Block(BLOCK_RED_TERRACOTTA);
    pub const LIGHT_GRAY_TERRACOTTA: Block = Block(BLOCK_LIGHT_GRAY_TERRACOTTA);
}

pub mod block_state_ext {
    use super::BlockStateId;

    pub trait BlockStateExt {
        fn has_fluid(&self) -> bool;
    }

    impl BlockStateExt for BlockStateId {
        #[inline(always)]
        fn has_fluid(&self) -> bool {
            super::is_liquid(*self)
        }
    }
}

pub use block_state_ext::BlockStateExt;

pub trait RegistryEntry {
    fn id(&self) -> usize;
}

pub trait RegistryExt {}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Biome(pub u16);

impl RegistryEntry for Biome {
    #[inline(always)]
    fn id(&self) -> usize {
        self.0 as usize
    }
}

impl std::ops::Deref for Biome {
    type Target = Biome;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self
    }
}

pub mod vanilla_biomes {
    use super::Biome;

    pub static BADLANDS: Biome = Biome(0);
    pub static BAMBOO_JUNGLE: Biome = Biome(1);
    pub static BEACH: Biome = Biome(2);
    pub static BIRCH_FOREST: Biome = Biome(3);
    pub static CHERRY_GROVE: Biome = Biome(4);
    pub static COLD_OCEAN: Biome = Biome(5);
    pub static DARK_FOREST: Biome = Biome(6);
    pub static DEEP_COLD_OCEAN: Biome = Biome(7);
    pub static DEEP_DARK: Biome = Biome(8);
    pub static DEEP_FROZEN_OCEAN: Biome = Biome(9);
    pub static DEEP_LUKEWARM_OCEAN: Biome = Biome(10);
    pub static DEEP_OCEAN: Biome = Biome(11);
    pub static DESERT: Biome = Biome(12);
    pub static DRIPSTONE_CAVES: Biome = Biome(13);
    pub static ERODED_BADLANDS: Biome = Biome(14);
    pub static FLOWER_FOREST: Biome = Biome(15);
    pub static FOREST: Biome = Biome(16);
    pub static FROZEN_OCEAN: Biome = Biome(17);
    pub static FROZEN_PEAKS: Biome = Biome(18);
    pub static FROZEN_RIVER: Biome = Biome(19);
    pub static GROVE: Biome = Biome(20);
    pub static ICE_SPIKES: Biome = Biome(21);
    pub static JAGGED_PEAKS: Biome = Biome(22);
    pub static JUNGLE: Biome = Biome(23);
    pub static LUKEWARM_OCEAN: Biome = Biome(24);
    pub static LUSH_CAVES: Biome = Biome(25);
    pub static MANGROVE_SWAMP: Biome = Biome(26);
    pub static MEADOW: Biome = Biome(27);
    pub static MUSHROOM_FIELDS: Biome = Biome(28);
    pub static OCEAN: Biome = Biome(29);
    pub static OLD_GROWTH_BIRCH_FOREST: Biome = Biome(30);
    pub static OLD_GROWTH_PINE_TAIGA: Biome = Biome(31);
    pub static OLD_GROWTH_SPRUCE_TAIGA: Biome = Biome(32);
    pub static PALE_GARDEN: Biome = Biome(33);
    pub static PLAINS: Biome = Biome(34);
    pub static RIVER: Biome = Biome(35);
    pub static SAVANNA: Biome = Biome(36);
    pub static SAVANNA_PLATEAU: Biome = Biome(37);
    pub static SNOWY_BEACH: Biome = Biome(38);
    pub static SNOWY_PLAINS: Biome = Biome(39);
    pub static SNOWY_SLOPES: Biome = Biome(40);
    pub static SNOWY_TAIGA: Biome = Biome(41);
    pub static SPARSE_JUNGLE: Biome = Biome(42);
    pub static STONY_PEAKS: Biome = Biome(43);
    pub static STONY_SHORE: Biome = Biome(44);
    pub static SULFUR_CAVES: Biome = Biome(45);
    pub static SUNFLOWER_PLAINS: Biome = Biome(46);
    pub static SWAMP: Biome = Biome(47);
    pub static TAIGA: Biome = Biome(48);
    pub static WARM_OCEAN: Biome = Biome(49);
    pub static WINDSWEPT_FOREST: Biome = Biome(50);
    pub static WINDSWEPT_GRAVELLY_HILLS: Biome = Biome(51);
    pub static WINDSWEPT_HILLS: Biome = Biome(52);
    pub static WINDSWEPT_SAVANNA: Biome = Biome(53);
    pub static WOODED_BADLANDS: Biome = Biome(54);
}

pub struct RegistryBlocks;
impl RegistryBlocks {
    #[inline(always)]
    pub fn get_default_state_id(&self, block: &vanilla_blocks::Block) -> BlockStateId {
        block.default_state()
    }
}

pub struct RegistryHolder {
    pub blocks: RegistryBlocks,
}

pub static REGISTRY: RegistryHolder = RegistryHolder {
    blocks: RegistryBlocks,
};
