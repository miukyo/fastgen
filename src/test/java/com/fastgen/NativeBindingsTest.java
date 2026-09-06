package com.fastgen;

import com.fastgen.nativebridge.FastGenBindings;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

public class NativeBindingsTest {

    @Test
    public void testChunkDataMethods() {
        for (java.lang.reflect.Method m : org.bukkit.generator.ChunkGenerator.ChunkData.class.getMethods()) {
            System.out.println("ChunkData method: " + m);
        }
    }

    @Test
    public void testPanamaHeightmapCall() {
        short[] heights = FastGenBindings.computeHeightmap(987654321L, 0, 0, -64, 320);
        assertNotNull(heights);
        assertEquals(256, heights.length);

        for (short h : heights) {
            assertTrue(h >= -64 && h <= 320, "Height out of bounds: " + h);
        }
    }

    @Test
    public void testPanamaClimateCall() {
        long seed = 424242L;
        float[] climate = FastGenBindings.computeClimate(seed, 0, 0);

        assertNotNull(climate);
        assertEquals(256 * 5, climate.length);

        for (int i = 0; i < climate.length; i++) {
            assertTrue(climate[i] >= -1.0f && climate[i] <= 1.0f, "Climate value out of range at index " + i + ": " + climate[i]);
        }
    }

    @Test
    public void testGenerateChunkCall() {
        long seed = 123456789L;
        int minY = -64;
        int height = 384;
        byte[] blocks = new byte[16 * 16 * height];
        short[] heights = new short[256];

        FastGenBindings.generateChunk(seed, 0, 0, minY, height, blocks, heights);

        // Verify heightmap populated
        for (short h : heights) {
            assertTrue(h >= minY && h <= minY + height, "Height out of bounds: " + h);
        }

        // Verify bottom layer has bedrock (id 0)
        for (int x = 0; x < 16; x++) {
            for (int z = 0; z < 16; z++) {
                int idx = (x * 16 + z) * height;
                assertEquals(0, blocks[idx], "Expected bedrock at bottom layer");
            }
        }

        // Verify solid blocks and air blocks exist
        int solidCount = 0;
        int airCount = 0;
        for (byte b : blocks) {
            int id = b & 0xFF;
            if (id == 12) { // Air
                airCount++;
            } else {
                solidCount++;
                assertTrue(id < 29, "Invalid block state ID: " + id);
            }
        }

        assertTrue(solidCount > 0, "No solid blocks generated");
        assertTrue(airCount > 0, "No air blocks generated");
    }

    @Test
    public void testComputeBiomesCall() {
        long seed = 123456789L;
        byte[] biomes = FastGenBindings.computeBiomes(seed, 0, 0);
        assertNotNull(biomes);
        assertEquals(256, biomes.length);

        for (byte b : biomes) {
            int id = b & 0xFF;
            assertTrue(id < 55, "Biome ID out of range: " + id);
        }

        byte singleBiome = FastGenBindings.getBiome(seed, 100, 64, -200);
        int singleId = singleBiome & 0xFF;
        assertTrue(singleId < 55, "Single biome ID out of range: " + singleId);
    }

    @Test
    public void testBaseHeightCall() {
        long seed = 123456789L;
        int minY = -64;
        int height = 384;
        byte[] blocks = new byte[16 * 16 * height];
        short[] heights = new short[256];
        FastGenBindings.generateChunk(seed, 0, 0, minY, height, blocks, heights);

        int maxDiff = 0;
        int totalDiff = 0;
        for (int x = 0; x < 16; x++) {
            for (int z = 0; z < 16; z++) {
                int baseH = FastGenBindings.getBaseHeight(seed, x, z, minY, minY + height);
                int actualH = heights[x * 16 + z];
                int diff = Math.abs(baseH - actualH);
                if (diff > maxDiff) maxDiff = diff;
                totalDiff += diff;
            }
        }
        System.out.printf("[TEST] getBaseHeight vs actual: avg diff=%.2f, max diff=%d\n", totalDiff / 256.0, maxDiff);
    }

