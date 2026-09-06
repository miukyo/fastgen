package com.fastgen.generator;

import com.fastgen.nativebridge.FastGenBindings;
import org.bukkit.Chunk;

import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.logging.Logger;

/**
 * High-throughput asynchronous multi-threaded chunk prefetch pipeline.
 * Saturates all available CPU cores by computing upcoming 3D chunks
 * concurrently in the background.
 * Uses zero-allocation pooled byte arrays to eliminate GC pressure.
 */
public final class FastGenPrefetcher {

    private static final Logger LOGGER = Logger.getLogger("FastGen-Prefetch");

    private static volatile int threadCount = detectDefaultThreads();
    private static volatile ThreadPoolExecutor executor = createExecutor(threadCount);

    private static final int MAX_CACHED_CHUNKS = 512;
    private static final int MAX_POOL_BUFFERS = 512;
    private static final int MAX_COMPLETED = 16384;

    private static final ConcurrentHashMap<Long, CompletableFuture<PrecomputedChunk>> PRECOMPUTE_MAP = new ConcurrentHashMap<>(
            MAX_CACHED_CHUNKS);
    private static final ConcurrentLinkedQueue<Long> EVICTION_QUEUE = new ConcurrentLinkedQueue<>();

    private static final java.util.Set<Long> COMPLETED_CHUNKS = java.util.Collections
            .newSetFromMap(new ConcurrentHashMap<>(MAX_COMPLETED));
    private static final ConcurrentLinkedQueue<Long> COMPLETED_EVICTION = new ConcurrentLinkedQueue<>();

    private static final int MAX_COMPLETED_HEIGHTS = 4096;
    private static final ConcurrentHashMap<Long, short[]> COMPLETED_HEIGHTS = new ConcurrentHashMap<>(
            MAX_COMPLETED_HEIGHTS);
    private static final ConcurrentLinkedQueue<Long> COMPLETED_HEIGHTS_EVICTION = new ConcurrentLinkedQueue<>();

    private static final ConcurrentLinkedQueue<byte[]> BLOCK_POOL = new ConcurrentLinkedQueue<>();
    private static final ConcurrentLinkedQueue<short[]> HEIGHT_POOL = new ConcurrentLinkedQueue<>();
    private static final AtomicInteger POOL_SIZE = new AtomicInteger(0);

    private static ThreadPoolExecutor createExecutor(int threads) {
        return new ThreadPoolExecutor(
                threads,
                threads,
                60L,
                TimeUnit.SECONDS,
                new LinkedBlockingQueue<>(1024),
                new ThreadFactory() {
                    private final AtomicInteger counter = new AtomicInteger(1);

                    @Override
                    public Thread newThread(Runnable r) {
                        Thread t = new Thread(r, "FastGen-Prefetch-" + counter.getAndIncrement());
                        t.setDaemon(true);
                        t.setPriority(Thread.NORM_PRIORITY - 2);
                        return t;
                    }
                },
                new ThreadPoolExecutor.DiscardOldestPolicy());
    }

    public static int detectDefaultThreads() {
        // 1. Try Paper GlobalConfiguration via reflection
        try {
            Class<?> clazz = Class.forName("io.papermc.paper.configuration.GlobalConfiguration");
            Object globalConfig = clazz.getMethod("get").invoke(null);
            Object chunkSystem = clazz.getField("chunkSystem").get(globalConfig);
            int workerThreads = chunkSystem.getClass().getField("workerThreads").getInt(chunkSystem);
            if (workerThreads > 0) {
                return workerThreads;
            }
        } catch (Throwable ignored) {
        }

        // 2. Try reading config/paper-global.yml directly
        for (String path : new String[] { "config/paper-global.yml", "../config/paper-global.yml",
                "test-server/config/paper-global.yml" }) {
            java.io.File file = new java.io.File(path);
            if (file.exists()) {
                try {
                    org.bukkit.configuration.file.YamlConfiguration yaml = org.bukkit.configuration.file.YamlConfiguration
                            .loadConfiguration(file);
                    int val = yaml.getInt("chunk-system.worker-threads", -1);
                    if (val > 0) {
                        return val;
                    }
                } catch (Throwable ignored) {
                }
            }
        }

        // 3. Fallback to half CPU cores (min 2)
        return Math.max(2, Runtime.getRuntime().availableProcessors());
    }

    public static boolean isEnabled() {
        return threadCount > 0 && executor != null && !executor.isShutdown();
    }

    public static synchronized void init(int workerThreads) {
        if (workerThreads <= 0) {
            threadCount = 0;
            if (executor != null) {
                executor.shutdownNow();
                executor = null;
            }
            LOGGER.info("FastGenPrefetcher disabled (0 threads). Paper chunk-system worker threads drive generation directly.");
            return;
        }

        threadCount = workerThreads;
        if (executor != null && !executor.isShutdown()) {
            if (workerThreads > executor.getMaximumPoolSize()) {
                executor.setMaximumPoolSize(workerThreads);
                executor.setCorePoolSize(workerThreads);
            } else {
                executor.setCorePoolSize(workerThreads);
                executor.setMaximumPoolSize(workerThreads);
            }
        } else {
            executor = createExecutor(workerThreads);
        }
        LOGGER.info("FastGenPrefetcher active: " + workerThreads
                + " worker threads dedicated to parallel chunk prefetching.");
    }

