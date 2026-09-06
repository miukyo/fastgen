package com.fastgen.generator;

import com.fastgen.nativebridge.FastGenBindings;
import org.bukkit.block.Biome;
import org.bukkit.generator.BiomeProvider;
import org.bukkit.generator.WorldInfo;

import java.util.Arrays;
import java.util.List;

/**
 * 1:1 Vanilla 1.21 & SteelMC Biome Provider.
 * Driven directly by the native Double-Perlin climate sampler and flattened R-Tree
 * searching all 7,594 vanilla multi-noise climate entries.
 */
public class FastGenBiomeProvider extends BiomeProvider {

    public static Biome resolveBiome(String name, Biome fallback) {
        try {
            org.bukkit.NamespacedKey key = org.bukkit.NamespacedKey.minecraft(name.toLowerCase());
            Biome b = org.bukkit.Registry.BIOME.get(key);
            if (b != null) {
                return b;
            }
        } catch (Throwable ignored) {}

        try {
            return Biome.valueOf(name.toUpperCase());
        } catch (Throwable ignored) {}

        try {
            java.lang.reflect.Field f = Biome.class.getField(name.toUpperCase());
            Object val = f.get(null);
            if (val instanceof Biome b) {
                return b;
            }
        } catch (Throwable ignored) {}

        return fallback;
    }

    public static void initBiomes() {
        Biome sulfur = resolveBiome("SULFUR_CAVES", null);
        if (sulfur != null) {
            VANILLA_BIOMES[45] = sulfur;
        }
        Biome paleGarden = resolveBiome("PALE_GARDEN", null);
        if (paleGarden != null) {
            VANILLA_BIOMES[33] = paleGarden;
        }
    }

    public static final Biome[] VANILLA_BIOMES = new Biome[] {
            Biome.BADLANDS,                     // 0
            Biome.BAMBOO_JUNGLE,                // 1
            Biome.BEACH,                        // 2
            Biome.BIRCH_FOREST,                 // 3
            Biome.CHERRY_GROVE,                 // 4
            Biome.COLD_OCEAN,                   // 5
            Biome.DARK_FOREST,                  // 6
            Biome.DEEP_COLD_OCEAN,              // 7
            Biome.DEEP_DARK,                    // 8
            Biome.DEEP_FROZEN_OCEAN,            // 9
            Biome.DEEP_LUKEWARM_OCEAN,          // 10
            Biome.DEEP_OCEAN,                   // 11
            Biome.DESERT,                       // 12
            Biome.DRIPSTONE_CAVES,              // 13
            Biome.ERODED_BADLANDS,              // 14
            Biome.FLOWER_FOREST,                // 15
            Biome.FOREST,                       // 16
            Biome.FROZEN_OCEAN,                 // 17
            Biome.FROZEN_PEAKS,                 // 18
            Biome.FROZEN_RIVER,                 // 19
            Biome.GROVE,                        // 20
            Biome.ICE_SPIKES,                   // 21
            Biome.JAGGED_PEAKS,                 // 22
            Biome.JUNGLE,                       // 23
            Biome.LUKEWARM_OCEAN,               // 24
            Biome.LUSH_CAVES,                   // 25
            Biome.MANGROVE_SWAMP,               // 26
            Biome.MEADOW,                       // 27
            Biome.MUSHROOM_FIELDS,              // 28
            Biome.OCEAN,                        // 29
            Biome.OLD_GROWTH_BIRCH_FOREST,      // 30
            Biome.OLD_GROWTH_PINE_TAIGA,        // 31
            Biome.OLD_GROWTH_SPRUCE_TAIGA,      // 32
            resolveBiome("PALE_GARDEN", Biome.DARK_FOREST), // 33 (Minecraft 26.1)
            Biome.PLAINS,                       // 34
            Biome.RIVER,                        // 35
            Biome.SAVANNA,                      // 36
            Biome.SAVANNA_PLATEAU,              // 37
            Biome.SNOWY_BEACH,                  // 38
            Biome.SNOWY_PLAINS,                 // 39
            Biome.SNOWY_SLOPES,                 // 40
            Biome.SNOWY_TAIGA,                  // 41
            Biome.SPARSE_JUNGLE,                // 42
            Biome.STONY_PEAKS,                  // 43
            Biome.STONY_SHORE,                  // 44
            resolveBiome("SULFUR_CAVES", Biome.DRIPSTONE_CAVES), // 45 (Minecraft 26.2)
            Biome.SUNFLOWER_PLAINS,             // 46
            Biome.SWAMP,                        // 47
            Biome.TAIGA,                        // 48
            Biome.WARM_OCEAN,                   // 49
            Biome.WINDSWEPT_FOREST,             // 50
            Biome.WINDSWEPT_GRAVELLY_HILLS,     // 51
            Biome.WINDSWEPT_HILLS,              // 52
            Biome.WINDSWEPT_SAVANNA,            // 53
            Biome.WOODED_BADLANDS               // 54
    };

    private static final int CACHE_SIZE = 1024;
    private static final int CACHE_MASK = CACHE_SIZE - 1;

    private static final ThreadLocal<ChunkBiomeEntry[]> CHUNK_CACHE = ThreadLocal.withInitial(() -> {
        ChunkBiomeEntry[] entries = new ChunkBiomeEntry[CACHE_SIZE];
        for (int i = 0; i < CACHE_SIZE; i++) {
            entries[i] = new ChunkBiomeEntry();
        }
        return entries;
    });

