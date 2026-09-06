package com.fastgen.generator;

import com.fastgen.FastGenPlugin;
import com.fastgen.nativebridge.FastGenBindings;
import org.bukkit.Chunk;
import org.bukkit.HeightMap;
import org.bukkit.Material;
import org.bukkit.World;
import org.bukkit.block.Biome;
import org.bukkit.block.data.BlockData;
import org.bukkit.generator.BiomeProvider;
import org.bukkit.generator.BlockPopulator;
import org.bukkit.generator.ChunkGenerator;
import org.bukkit.generator.WorldInfo;

import java.util.Collections;
import java.util.List;
import java.util.Random;

/**
 * 1:1 SteelMC Vanilla Chunk Generator for Paper.
 * Full 3D noise router, multi-channel density interpolation, aquifer fluid/barrier simulation,
 * caves, and surface rules are computed natively in Rust via Panama FFI.
 * Paper handles structures, trees, mob spawning, and vanilla decorations.
 */
public class FastGenChunkGenerator extends ChunkGenerator {

    private final FastGenPlugin plugin;
    private final FastGenBlockPopulator populator = new FastGenBlockPopulator();
    private final FastGenBiomeProvider biomeProvider = new FastGenBiomeProvider();

    // Thread-local reusable buffers to eliminate multi-megabyte heap allocation per chunk
    private static final ThreadLocal<byte[]> CHUNK_BLOCKS_BUFFER = ThreadLocal.withInitial(() -> new byte[16 * 16 * 384]);
    private static final ThreadLocal<short[]> CHUNK_HEIGHTS_BUFFER = ThreadLocal.withInitial(() -> new short[256]);

    // 1:1 Palette mapping from native BlockStateId (0..28) to Bukkit Material
    private static final Material[] PALETTE = new Material[29];
    private static volatile BlockData[] CACHED_PALETTE_DATA = null;

    private static BlockData[] getPaletteData() {
        BlockData[] data = CACHED_PALETTE_DATA;
        if (data == null) {
            data = new BlockData[PALETTE.length];
            for (int i = 0; i < PALETTE.length; i++) {
                if (PALETTE[i] != null) {
                    try {
                        data[i] = PALETTE[i].createBlockData();
                    } catch (Throwable ignored) {
                        // Non-server environments (like tests)
                    }
                }
            }
            CACHED_PALETTE_DATA = data;
        }
        return data;
    }
    static {
        PALETTE[0] = Material.BEDROCK;
        PALETTE[1] = Material.COARSE_DIRT;
        PALETTE[2] = Material.GRASS_BLOCK;
        PALETTE[3] = Material.DIRT;
        PALETTE[4] = Material.WATER;
        PALETTE[5] = Material.ORANGE_TERRACOTTA;
        PALETTE[6] = Material.TERRACOTTA;
        PALETTE[7] = Material.RED_SANDSTONE;
        PALETTE[8] = Material.RED_SAND;
        PALETTE[9] = Material.WHITE_TERRACOTTA;
        PALETTE[10] = Material.STONE;
        PALETTE[11] = Material.GRAVEL;
        PALETTE[12] = Material.AIR;
        PALETTE[13] = Material.ICE;
        PALETTE[14] = Material.PACKED_ICE;
        PALETTE[15] = Material.SNOW_BLOCK;
        PALETTE[16] = Material.POWDER_SNOW;
        PALETTE[17] = Material.CALCITE;
        PALETTE[18] = Material.SANDSTONE;
        PALETTE[19] = Material.SAND;
        PALETTE[20] = Material.PODZOL;
        PALETTE[21] = Material.MUD;
        PALETTE[22] = Material.MYCELIUM;
        PALETTE[23] = Material.DEEPSLATE;
        PALETTE[24] = Material.LAVA;
        PALETTE[25] = Material.YELLOW_TERRACOTTA;
        PALETTE[26] = Material.BROWN_TERRACOTTA;
        PALETTE[27] = Material.RED_TERRACOTTA;
        PALETTE[28] = Material.LIGHT_GRAY_TERRACOTTA;
    }

    public FastGenChunkGenerator(FastGenPlugin plugin) {
        this.plugin = plugin;
        NmsChunkWriter.initPalette(PALETTE);
    }

    public FastGenChunkGenerator() {
        this(null);
    }

    @Override
    public BiomeProvider getDefaultBiomeProvider(WorldInfo worldInfo) {
        return this.biomeProvider;
    }

    @Override
    public void generateBedrock(WorldInfo worldInfo, Random random, int chunkX, int chunkZ, ChunkData chunkData) {
        // Bedrock is calculated natively by 1:1 SteelMC surface rules in generateNoise()
    }