    @Test
    public void testPrefetcherExactBaseHeight() {
        long seed = 123456789L;
        int minY = -64;
        int height = 384;
        byte[] blocks = new byte[16 * 16 * height];
        short[] heights = new short[256];
        FastGenBindings.generateChunk(seed, 2, 3, minY, height, blocks, heights);

        for (int x = 0; x < 16; x++) {
            for (int z = 0; z < 16; z++) {
                int worldX = 2 * 16 + x;
                int worldZ = 3 * 16 + z;
                int prefetchH = com.fastgen.generator.FastGenPrefetcher.getBaseHeight(
                        seed, worldX, worldZ, minY, minY + height, org.bukkit.HeightMap.WORLD_SURFACE_WG);
                int actualH = heights[x * 16 + z];
                assertEquals(actualH, prefetchH, "Exact surface height mismatch at (" + worldX + ", " + worldZ + ")");
            }
        }
        System.out.println("[TEST] FastGenPrefetcher.getBaseHeight: 100% exact match across all 256 columns!");
    }

    @Test
    public void testWorldgenVersionSwitching() {
        // Test setting to 26.1
        FastGenBindings.setVersion(1);
        assertEquals(1, FastGenBindings.getVersion());

        // Test setting to 26.2
        FastGenBindings.setVersion(2);
        assertEquals(2, FastGenBindings.getVersion());

        // Test parsing
        assertEquals(com.fastgen.generator.WorldgenVersion.V26_1, com.fastgen.generator.WorldgenVersion.parse("26.1"));
        assertEquals(com.fastgen.generator.WorldgenVersion.V26_1, com.fastgen.generator.WorldgenVersion.parse("1.21"));
        assertEquals(com.fastgen.generator.WorldgenVersion.V26_2, com.fastgen.generator.WorldgenVersion.parse("26.2"));
        assertEquals(com.fastgen.generator.WorldgenVersion.V26_2, com.fastgen.generator.WorldgenVersion.parse("auto"));
    }

    @Test
    public void testWorkerThreadsRespectPaperGlobal() {
        int detected = com.fastgen.generator.FastGenPrefetcher.detectDefaultThreads();
        assertTrue(detected >= 2, "Detected worker threads should be at least 2: " + detected);

        // Test explicit init
        com.fastgen.generator.FastGenPrefetcher.init(6);
        assertEquals(6, com.fastgen.generator.FastGenPrefetcher.getThreadCount());

        com.fastgen.generator.FastGenPrefetcher.init(4);
        assertEquals(4, com.fastgen.generator.FastGenPrefetcher.getThreadCount());
    }

    @Test
    public void testBenchmarkTimings() {
        long seed = 123456789L;
        int runs = 50;

        // 1. generateChunk
        byte[] blocks = new byte[16 * 16 * 384];
        short[] heights = new short[256];
        long startGen = System.nanoTime();
        for (int i = 0; i < runs; i++) {
            FastGenBindings.generateChunk(seed, i, i, -64, 384, blocks, heights);
        }
        double msPerGen = (System.nanoTime() - startGen) / 1_000_000.0 / runs;

        // 2. computeBiomes
        long startBiomes = System.nanoTime();
        for (int i = 0; i < runs; i++) {
            FastGenBindings.computeBiomes(seed, i, i);
        }
        double msPerBiomes = (System.nanoTime() - startBiomes) / 1_000_000.0 / runs;

        // 3. getBiome (single point)
        int singleRuns = 10000;
        long startSingle = System.nanoTime();
        for (int i = 0; i < singleRuns; i++) {
            FastGenBindings.getBiome(seed, i, 64, i);
        }
        double nsPerSingleBiome = (double)(System.nanoTime() - startSingle) / singleRuns;

        System.out.printf("[BENCHMARK] generateChunk avg: %.3f ms\n", msPerGen);
        System.out.printf("[BENCHMARK] computeBiomes avg: %.3f ms\n", msPerBiomes);
        System.out.printf("[BENCHMARK] getBiome single avg: %.1f ns\n", nsPerSingleBiome);
    }