    private static final int CAVE_CACHE_SIZE = 4096;
    private static final int CAVE_CACHE_MASK = CAVE_CACHE_SIZE - 1;

    private static final ThreadLocal<CaveBiomeEntry[]> CAVE_CACHE = ThreadLocal.withInitial(() -> {
        CaveBiomeEntry[] entries = new CaveBiomeEntry[CAVE_CACHE_SIZE];
        for (int i = 0; i < CAVE_CACHE_SIZE; i++) {
            entries[i] = new CaveBiomeEntry();
        }
        return entries;
    });

    private static class CaveBiomeEntry {
        long seed = Long.MIN_VALUE;
        int qx = Integer.MIN_VALUE;
        int qy = Integer.MIN_VALUE;
        int qz = Integer.MIN_VALUE;
        int biomeId = 34; // default Plains
    }

    private static class LastPrefetchEntry {
        long seed = Long.MIN_VALUE;
        int chunkX = Integer.MIN_VALUE;
        int chunkZ = Integer.MIN_VALUE;
    }

    private static final ThreadLocal<LastPrefetchEntry> LAST_PREFETCH = ThreadLocal.withInitial(LastPrefetchEntry::new);

    private static class ChunkBiomeEntry {
        long seed = Long.MIN_VALUE;
        int chunkX = Integer.MIN_VALUE;
        int chunkZ = Integer.MIN_VALUE;
        final byte[] biomes = new byte[256];
    }

    @Override
    public Biome getBiome(WorldInfo worldInfo, int x, int y, int z) {
        long seed = worldInfo.getSeed();

        // 3D cave biome check with fast 4096-entry quart cache
        // Cave biomes (Deep Dark, Lush Caves, Dripstone Caves, Sulfur Caves) occur between Y=-64 and Y=128
        if (y <= 128) {
            int qx = x >> 2;
            int qy = y >> 2;
            int qz = z >> 2;
            int slot = (((qx * 31 + qy) * 31 + qz) ^ (int) (seed ^ (seed >>> 32))) & CAVE_CACHE_MASK;
            CaveBiomeEntry[] caveCache = CAVE_CACHE.get();
            CaveBiomeEntry cEntry = caveCache[slot];

            if (cEntry.seed == seed && cEntry.qx == qx && cEntry.qy == qy && cEntry.qz == qz) {
                return (cEntry.biomeId < VANILLA_BIOMES.length) ? VANILLA_BIOMES[cEntry.biomeId] : Biome.PLAINS;
            }

            byte caveId = FastGenBindings.getBiome(seed, x, y, z);
            int id = caveId & 0xFF;
            cEntry.seed = seed;
            cEntry.qx = qx;
            cEntry.qy = qy;
            cEntry.qz = qz;
            cEntry.biomeId = id;

            // Trigger background precomputation for this chunk ahead of the NOISE stage (once per chunk)
            int chunkX = x >> 4;
            int chunkZ = z >> 4;
            LastPrefetchEntry last = LAST_PREFETCH.get();
            if (last.seed != seed || last.chunkX != chunkX || last.chunkZ != chunkZ) {
                last.seed = seed;
                last.chunkX = chunkX;
                last.chunkZ = chunkZ;
                int minH = worldInfo.getMinHeight();
                int h = worldInfo.getMaxHeight() - minH;
                FastGenPrefetcher.prefetch(seed, chunkX, chunkZ, minH, h);
            }

            return (id < VANILLA_BIOMES.length) ? VANILLA_BIOMES[id] : Biome.PLAINS;
        }

        int chunkX = x >> 4;
        int chunkZ = z >> 4;
        int localX = x & 15;
        int localZ = z & 15;

        int slot = (((chunkX * 0x1f1f1f1f) ^ chunkZ) & 0x7fffffff) & CACHE_MASK;
        ChunkBiomeEntry[] cache = CHUNK_CACHE.get();
        ChunkBiomeEntry entry = cache[slot];

        if (entry.seed != seed || entry.chunkX != chunkX || entry.chunkZ != chunkZ) {
            FastGenBindings.computeBiomes(seed, chunkX, chunkZ, entry.biomes);
            entry.seed = seed;
            entry.chunkX = chunkX;
            entry.chunkZ = chunkZ;

            // Trigger background precomputation for this chunk ahead of the NOISE stage
            int minH = worldInfo.getMinHeight();
            int h = worldInfo.getMaxHeight() - minH;
            FastGenPrefetcher.prefetch(seed, chunkX, chunkZ, minH, h);
        }

        int idx = localX * 16 + localZ;
        int id = entry.biomes[idx] & 0xFF;
        return (id < VANILLA_BIOMES.length) ? VANILLA_BIOMES[id] : Biome.PLAINS;
    }

    @Override
    public Biome getBiome(WorldInfo worldInfo, int x, int y, int z, org.bukkit.generator.BiomeParameterPoint biomeParameterPoint) {
        return getBiome(worldInfo, x, y, z);
    }

    @Override
    public List<Biome> getBiomes(WorldInfo worldInfo) {
        return Arrays.asList(VANILLA_BIOMES);
    }
}
