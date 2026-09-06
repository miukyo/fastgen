package com.fastgen.nativebridge;

import java.lang.foreign.*;
import java.lang.invoke.MethodHandle;

/**
 * Foreign Function & Memory (Panama) native bindings to fastgen_native.dll.
 * Provides zero-overhead access to 1:1 SteelMC math, density functions, aquifers, and carvers.
 */
public final class FastGenBindings {

    private static final MethodHandle HAS_AVX2_HANDLE;
    private static final MethodHandle GENERATE_CHUNK_HANDLE;
    private static final MethodHandle GET_BASE_HEIGHT_HANDLE;
    private static final MethodHandle COMPUTE_HEIGHTMAP_HANDLE;
    private static final MethodHandle COMPUTE_BIOMES_HANDLE;
    private static final MethodHandle GET_BIOME_HANDLE;
    private static final MethodHandle COMPUTE_CLIMATE_HANDLE;
    private static final MethodHandle SET_VERSION_HANDLE;
    private static final MethodHandle GET_VERSION_HANDLE;
    private static final MethodHandle SET_WORKER_THREADS_HANDLE;
    private static final MethodHandle GET_WORKER_THREADS_HANDLE;
    private static final MethodHandle GENERATE_CHUNKS_BATCH_HANDLE;

    private static final int MAX_CHUNK_BLOCKS = 16 * 16 * 384;
    private static final ThreadLocal<MemorySegment> THREAD_BLOCKS_SEG = ThreadLocal.withInitial(() ->
            Arena.ofAuto().allocate(MAX_CHUNK_BLOCKS)
    );
    private static final ThreadLocal<MemorySegment> THREAD_HEIGHTS_SEG = ThreadLocal.withInitial(() ->
            Arena.ofAuto().allocate(256 * ValueLayout.JAVA_SHORT.byteSize())
    );
    private static final ThreadLocal<MemorySegment> THREAD_BIOMES_SEG = ThreadLocal.withInitial(() ->
            Arena.ofAuto().allocate(256 * ValueLayout.JAVA_BYTE.byteSize())
    );
    private static final ThreadLocal<MemorySegment> THREAD_CLIMATE_SEG = ThreadLocal.withInitial(() ->
            Arena.ofAuto().allocate(256 * 5 * ValueLayout.JAVA_FLOAT.byteSize())
    );