    @Test
    public void testMultiThreadedPrefetcher() {
        com.fastgen.generator.FastGenPrefetcher.init(4);
        long seed = 123456789L;
        int chunkCount = 20;

        long start = System.nanoTime();
        for (int i = 0; i < chunkCount; i++) {
            com.fastgen.generator.FastGenPrefetcher.prefetch(seed, i, i, -64, 384);
        }

        int completed = 0;
        for (int i = 0; i < chunkCount; i++) {
            long key = org.bukkit.Chunk.getChunkKey(i, i);
            com.fastgen.generator.FastGenPrefetcher.PrecomputedChunk chunk =
                    com.fastgen.generator.FastGenPrefetcher.poll(key);
            if (chunk != null) {
                assertNotNull(chunk.blocks);
                assertNotNull(chunk.heights);
                assertEquals(16 * 16 * 384, chunk.blocks.length);
                completed++;
                com.fastgen.generator.FastGenPrefetcher.recycle(chunk.blocks, chunk.heights);
            }
        }

        long elapsedNanos = System.nanoTime() - start;
        double totalMs = elapsedNanos / 1_000_000.0;
        double msPerChunk = totalMs / chunkCount;

        System.out.printf("[BENCHMARK] Multithreaded prefetch (%d threads): %d chunks in %.2f ms (effective %.3f ms/chunk)\n",
                com.fastgen.generator.FastGenPrefetcher.getThreadCount(), completed, totalMs, msPerChunk);
        assertEquals(chunkCount, completed, "All prefetched chunks should complete");
    }

    @Test
    public void testPrefetcherDisabledZeroThreads() {
        com.fastgen.generator.FastGenPrefetcher.init(0);
        assertFalse(com.fastgen.generator.FastGenPrefetcher.isEnabled());
        assertEquals(0, com.fastgen.generator.FastGenPrefetcher.getThreadCount());

        long seed = 123456789L;
        com.fastgen.generator.FastGenPrefetcher.prefetch(seed, 999, 999, -64, 384);
        assertNull(com.fastgen.generator.FastGenPrefetcher.poll(org.bukkit.Chunk.getChunkKey(999, 999)));

        // getBaseHeight still works synchronously when prefetcher is disabled
        int h = com.fastgen.generator.FastGenPrefetcher.getBaseHeight(seed, 100, 200, -64, 320, org.bukkit.HeightMap.WORLD_SURFACE_WG);
        assertTrue(h >= -64 && h <= 320);
    }

    @Test
    public void testSulfurCavesDiscovery() {
        FastGenBindings.setVersion(2); // 26.2 mode
        long seed = 123456789L;
        boolean found = false;
        int foundX = 0, foundY = 0, foundZ = 0;

        // Search over a 2000x2000 area at cave heights (Y=20, Y=40, Y=60)
        outer:
        for (int y : new int[]{30, 50, 70}) {
            for (int x = -1000; x <= 1000; x += 16) {
                for (int z = -1000; z <= 1000; z += 16) {
                    byte biomeId = FastGenBindings.getBiome(seed, x, y, z);
                    if ((biomeId & 0xFF) == 45) { // SULFUR_CAVES
                        found = true;
                        foundX = x;
                        foundY = y;
                        foundZ = z;
                        break outer;
                    }
                }
            }
        }

        assertTrue(found, "Sulfur caves (ID 45) should be found within 1000 blocks at cave height in 26.2 mode");
        System.out.printf("[TEST] Found sulfur caves (45) at X=%d, Y=%d, Z=%d on seed %d\n", foundX, foundY, foundZ, seed);

        // Switch to 26.1 and verify that the exact same coordinates do NOT return 45
        FastGenBindings.setVersion(1); // 26.1 mode
        byte biomeId26_1 = FastGenBindings.getBiome(seed, foundX, foundY, foundZ);
        assertNotEquals(45, biomeId26_1 & 0xFF, "26.1 must not generate sulfur caves (45) at this coordinate");
        System.out.printf("[TEST] Same coordinate in 26.1 returns biome ID: %d\n", biomeId26_1 & 0xFF);

        // Reset back to 26.2
        FastGenBindings.setVersion(2);
    }

    @Test
    public void testRustNativeWorkerThreads() {
        int initial = FastGenBindings.getWorkerThreads();
        assertTrue(initial > 0, "Native worker threads should be > 0");
        System.out.println("[TEST] Native initial worker threads: " + initial);

        // Verify paper-global.yml default detection (4)
        int paperThreads = com.fastgen.generator.FastGenPrefetcher.detectDefaultThreads();
        assertEquals(4, paperThreads, "Paper-global config defines 4 worker-threads");

        // Set to paper threads
        int setThreads = FastGenBindings.setWorkerThreads(paperThreads);
        assertEquals(paperThreads, setThreads);
        assertEquals(paperThreads, FastGenBindings.getWorkerThreads());

        // Test changing worker threads
        int changed = FastGenBindings.setWorkerThreads(6);
        assertEquals(6, changed);
        assertEquals(6, FastGenBindings.getWorkerThreads());

        // Restore
        FastGenBindings.setWorkerThreads(paperThreads);
    }
}

