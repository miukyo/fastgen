package com.fastgen;

import com.fastgen.debug.FastGenTracker;
import com.fastgen.generator.FastGenBiomeProvider;
import com.fastgen.generator.FastGenChunkGenerator;
import com.fastgen.nativebridge.FastGenBindings;
import com.fastgen.generator.WorldgenVersion;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.format.NamedTextColor;
import org.bukkit.Chunk;
import org.bukkit.World;
import org.bukkit.block.Biome;
import org.bukkit.command.Command;
import org.bukkit.command.CommandExecutor;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;
import org.bukkit.generator.ChunkGenerator;
import org.bukkit.plugin.java.JavaPlugin;

public final class FastGenPlugin extends JavaPlugin implements CommandExecutor {

    private FastGenTracker tracker;
    private FastGenChunkGenerator chunkGenerator;
    private WorldgenVersion activeVersion = WorldgenVersion.V26_2;

    @Override
    public void onEnable() {
        getLogger().info("Initializing FastGen (Project Panama + Rust Native)...");

        // Warm up and verify native bridge
        try {
            long start = System.nanoTime();
            short[] test = FastGenBindings.computeHeightmap(12345L, 0, 0, -64, 320);
            long elapsed = System.nanoTime() - start;
            boolean avx2 = FastGenBindings.hasAvx2();
            getLogger().info(String.format("FastGen native bridge linked successfully! Warmup: %.3f ms (%d heights, AVX2 SIMD: %s).",
                    elapsed / 1_000_000.0, test.length, avx2 ? "ENABLED" : "DISABLED"));
        } catch (Throwable t) {
            getLogger().severe("Failed to initialize FastGen native library! Disabling plugin.");
            t.printStackTrace();
            getServer().getPluginManager().disablePlugin(this);
            return;
        }

        this.tracker = new FastGenTracker(this);
        this.chunkGenerator = new FastGenChunkGenerator(this);

        // Initialize and bind custom 26.1 / 26.2 biomes
        FastGenBiomeProvider.initBiomes();

        // Configure and apply world generation version target
        saveDefaultConfig();
        String configuredVersion = getConfig().getString("worldgen-version", "auto");
        WorldgenVersion targetVersion;
        if ("auto".equalsIgnoreCase(configuredVersion)) {
            targetVersion = detectServerVersion();
            getLogger().info(String.format("Auto-detected Minecraft worldgen target: %s", targetVersion.getLabel()));
        } else {
            targetVersion = WorldgenVersion.parse(configuredVersion);
            getLogger().info(String.format("Configured Minecraft worldgen target: %s", targetVersion.getLabel()));
        }
        setActiveVersion(targetVersion);

        // Configure worker threads (respects paper-global.yml chunk-system.worker-threads by default)
        int workerThreads = resolveWorkerThreads();
        FastGenBindings.setWorkerThreads(workerThreads);
        com.fastgen.generator.FastGenPrefetcher.init(workerThreads);
        getLogger().info(String.format("Configured worker threads: Java Prefetch=%d, Rust Native (Rayon)=%d",
                workerThreads, FastGenBindings.getWorkerThreads()));

        // Register tracking listener
        getServer().getPluginManager().registerEvents(this.tracker, this);

        // Audit loaded worlds
        for (World w : getServer().getWorlds()) {
            tracker.checkWorldGenerator(w);
        }
    }

    public WorldgenVersion getActiveVersion() {
        return this.activeVersion;
    }

    public void setActiveVersion(WorldgenVersion version) {
        this.activeVersion = version;
        FastGenBindings.setVersion(version.getNativeId());
        FastGenBiomeProvider.initBiomes();
        getLogger().info("Active FastGen worldgen version set to: " + version.getLabel() +
                " (Native ID: " + version.getNativeId() + ", Sulfur Caves Biome: " +
                (FastGenBiomeProvider.VANILLA_BIOMES[45] != null ? FastGenBiomeProvider.VANILLA_BIOMES[45].getKey() : "N/A") + ")");
    }

    private WorldgenVersion detectServerVersion() {
        if (FastGenBiomeProvider.resolveBiome("SULFUR_CAVES", null) != null) {
            return WorldgenVersion.V26_2;
        }

        if (FastGenBiomeProvider.resolveBiome("PALE_GARDEN", null) != null) {
            return WorldgenVersion.V26_1;
        }

        try {
            String mcVer = getServer().getMinecraftVersion();
            if (mcVer != null && mcVer.contains("26.2")) {
                return WorldgenVersion.V26_2;
            } else if (mcVer != null && (mcVer.contains("26.1") || mcVer.contains("1.21"))) {
                return WorldgenVersion.V26_1;
            }
        } catch (Throwable ignored) {}

        return WorldgenVersion.V26_2;
    }

