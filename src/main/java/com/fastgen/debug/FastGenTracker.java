package com.fastgen.debug;

import com.fastgen.FastGenPlugin;
import com.fastgen.generator.FastGenBiomeProvider;
import com.fastgen.generator.FastGenChunkGenerator;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.format.NamedTextColor;
import org.bukkit.Bukkit;
import org.bukkit.Chunk;
import org.bukkit.NamespacedKey;
import org.bukkit.World;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.world.ChunkLoadEvent;
import org.bukkit.event.world.WorldInitEvent;
import org.bukkit.event.world.WorldLoadEvent;
import org.bukkit.persistence.PersistentDataType;

import java.util.Collections;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.atomic.AtomicLong;

public final class FastGenTracker implements Listener {

    private final FastGenPlugin plugin;
    private final NamespacedKey pdcKey;
    private final AtomicLong totalChunks = new AtomicLong(0);
    private final Set<UUID> debugPlayers = ConcurrentHashMap.newKeySet();

    public FastGenTracker(FastGenPlugin plugin) {
        this.plugin = plugin;
        this.pdcKey = new NamespacedKey(plugin, "fastgen_generated");
    }

    public void recordChunk(String worldName, int chunkX, int chunkZ, long elapsedNanos, short height) {
        long count = totalChunks.incrementAndGet();

        // Periodic console indicators so admins see FastGen actively working
        if (count == 1 || count % 500 == 0) {
            double ms = elapsedNanos / 1_000_000.0;
            plugin.getLogger().info(String.format(
                    "★ [FastGen ACTIVE] %s chunk [%d, %d] in '%s' (Rust Native: %.3f ms, Height: %d, Total: %d chunks).",
                    count == 1 ? "First" : "Progress", chunkX, chunkZ, worldName, ms, height, count
            ));
        }

        // Live in-game debug indicators for subscribed players
        if (!debugPlayers.isEmpty()) {
            double ms = elapsedNanos / 1_000_000.0;
            Component msg = Component.text(String.format(
                    "[FastGen] Chunk [%d, %d] | Height: %d | Time: %.2f ms",
                    chunkX, chunkZ, height, ms
            ), NamedTextColor.GREEN);

            for (UUID uuid : debugPlayers) {
                Player p = Bukkit.getPlayer(uuid);
                if (p != null && p.isOnline() && p.getWorld().getName().equals(worldName)) {
                    p.sendActionBar(msg);
                }
            }
        }
    }

    public boolean isFastGenChunk(Chunk chunk) {
        return chunk.getWorld().getGenerator() instanceof FastGenChunkGenerator;
    }

    public boolean isFastGenWorld(World world) {
        return world.getGenerator() instanceof FastGenChunkGenerator;
    }

    public boolean toggleDebug(Player player) {
        if (debugPlayers.contains(player.getUniqueId())) {
            debugPlayers.remove(player.getUniqueId());
            return false;
        } else {
            debugPlayers.add(player.getUniqueId());
            return true;
        }
    }

    public long getTotalChunks() {
        return totalChunks.get();
    }

    @EventHandler
    public void onWorldInit(WorldInitEvent event) {
        checkWorldGenerator(event.getWorld());
    }

    @EventHandler
    public void onWorldLoad(WorldLoadEvent event) {
        checkWorldGenerator(event.getWorld());
    }

    public void checkWorldGenerator(World world) {
        if (isFastGenWorld(world)) {
            plugin.getLogger().info(String.format(
                    "✔ [FastGen STATUS] World '%s' is ACTIVE with FastGen ChunkGenerator (Rust Native Panama).",
                    world.getName()
            ));
        } else {
            plugin.getLogger().warning(String.format(
                    "⚠ [FastGen STATUS] World '%s' generator is NOT set to FastGen (currently: %s).",
                    world.getName(),
                    world.getGenerator() == null ? "Vanilla" : world.getGenerator().getClass().getSimpleName()
            ));
            plugin.getLogger().warning(String.format(
                    "⚠ To activate FastGen for '%s', add this to bukkit.yml:\n  worlds:\n    %s:\n      generator: FastGen",
                    world.getName(), world.getName()
            ));
        }
    }
}
