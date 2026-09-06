package com.fastgen.generator;

import org.bukkit.Material;
import org.bukkit.block.CreatureSpawner;
import org.bukkit.entity.EntityType;
import org.bukkit.generator.BlockPopulator;
import org.bukkit.generator.LimitedRegion;
import org.bukkit.generator.WorldInfo;

import java.util.Random;

/**
 * Plugin-side populator for custom structures, bonus ores, and dungeon features.
 * Vanilla structures, decorations, and mob spawns are handled natively by the Paper pipeline.
 */
public class FastGenBlockPopulator extends BlockPopulator {

    @Override
    public void populate(WorldInfo worldInfo, Random random, int chunkX, int chunkZ, LimitedRegion limitedRegion) {
        int worldX = chunkX * 16;
        int worldZ = chunkZ * 16;

        // Custom Java-side underground dungeon feature (5% chance per chunk)
        if (random.nextInt(100) < 5) {
            populateDungeon(worldX, worldZ, random, limitedRegion);
        }

        // Custom Java-side bonus ore pockets
        populateBonusOres(worldX, worldZ, random, limitedRegion);
    }

    private void populateDungeon(int worldX, int worldZ, Random random, LimitedRegion region) {
        int cx = worldX + 8;
        int cz = worldZ + 8;
        int cy = random.nextInt(40) - 20; // y: -20 to 20

        // 7x7 Cobble room
        for (int dx = -3; dx <= 3; dx++) {
            for (int dz = -3; dz <= 3; dz++) {
                int px = cx + dx;
                int pz = cz + dz;

                if (!region.isInRegion(px, cy, pz)) continue;

                // Floor
                region.setType(px, cy, pz, random.nextBoolean() ? Material.MOSSY_COBBLESTONE : Material.COBBLESTONE);
                // Ceiling
                region.setType(px, cy + 4, pz, Material.COBBLESTONE);

                for (int dy = 1; dy <= 3; dy++) {
                    boolean isWall = dx == -3 || dx == 3 || dz == -3 || dz == 3;
                    if (isWall) {
                        region.setType(px, cy + dy, pz, random.nextInt(3) == 0 ? Material.MOSSY_COBBLESTONE : Material.COBBLESTONE);
                    } else {
                        region.setType(px, cy + dy, pz, Material.CAVE_AIR);
                    }
                }
            }
        }

        // Central monster spawner
        if (region.isInRegion(cx, cy + 1, cz)) {
            region.setType(cx, cy + 1, cz, Material.SPAWNER);
            try {
                if (region.getBlockState(cx, cy + 1, cz) instanceof CreatureSpawner spawner) {
                    spawner.setSpawnedType(random.nextBoolean() ? EntityType.SKELETON : EntityType.ZOMBIE);
                    spawner.update();
                }
            } catch (Exception ignored) {}
        }

        // Chests on walls
        if (region.isInRegion(cx - 2, cy + 1, cz)) region.setType(cx - 2, cy + 1, cz, Material.CHEST);
        if (region.isInRegion(cx + 2, cy + 1, cz)) region.setType(cx + 2, cy + 1, cz, Material.CHEST);
    }

    private void populateBonusOres(int worldX, int worldZ, Random random, LimitedRegion region) {
        // Bonus deepslate diamond vein
        if (random.nextInt(100) < 15) {
            int ox = worldX + random.nextInt(16);
            int oy = -60 + random.nextInt(40);
            int oz = worldZ + random.nextInt(16);

            for (int s = 0; s < 4; s++) {
                int px = ox + random.nextInt(3) - 1;
                int py = oy + random.nextInt(3) - 1;
                int pz = oz + random.nextInt(3) - 1;

                if (region.isInRegion(px, py, pz) && region.getType(px, py, pz) == Material.DEEPSLATE) {
                    region.setType(px, py, pz, Material.DEEPSLATE_DIAMOND_ORE);
                }
            }
        }
    }
}