    static {
        Linker linker = Linker.nativeLinker();
        SymbolLookup lookup = NativeLibLoader.load();

        MemorySegment avx2Symbol = lookup.find("fastgen_has_avx2")
                .orElseThrow(() -> new UnsatisfiedLinkError("fastgen_has_avx2 not found"));
        HAS_AVX2_HANDLE = linker.downcallHandle(
                avx2Symbol,
                FunctionDescriptor.of(ValueLayout.JAVA_INT),
                Linker.Option.critical(false)
        );

        MemorySegment generateChunkSymbol = lookup.find("fastgen_generate_chunk")
                .orElseThrow(() -> new UnsatisfiedLinkError("fastgen_generate_chunk not found"));
        GENERATE_CHUNK_HANDLE = linker.downcallHandle(
                generateChunkSymbol,
                FunctionDescriptor.ofVoid(
                        ValueLayout.JAVA_LONG, // seed
                        ValueLayout.JAVA_INT,  // chunkX
                        ValueLayout.JAVA_INT,  // chunkZ
                        ValueLayout.JAVA_INT,  // minY
                        ValueLayout.JAVA_INT,  // height
                        ValueLayout.ADDRESS,   // out_blocks
                        ValueLayout.ADDRESS    // out_heights
                )
        );

        MemorySegment getBaseHeightSymbol = lookup.find("fastgen_get_base_height")
                .orElseThrow(() -> new UnsatisfiedLinkError("fastgen_get_base_height not found"));
        GET_BASE_HEIGHT_HANDLE = linker.downcallHandle(
                getBaseHeightSymbol,
                FunctionDescriptor.of(
                        ValueLayout.JAVA_INT,  // return height
                        ValueLayout.JAVA_LONG, // seed
                        ValueLayout.JAVA_INT,  // x
                        ValueLayout.JAVA_INT,  // z
                        ValueLayout.JAVA_INT,  // minY
                        ValueLayout.JAVA_INT   // maxY
                ),
                Linker.Option.critical(false)
        );

        MemorySegment heightmapSymbol = lookup.find("fastgen_compute_heightmap")
                .orElseThrow(() -> new UnsatisfiedLinkError("fastgen_compute_heightmap not found"));
        COMPUTE_HEIGHTMAP_HANDLE = linker.downcallHandle(
                heightmapSymbol,
                FunctionDescriptor.ofVoid(
                        ValueLayout.JAVA_LONG, // seed
                        ValueLayout.JAVA_INT,  // chunkX
                        ValueLayout.JAVA_INT,  // chunkZ
                        ValueLayout.JAVA_INT,  // minY
                        ValueLayout.JAVA_INT,  // maxY
                        ValueLayout.ADDRESS    // out_heights
                )
        );

        MemorySegment biomesSymbol = lookup.find("fastgen_compute_biomes")
                .orElseThrow(() -> new UnsatisfiedLinkError("fastgen_compute_biomes not found"));
        COMPUTE_BIOMES_HANDLE = linker.downcallHandle(
                biomesSymbol,
                FunctionDescriptor.ofVoid(
                        ValueLayout.JAVA_LONG, // seed
                        ValueLayout.JAVA_INT,  // chunkX
                        ValueLayout.JAVA_INT,  // chunkZ
                        ValueLayout.ADDRESS    // out_biomes
                )
        );

        MemorySegment getBiomeSymbol = lookup.find("fastgen_get_biome")
                .orElseThrow(() -> new UnsatisfiedLinkError("fastgen_get_biome not found"));
        GET_BIOME_HANDLE = linker.downcallHandle(
                getBiomeSymbol,
                FunctionDescriptor.of(
                        ValueLayout.JAVA_BYTE,
                        ValueLayout.JAVA_LONG, // seed
                        ValueLayout.JAVA_INT,  // x
                        ValueLayout.JAVA_INT,  // y
                        ValueLayout.JAVA_INT   // z
                ),
                Linker.Option.critical(false)
        );

        MemorySegment climateSymbol = lookup.find("fastgen_compute_climate")
                .orElseThrow(() -> new UnsatisfiedLinkError("fastgen_compute_climate not found"));
        COMPUTE_CLIMATE_HANDLE = linker.downcallHandle(
                climateSymbol,
                FunctionDescriptor.ofVoid(
                        ValueLayout.JAVA_LONG,   // seed
                        ValueLayout.JAVA_INT,    // chunkX
                        ValueLayout.JAVA_INT,    // chunkZ
                        ValueLayout.JAVA_DOUBLE, // c_scale
                        ValueLayout.JAVA_DOUBLE, // e_scale
                        ValueLayout.JAVA_DOUBLE, // t_scale
                        ValueLayout.JAVA_DOUBLE, // h_scale
                        ValueLayout.JAVA_DOUBLE, // w_scale
                        ValueLayout.ADDRESS      // out_climate
                )
        );

        MethodHandle setVersion = null;
        MethodHandle getVersion = null;
        MethodHandle setThreads = null;
        MethodHandle getThreads = null;
        MethodHandle generateBatch = null;
        try {
            MemorySegment setVersionSymbol = lookup.find("fastgen_set_version").orElse(null);
            if (setVersionSymbol != null) {
                setVersion = linker.downcallHandle(
                        setVersionSymbol,
                        FunctionDescriptor.ofVoid(ValueLayout.JAVA_INT),
                        Linker.Option.critical(false)
                );
            }
            MemorySegment getVersionSymbol = lookup.find("fastgen_get_version").orElse(null);
            if (getVersionSymbol != null) {
                getVersion = linker.downcallHandle(
                        getVersionSymbol,
                        FunctionDescriptor.of(ValueLayout.JAVA_INT),
                        Linker.Option.critical(false)
                );
            }
            MemorySegment setThreadsSymbol = lookup.find("fastgen_set_worker_threads").orElse(null);
            if (setThreadsSymbol != null) {
                setThreads = linker.downcallHandle(
                        setThreadsSymbol,
                        FunctionDescriptor.of(ValueLayout.JAVA_INT, ValueLayout.JAVA_INT),
                        Linker.Option.critical(false)
                );
            }
            MemorySegment getThreadsSymbol = lookup.find("fastgen_get_worker_threads").orElse(null);
            if (getThreadsSymbol != null) {
                getThreads = linker.downcallHandle(
                        getThreadsSymbol,
                        FunctionDescriptor.of(ValueLayout.JAVA_INT),
                        Linker.Option.critical(false)
                );
            }
            MemorySegment batchSymbol = lookup.find("fastgen_generate_chunks_batch").orElse(null);
            if (batchSymbol != null) {
                generateBatch = linker.downcallHandle(
                        batchSymbol,
                        FunctionDescriptor.ofVoid(
                                ValueLayout.JAVA_LONG, // seed
                                ValueLayout.ADDRESS,   // coords
                                ValueLayout.JAVA_INT,  // count
                                ValueLayout.JAVA_INT,  // minY
                                ValueLayout.JAVA_INT,  // height
                                ValueLayout.ADDRESS,   // out_blocks
                                ValueLayout.ADDRESS    // out_heights
                        )
                );
            }
        } catch (Throwable ignored) {}
        SET_VERSION_HANDLE = setVersion;
        GET_VERSION_HANDLE = getVersion;
        SET_WORKER_THREADS_HANDLE = setThreads;
        GET_WORKER_THREADS_HANDLE = getThreads;
        GENERATE_CHUNKS_BATCH_HANDLE = generateBatch;
    }