    private int resolveWorkerThreads() {
        if (getConfig().contains("prefetch.threads")) {
            int val = Math.max(0, getConfig().getInt("prefetch.threads"));
            getLogger().info("Prefetch threads explicitly configured in config.yml: " + val);
            return val;
        }
        if (getConfig().contains("prefetch.enabled") && !getConfig().getBoolean("prefetch.enabled")) {
            getLogger().info("Background chunk prefetch disabled via prefetch.enabled = false.");
            return 0;
        }

        String configured = getConfig().getString("worker-threads", "0");
        if (!"auto".equalsIgnoreCase(configured)) {
            try {
                int parsed = Integer.parseInt(configured);
                if (parsed >= 0) {
                    getLogger().info("Worker threads explicitly configured in config.yml: " + parsed);
                    return parsed;
                }
            } catch (NumberFormatException ignored) {}
        }

        // Under "auto": Paper's Moonrise chunk system already drives generation across
        // chunk-system.worker-threads from paper-global.yml. FastGen avoids spawning extra
        // threads by default so total generation threads strictly adhere to paper-global.yml.
        getLogger().info("Worker threads set to 'auto': Paper chunk-system worker threads drive generation directly (0 extra prefetch threads).");
        return 0;
    }

    public FastGenTracker getTracker() {
        return this.tracker;
    }

    @Override
    public void onDisable() {
        com.fastgen.generator.FastGenPrefetcher.shutdown();
        getLogger().info("FastGen disabled.");
    }

    @Override
    public ChunkGenerator getDefaultWorldGenerator(String worldName, String id) {
        return this.chunkGenerator;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (args.length > 0 && args[0].equalsIgnoreCase("check")) {
            handleCheckCommand(sender);
            return true;
        }

        if (args.length > 0 && args[0].equalsIgnoreCase("debug")) {
            if (sender instanceof Player player) {
                boolean enabled = tracker.toggleDebug(player);
                player.sendMessage(Component.text("FastGen live generation debug: " + (enabled ? "ENABLED" : "DISABLED"),
                        enabled ? NamedTextColor.GREEN : NamedTextColor.YELLOW));
            } else {
                sender.sendMessage("Debug toggle is only available for in-game players.");
            }
            return true;
        }

        if (args.length > 0 && args[0].equalsIgnoreCase("bench")) {
            int runs = 1000;
            long start = System.nanoTime();
            for (int i = 0; i < runs; i++) {
                FastGenBindings.computeHeightmap(42L, i, i, -64, 320);
            }
            long totalNanos = System.nanoTime() - start;
            double msPerChunk = (totalNanos / 1_000_000.0) / runs;
            sender.sendMessage(String.format("FastGen Native Benchmark: %d chunks generated in %.2f ms (avg %.3f ms/chunk)",
                    runs, totalNanos / 1_000_000.0, msPerChunk));
            return true;
        }

        if (args.length > 0 && args[0].equalsIgnoreCase("version")) {
            if (args.length > 1) {
                if (!sender.hasPermission("fastgen.admin")) {
                    sender.sendMessage(Component.text("You do not have permission to change worldgen version.", NamedTextColor.RED));
                    return true;
                }
                WorldgenVersion v = WorldgenVersion.parse(args[1]);
                setActiveVersion(v);
                getConfig().set("worldgen-version", v.getLabel());
                saveConfig();
                sender.sendMessage(Component.text("FastGen world generation target set to: " + v.getLabel(), NamedTextColor.GREEN));
                return true;
            }
            sender.sendMessage(Component.text("=== FastGen Worldgen Version ===", NamedTextColor.GOLD));
            sender.sendMessage(Component.text("Active Target: " + activeVersion.getLabel() +
                    " (Native: " + (FastGenBindings.getVersion() == 1 ? "26.1 - 7,593 biomes" : "26.2 - 7,594 biomes") + ")", NamedTextColor.GREEN));
            sender.sendMessage(Component.text("Usage: /fastgen version <26.1|26.2>", NamedTextColor.GRAY));
            return true;
        }

        if (args.length > 0 && args[0].equalsIgnoreCase("locate")) {
            handleLocateCommand(sender, args);
            return true;
        }

        sender.sendMessage(Component.text("=== FastGen Status ===", NamedTextColor.GOLD));
        sender.sendMessage(Component.text("Rust Native: Linked (fastgen_native.dll, AVX2 SIMD: " +
                (FastGenBindings.hasAvx2() ? "ACTIVE" : "SCALAR") + ")", NamedTextColor.GREEN));
        sender.sendMessage(Component.text("Worldgen Target: " + activeVersion.getLabel() + " (Minecraft " +
                (FastGenBindings.getVersion() == 1 ? "26.1" : "26.2") + ")", NamedTextColor.LIGHT_PURPLE));
        sender.sendMessage(Component.text("Total Chunks Generated: " + tracker.getTotalChunks(), NamedTextColor.AQUA));
        sender.sendMessage(Component.text("Commands:", NamedTextColor.GRAY));
        sender.sendMessage(Component.text(" - /fastgen check : Check if current chunk/world was generated by FastGen", NamedTextColor.YELLOW));
        sender.sendMessage(Component.text(" - /fastgen locate [biome] : Instant 3D search for biomes (e.g. sulfur_caves)", NamedTextColor.YELLOW));
        sender.sendMessage(Component.text(" - /fastgen version [26.1|26.2] : View or change active worldgen version", NamedTextColor.YELLOW));
        sender.sendMessage(Component.text(" - /fastgen debug : Toggle live on-screen generation alerts", NamedTextColor.YELLOW));
        sender.sendMessage(Component.text(" - /fastgen bench : Benchmark Rust Panama downcall throughput", NamedTextColor.YELLOW));
        return true;
    }