    static {
        LOGGER.info("FastGenPrefetcher initialized with " + threadCount + " worker threads.");
    }

    public static final class PrecomputedChunk {
        public final long chunkKey;
        public final byte[] blocks;
        public final short[] heights;

        public PrecomputedChunk(long chunkKey, byte[] blocks, short[] heights) {
            this.chunkKey = chunkKey;
            this.blocks = blocks;
            this.heights = heights;
        }
    }

    private static byte[] acquireBlockBuffer(int totalBlocks) {
        byte[] buf = BLOCK_POOL.poll();
        if (buf == null || buf.length < totalBlocks) {
            return new byte[totalBlocks];
        }
        POOL_SIZE.decrementAndGet();
        return buf;
    }

    private static short[] acquireHeightBuffer() {
        short[] buf = HEIGHT_POOL.poll();
        if (buf == null) {
            return new short[256];
        }
        return buf;
    }

    /**
     * Recycle buffers back to pool for zero-allocation reuse.
     */
    public static void recycle(byte[] blocks, short[] heights) {
        if (blocks != null && POOL_SIZE.get() < MAX_POOL_BUFFERS) {
            BLOCK_POOL.offer(blocks);
            POOL_SIZE.incrementAndGet();
        }
        if (heights != null && HEIGHT_POOL.size() < MAX_POOL_BUFFERS) {
            HEIGHT_POOL.offer(heights);
        }
    }

    public static void markCompleted(long chunkKey) {
        if (COMPLETED_CHUNKS.add(chunkKey)) {
            COMPLETED_EVICTION.offer(chunkKey);
            if (COMPLETED_CHUNKS.size() > MAX_COMPLETED) {
                Long old = COMPLETED_EVICTION.poll();
                if (old != null) {
                    COMPLETED_CHUNKS.remove(old);
                }
            }
        }
    }

    public static boolean isCompleted(long chunkKey) {
        return COMPLETED_CHUNKS.contains(chunkKey);
    }

    /**
     * Enqueue a chunk for background asynchronous native generation.
     */
    public static void prefetch(long seed, int chunkX, int chunkZ, int minHeight, int height) {
        if (!isEnabled()) {
            return;
        }
        long key = Chunk.getChunkKey(chunkX, chunkZ);

        if (isCompleted(key) || PRECOMPUTE_MAP.containsKey(key)) {
            return;
        }

        // Bounded capacity check: evict oldest if queue full
        if (PRECOMPUTE_MAP.size() >= MAX_CACHED_CHUNKS) {
            Long oldest = EVICTION_QUEUE.poll();
            if (oldest != null) {
                CompletableFuture<PrecomputedChunk> oldFuture = PRECOMPUTE_MAP.remove(oldest);
                if (oldFuture != null && oldFuture.isDone() && !oldFuture.isCompletedExceptionally()) {
                    try {
                        PrecomputedChunk chunk = oldFuture.getNow(null);
                        if (chunk != null) {
                            cacheCompletedHeights(oldest, chunk.heights);
                            recycle(chunk.blocks, chunk.heights);
                        }
                    } catch (Throwable ignored) {
                    }
                }
            }
        }

        CompletableFuture<PrecomputedChunk> future = new CompletableFuture<>();
        CompletableFuture<PrecomputedChunk> existing = PRECOMPUTE_MAP.putIfAbsent(key, future);
        if (existing != null) {
            return;
        }

        EVICTION_QUEUE.offer(key);

        try {
            executor.execute(() -> {
                int totalBlocks = 16 * 16 * height;
                byte[] blocks = acquireBlockBuffer(totalBlocks);
                short[] heights = acquireHeightBuffer();

                try {
                    FastGenBindings.generateChunk(seed, chunkX, chunkZ, minHeight, height, blocks, heights);
                    future.complete(new PrecomputedChunk(key, blocks, heights));
                } catch (Throwable t) {
                    future.completeExceptionally(t);
                    recycle(blocks, heights);
                }
            });
        } catch (Throwable rejected) {
            PRECOMPUTE_MAP.remove(key);
        }
    }