    private FastGenBindings() {}

    /**
     * Set target Minecraft world generation version (1 = 26.1, 2 = 26.2).
     */
    public static void setVersion(int version) {
        if (SET_VERSION_HANDLE != null) {
            try {
                SET_VERSION_HANDLE.invokeExact(version);
            } catch (Throwable t) {
                throw new RuntimeException("Error invoking fastgen_set_version", t);
            }
        }
    }

    /**
     * Get active Minecraft world generation version (1 = 26.1, 2 = 26.2).
     */
    public static int getVersion() {
        if (GET_VERSION_HANDLE != null) {
            try {
                return (int) GET_VERSION_HANDLE.invokeExact();
            } catch (Throwable t) {
                return 2;
            }
        }
        return 2;
    }

    /**
     * Set target worker threads count for Rust native Rayon pool.
     */
    public static int setWorkerThreads(int threads) {
        if (SET_WORKER_THREADS_HANDLE != null) {
            try {
                return (int) SET_WORKER_THREADS_HANDLE.invokeExact(threads);
            } catch (Throwable t) {
                throw new RuntimeException("Error invoking fastgen_set_worker_threads", t);
            }
        }
        return threads;
    }

    /**
     * Get active worker threads count in Rust native library.
     */
    public static int getWorkerThreads() {
        if (GET_WORKER_THREADS_HANDLE != null) {
            try {
                return (int) GET_WORKER_THREADS_HANDLE.invokeExact();
            } catch (Throwable t) {
                return 4;
            }
        }
        return 4;
    }

    /**
     * Check if AVX2 SIMD acceleration is supported and active in native library.
     */
    public static boolean hasAvx2() {
        try {
            return ((int) HAS_AVX2_HANDLE.invokeExact()) == 1;
        } catch (Throwable t) {
            return false;
        }
    }

    /**
     * Fast single-column base height lookup. Zero allocations, instant O(1) evaluation in CPU registers.
     * Used by Paper during spawn point selection and structure probes.
     */
    public static int getBaseHeight(long seed, int x, int z, int minY, int maxY) {
        try {
            return (int) GET_BASE_HEIGHT_HANDLE.invokeExact(seed, x, z, minY, maxY);
        } catch (Throwable t) {
            return 64;
        }
    }