    private void handleCheckCommand(CommandSender sender) {
        World world;
        int chunkX = 0;
        int chunkZ = 0;
        Chunk chunk = null;

        if (sender instanceof Player player) {
            world = player.getWorld();
            chunk = player.getLocation().getChunk();
            chunkX = chunk.getX();
            chunkZ = chunk.getZ();
        } else {
            world = getServer().getWorlds().get(0);
        }

        boolean isWorldFastGen = tracker.isFastGenWorld(world);
        boolean isChunkFastGen = chunk != null && tracker.isFastGenChunk(chunk);

        sender.sendMessage(Component.text("--------------------------------------------", NamedTextColor.DARK_GRAY));
        sender.sendMessage(Component.text("FastGen World Generation Diagnostic", NamedTextColor.GOLD));
        sender.sendMessage(Component.text("World: " + world.getName(), NamedTextColor.WHITE));

        if (isWorldFastGen) {
            sender.sendMessage(Component.text("World Generator: FastGen (ACTIVE - Rust Native Panama)", NamedTextColor.GREEN));
            sender.sendMessage(Component.text("Worker Threads: " + FastGenBindings.getWorkerThreads() +
                    " (Paper chunk-system synced, Rayon Native Pool)", NamedTextColor.GREEN));
            sender.sendMessage(Component.text("Worldgen Rules: Minecraft " + activeVersion.getLabel() +
                    " (Native: " + (FastGenBindings.getVersion() == 1 ? "26.1 - 7,593 biomes" : "26.2 - 7,594 biomes") + ")", NamedTextColor.AQUA));
        } else {
            sender.sendMessage(Component.text("World Generator: NOT FastGen (Currently using: " +
                    (world.getGenerator() == null ? "Vanilla" : world.getGenerator().getClass().getSimpleName()) + ")", NamedTextColor.RED));
            sender.sendMessage(Component.text("⚠ To generate world with FastGen, add to bukkit.yml:", NamedTextColor.YELLOW));
            sender.sendMessage(Component.text("  worlds:\n    " + world.getName() + ":\n      generator: FastGen", NamedTextColor.GRAY));
        }

        if (chunk != null) {
            if (isChunkFastGen) {
                sender.sendMessage(Component.text(String.format("Chunk [%d, %d]: GENERATED BY FASTGEN ✔", chunkX, chunkZ), NamedTextColor.GREEN));
            } else {
                sender.sendMessage(Component.text(String.format("Chunk [%d, %d]: GENERATED BY VANILLA (Not FastGen)", chunkX, chunkZ), NamedTextColor.RED));
            }

            // Sample native calculation at chunk coordinates
            byte[] biomes = FastGenBindings.computeBiomes(world.getSeed(), chunkX, chunkZ);
            short[] heights = FastGenBindings.computeHeightmap(world.getSeed(), chunkX, chunkZ, world.getMinHeight(), world.getMaxHeight());
            int centerBiomeId = biomes[8 * 16 + 8] & 0xFF;
            Biome centerBiome = (centerBiomeId < FastGenBiomeProvider.VANILLA_BIOMES.length)
                    ? FastGenBiomeProvider.VANILLA_BIOMES[centerBiomeId]
                    : Biome.PLAINS;
            short centerHeight = heights[8 * 16 + 8];


            sender.sendMessage(Component.text(String.format(
                    "Native Metrics: Biome=%s, SurfaceHeight=%d",
                    centerBiome.toString(), centerHeight
            ), NamedTextColor.AQUA));
        }

        sender.sendMessage(Component.text("Total FastGen Chunks Generated: " + tracker.getTotalChunks(), NamedTextColor.LIGHT_PURPLE));
        sender.sendMessage(Component.text("--------------------------------------------", NamedTextColor.DARK_GRAY));
    }