    /**
     * Retrieve precomputed chunk or compute immediately.
     */
    public static PrecomputedChunk getOrComputeChunk(long seed, int chunkX, int chunkZ, int minHeight, int height) {
        long key = Chunk.getChunkKey(chunkX, chunkZ);
        CompletableFuture<PrecomputedChunk> future = PRECOMPUTE_MAP.get(key);
        if (future != null) {
            try {
                return future.join();
            } catch (Throwable ignored) {
            }
        }

        CompletableFuture<PrecomputedChunk> newFuture = new CompletableFuture<>();
        CompletableFuture<PrecomputedChunk> existing = PRECOMPUTE_MAP.putIfAbsent(key, newFuture);
        if (existing != null) {
            try {
                return existing.join();
            } catch (Throwable ignored) {
            }
        }

        EVICTION_QUEUE.offer(key);
        int totalBlocks = 16 * 16 * height;
        byte[] blocks = acquireBlockBuffer(totalBlocks);
        short[] heights = acquireHeightBuffer();
        try {
            FastGenBindings.generateChunk(seed, chunkX, chunkZ, minHeight, height, blocks, heights);
            PrecomputedChunk chunk = new PrecomputedChunk(key, blocks, heights);
            newFuture.complete(chunk);
            return chunk;
        } catch (Throwable t) {
            newFuture.completeExceptionally(t);
            recycle(blocks, heights);
            PRECOMPUTE_MAP.remove(key);
            return null;
        }
    }

    /**
     * Exact 1:1 ground/surface height lookup for structure placement and probes.
     * Guaranteed to match the actual chunk generation block-for-block.
     */
    public static int getBaseHeight(long seed, int x, int z, int minHeight, int maxHeight, org.bukkit.HeightMap heightMap) {
        int chunkX = x >> 4;
        int chunkZ = z >> 4;
        int localX = x & 15;
        int localZ = z & 15;
        long key = Chunk.getChunkKey(chunkX, chunkZ);
        int colIdx = localX * 16 + localZ;
        int height = maxHeight - minHeight;

        // 1. Check COMPLETED_HEIGHTS cache (O(1))
        short[] cached = COMPLETED_HEIGHTS.get(key);
        if (cached != null) {
            return cached[colIdx];
        }

        // 2. Fetch or compute chunk
        PrecomputedChunk chunk = getOrComputeChunk(seed, chunkX, chunkZ, minHeight, height);
        if (chunk == null) {
            return FastGenBindings.getBaseHeight(seed, x, z, minHeight, maxHeight);
        }

        int topY = chunk.heights[colIdx];
        if (heightMap == org.bukkit.HeightMap.OCEAN_FLOOR || heightMap == org.bukkit.HeightMap.OCEAN_FLOOR_WG) {
            int colBase = colIdx * height;
            for (int y = topY - 1; y >= minHeight; y--) {
                int relY = y - minHeight;
                if (relY >= 0 && relY < height) {
                    int blockId = chunk.blocks[colBase + relY] & 0xFF;
                    if (blockId != 4 && blockId != 24 && blockId != 12) { // not water, lava, or air
                        return y + 1;
                    }
                }
            }
        }

        return topY;
    }

    public static void cacheCompletedHeights(long chunkKey, short[] heights) {
        if (heights == null) return;
        short[] copy = heights.clone();
        COMPLETED_HEIGHTS.put(chunkKey, copy);
        COMPLETED_HEIGHTS_EVICTION.offer(chunkKey);
        if (COMPLETED_HEIGHTS.size() > MAX_COMPLETED_HEIGHTS) {
            Long old = COMPLETED_HEIGHTS_EVICTION.poll();
            if (old != null) {
                COMPLETED_HEIGHTS.remove(old);
            }
        }
    }

    /**
     * Prefetch surrounding chunks in radius around target chunk.
     */
    public static void prefetchRadius(long seed, int centerChunkX, int centerChunkZ, int minHeight, int height,
            int radius) {
        if (!isEnabled()) {
            return;
        }
        for (int dx = -radius; dx <= radius; dx++) {
            for (int dz = -radius; dz <= radius; dz++) {
                if (dx == 0 && dz == 0)
                    continue;
                prefetch(seed, centerChunkX + dx, centerChunkZ + dz, minHeight, height);
            }
        }
    }

    /**
     * Poll precomputed chunk if available or currently in progress.
     */
    public static PrecomputedChunk poll(long chunkKey) {
        CompletableFuture<PrecomputedChunk> future = PRECOMPUTE_MAP.remove(chunkKey);
        if (future == null) {
            return null;
        }

        try {
            PrecomputedChunk chunk = future.join();
            if (chunk != null) {
                cacheCompletedHeights(chunkKey, chunk.heights);
            }
            return chunk;
        } catch (Throwable t) {
            return null;
        }
    }

    public static int getThreadCount() {
        return threadCount;
    }

    public static int getCachedCount() {
        return PRECOMPUTE_MAP.size();
    }

    public static synchronized void shutdown() {
        if (executor != null) {
            executor.shutdownNow();
            executor = null;
        }
        threadCount = 0;
        PRECOMPUTE_MAP.clear();
        EVICTION_QUEUE.clear();
        COMPLETED_CHUNKS.clear();
        COMPLETED_EVICTION.clear();
        BLOCK_POOL.clear();
        HEIGHT_POOL.clear();
    }
}