    /**
     * Fast 16x16 chunk heightmap array using off-heap native memory.
     */
    public static short[] computeHeightmap(long seed, int chunkX, int chunkZ, int minHeight, int maxHeight) {
        short[] result = new short[256];
        MemorySegment heightSeg = THREAD_HEIGHTS_SEG.get();
        try {
            COMPUTE_HEIGHTMAP_HANDLE.invokeExact(seed, chunkX, chunkZ, minHeight, maxHeight, heightSeg);
            MemorySegment.copy(heightSeg, ValueLayout.JAVA_SHORT, 0, result, 0, 256);
        } catch (Throwable t) {
            throw new RuntimeException("Error invoking fastgen_compute_heightmap", t);
        }
        return result;
    }

    /**
     * Compute 16x16 exact vanilla 1.21 biomes for chunk (256 bytes, values 0..54).
     */
    public static byte[] computeBiomes(long seed, int chunkX, int chunkZ) {
        byte[] result = new byte[256];
        computeBiomes(seed, chunkX, chunkZ, result);
        return result;
    }

    /**
     * Compute 16x16 exact vanilla 1.21 biomes directly into destination array (zero heap allocation).
     */
    public static void computeBiomes(long seed, int chunkX, int chunkZ, byte[] outBiomes) {
        MemorySegment segment = THREAD_BIOMES_SEG.get();
        try {
            COMPUTE_BIOMES_HANDLE.invokeExact(seed, chunkX, chunkZ, segment);
            MemorySegment.copy(segment, ValueLayout.JAVA_BYTE, 0, outBiomes, 0, 256);
        } catch (Throwable t) {
            throw new RuntimeException("Error invoking fastgen_compute_biomes", t);
        }
    }

    /**
     * Get exact vanilla 1.21 biome at any block position (x, y, z).
     */
    public static byte getBiome(long seed, int x, int y, int z) {
        try {
            return (byte) GET_BIOME_HANDLE.invokeExact(seed, x, y, z);
        } catch (Throwable t) {
            throw new RuntimeException("Error invoking fastgen_get_biome", t);
        }
    }

    /**
     * Compute 16x16 5D climate multi-noise points (256 * 5 floats).
     */
    public static float[] computeClimate(long seed, int chunkX, int chunkZ) {
        float[] result = new float[256 * 5];
        MemorySegment segment = THREAD_CLIMATE_SEG.get();
        try {
            COMPUTE_CLIMATE_HANDLE.invokeExact(seed, chunkX, chunkZ, 0.0, 0.0, 0.0, 0.0, 0.0, segment);
            MemorySegment.copy(segment, ValueLayout.JAVA_FLOAT, 0, result, 0, 256 * 5);
        } catch (Throwable t) {
            throw new RuntimeException("Error invoking fastgen_compute_climate", t);
        }
        return result;
    }

    /**
     * 1:1 Complete SteelMC Vanilla Chunk Generation Pipeline.
     * Computes full 3D density, aquifers, fluids, caves, and surface rules.
     * Generates all 16x384x16 blocks directly into outBlocks, and column heightmap into outHeights.
     *
     * Block indices: ((localX * 16 + localZ) * height) + (worldY - minY)
     */
    public static void generateChunk(long seed, int chunkX, int chunkZ, int minY, int height, byte[] outBlocks, short[] outHeights) {
        int totalBlocks = 16 * 16 * height;
        MemorySegment blocksSeg = THREAD_BLOCKS_SEG.get();
        if (blocksSeg.byteSize() < totalBlocks) {
            blocksSeg = Arena.ofAuto().allocate(totalBlocks);
        }
        MemorySegment heightsSeg = (outHeights != null) ? THREAD_HEIGHTS_SEG.get() : MemorySegment.NULL;

        try {
            GENERATE_CHUNK_HANDLE.invokeExact(seed, chunkX, chunkZ, minY, height, blocksSeg, heightsSeg);
            MemorySegment.copy(blocksSeg, ValueLayout.JAVA_BYTE, 0, outBlocks, 0, totalBlocks);
            if (outHeights != null) {
                MemorySegment.copy(heightsSeg, ValueLayout.JAVA_SHORT, 0, outHeights, 0, 256);
            }
        } catch (Throwable t) {
            throw new RuntimeException("Error invoking fastgen_generate_chunk", t);
        }
    }
}