    private void handleLocateCommand(CommandSender sender, String[] args) {
        World world = (sender instanceof Player p) ? p.getWorld() : getServer().getWorlds().get(0);
        long seed = world.getSeed();

        String query = (args.length > 1) ? args[1].toLowerCase() : "sulfur";
        int targetId = -1;
        String targetName = query;

        if (query.contains("sulfur")) {
            targetId = 45;
            targetName = "minecraft:sulfur_caves";
        } else if (query.contains("lush")) {
            targetId = 25;
            targetName = "minecraft:lush_caves";
        } else if (query.contains("dripstone")) {
            targetId = 13;
            targetName = "minecraft:dripstone_caves";
        } else if (query.contains("deep_dark")) {
            targetId = 8;
            targetName = "minecraft:deep_dark";
        } else {
            for (int i = 0; i < FastGenBiomeProvider.VANILLA_BIOMES.length; i++) {
                if (FastGenBiomeProvider.VANILLA_BIOMES[i].name().toLowerCase().contains(query)) {
                    targetId = i;
                    targetName = FastGenBiomeProvider.VANILLA_BIOMES[i].getKey().toString();
                    break;
                }
            }
        }

        if (targetId == -1) {
            sender.sendMessage(Component.text("Unknown biome: " + query, NamedTextColor.RED));
            return;
        }

        int startX = 0;
        int startZ = 0;
        if (sender instanceof Player p) {
            startX = p.getLocation().getBlockX();
            startZ = p.getLocation().getBlockZ();
        }

        sender.sendMessage(Component.text("Searching for " + targetName + " around [" + startX + ", " + startZ + "] on seed " + seed + "...", NamedTextColor.YELLOW));

        long startTime = System.nanoTime();
        int[] caveYs = (targetId == 8) ? new int[]{-40, -20} : new int[]{20, 35, 50, 70};

        int foundX = 0, foundY = 0, foundZ = 0;
        boolean found = false;

        searchLoop:
        for (int r = 0; r <= 200; r++) {
            for (int dx = -r; dx <= r; dx++) {
                if (dx == -r || dx == r) {
                    for (int dz = -r; dz <= r; dz++) {
                        int x = startX + (dx * 16);
                        int z = startZ + (dz * 16);
                        for (int y : caveYs) {
                            if ((FastGenBindings.getBiome(seed, x, y, z) & 0xFF) == targetId) {
                                found = true;
                                foundX = x;
                                foundY = y;
                                foundZ = z;
                                break searchLoop;
                            }
                        }
                    }
                } else {
                    int x = startX + (dx * 16);
                    for (int dz : new int[]{-r, r}) {
                        int z = startZ + (dz * 16);
                        for (int y : caveYs) {
                            if ((FastGenBindings.getBiome(seed, x, y, z) & 0xFF) == targetId) {
                                found = true;
                                foundX = x;
                                foundY = y;
                                foundZ = z;
                                break searchLoop;
                            }
                        }
                    }
                }
            }
        }

        long elapsedMs = (System.nanoTime() - startTime) / 1_000_000;
        if (found) {
            int dist = (int) Math.hypot(foundX - startX, foundZ - startZ);
            sender.sendMessage(Component.text(String.format("Found %s at X: %d, Y: %d, Z: %d (%d blocks away in %d ms)",
                    targetName, foundX, foundY, foundZ, dist, elapsedMs), NamedTextColor.GREEN));
            if (sender instanceof Player) {
                sender.sendMessage(Component.text("/tp " + foundX + " " + foundY + " " + foundZ, NamedTextColor.AQUA));
            }
        } else {
            sender.sendMessage(Component.text("Could not find " + targetName + " within 3,200 blocks (" + elapsedMs + " ms).", NamedTextColor.RED));
        }
    }
}