    @Override
    public void generateNoise(WorldInfo worldInfo, Random random, int chunkX, int chunkZ, ChunkData chunkData) {
        long start = System.nanoTime();
        long seed = worldInfo.getSeed();
        int minHeight = worldInfo.getMinHeight();
        int maxHeight = worldInfo.getMaxHeight();
        int height = maxHeight - minHeight;

        long chunkKey = Chunk.getChunkKey(chunkX, chunkZ);
        FastGenPrefetcher.PrecomputedChunk precomputed = FastGenPrefetcher.poll(chunkKey);
        byte[] blocks;
        short[] heights;
        boolean fromPrefetch = false;

        if (precomputed != null) {
            blocks = precomputed.blocks;
            heights = precomputed.heights;
            fromPrefetch = true;
        } else {
            blocks = CHUNK_BLOCKS_BUFFER.get();
            if (blocks.length < 16 * 16 * height) {
                blocks = new byte[16 * 16 * height];
            }
            heights = CHUNK_HEIGHTS_BUFFER.get();
            FastGenBindings.generateChunk(seed, chunkX, chunkZ, minHeight, height, blocks, heights);
        }
        long elapsed = System.nanoTime() - start;

        // Mark this chunk as completed so prefetcher never generates it again
        FastGenPrefetcher.markCompleted(chunkKey);
        FastGenPrefetcher.cacheCompletedHeights(chunkKey, heights);

        // Multithreaded lookahead: dispatch 8 immediate surrounding chunks to the background worker pool
        FastGenPrefetcher.prefetchRadius(seed, chunkX, chunkZ, minHeight, height, 1);

        if (plugin != null && plugin.getTracker() != null) {
            plugin.getTracker().recordChunk(
                    worldInfo.getName(), chunkX, chunkZ, elapsed,
                    heights[8 * 16 + 8]
            );
        }

        int maxChunkHeight = minHeight;
        for (int i = 0; i < 256; i++) {
            if (heights[i] > maxChunkHeight) {
                maxChunkHeight = heights[i];
            }
        }

        try {
            // Direct zero-overhead NMS section injection:
            // Bypasses CraftChunkData.setBlock() to eliminate BlockPos allocations and 160,000 heightmap scans
            if (NmsChunkWriter.isAvailable()) {
                boolean success = NmsChunkWriter.write(chunkData, minHeight, height, blocks, heights, maxChunkHeight);
                if (success) {
                    return;
                }
            }

            // Clean Bukkit API fallback for mock/test environments:
            // Skips air blocks to avoid redundant writes
            int numSections = height >> 4;
            BlockData[] paletteData = getPaletteData();
            boolean hasBlockData = (paletteData != null && paletteData[10] != null);

            for (int secIdx = 0; secIdx < numSections; secIdx++) {
                int secMinY = minHeight + (secIdx << 4);
                if (secMinY > maxChunkHeight) {
                    continue;
                }
                int yRelBase = secIdx << 4;

                for (int x = 0; x < 16; x++) {
                    int xBase = x * 16;
                    for (int z = 0; z < 16; z++) {
                        int colIdx = xBase + z;
                        if (secMinY > heights[colIdx]) {
                            continue;
                        }
                        int colBase = colIdx * height + yRelBase;
                        int maxDy = Math.min(16, heights[colIdx] - secMinY + 1);

                        for (int dy = 0; dy < maxDy; dy++) {
                            int id = blocks[colBase + dy] & 0xFF;
                            if (id != 12) { // not air
                                int wy = secMinY + dy;
                                if (hasBlockData) {
                                    BlockData data = (id < paletteData.length && paletteData[id] != null)
                                            ? paletteData[id] : paletteData[10];
                                    chunkData.setBlock(x, wy, z, data);
                                } else {
                                    Material mat = (id < PALETTE.length) ? PALETTE[id] : Material.STONE;
                                    chunkData.setBlock(x, wy, z, mat);
                                }
                            }
                        }
                    }
                }
            }
        } finally {
            if (fromPrefetch) {
                FastGenPrefetcher.recycle(blocks, heights);
            }
        }
    }

    @Override
    public void generateSurface(WorldInfo worldInfo, Random random, int chunkX, int chunkZ, ChunkData chunkData) {
        // Surface rules (grass, sand, terracotta, gravel, snow, etc.) applied natively in generateNoise()
    }


    @Override
    public List<BlockPopulator> getDefaultPopulators(World world) {
        // Vanilla Paper handles all decorations, trees, ores, and structures asynchronously.
        // Returning an empty list removes Paper's LimitedRegion locking barrier between chunks.
        return Collections.emptyList();
    }

    @Override
    public boolean shouldGenerateNoise() {
        return false;
    }

    @Override
    public int getBaseHeight(WorldInfo worldInfo, Random random, int x, int z, HeightMap heightMap) {
        return FastGenPrefetcher.getBaseHeight(worldInfo.getSeed(), x, z, worldInfo.getMinHeight(), worldInfo.getMaxHeight(), heightMap);
    }

    @Override
    public boolean shouldGenerateSurface() {
        return false;
    }

    @Override
    public boolean shouldGenerateBedrock() {
        return false;
    }

    @Override
    public boolean shouldGenerateCaves() {
        return false;
    }

    @Override
    public boolean shouldGenerateDecorations() {
        return true;
    }

    @Override
    public boolean shouldGenerateMobs() {
        return true;
    }

    @Override
    public boolean shouldGenerateStructures() {
        return true;
    }
}
