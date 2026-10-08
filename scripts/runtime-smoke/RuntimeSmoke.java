package dev.pigeonmoe.slimefun.testing;

import io.github.thebusybiscuit.slimefun4.api.items.SlimefunItem;
import io.github.thebusybiscuit.slimefun4.implementation.Slimefun;
import io.github.thebusybiscuit.slimefun4.utils.CustomHeads;
import io.github.thebusybiscuit.slimefun4.utils.HeadTexture;
import org.bukkit.Bukkit;
import org.bukkit.Material;
import org.bukkit.block.Block;
import org.bukkit.block.Skull;
import org.bukkit.inventory.ItemStack;
import org.bukkit.inventory.meta.SkullMeta;
import org.bukkit.plugin.java.JavaPlugin;

/** Integration probes that run on a real, isolated Paper server. */
public final class RuntimeSmoke extends JavaPlugin {
    @Override
    public void onEnable() {
        Bukkit.getScheduler().runTaskLater(this, () -> {
            try {
                require(Slimefun.instance() != null && Slimefun.instance().isEnabled(), "Slimefun must enable");
                int items = 0;
                for (SlimefunItem item : Slimefun.getRegistry().getAllSlimefunItems()) {
                    ItemStack stack = item.getItem();
                    if (!stack.isEmpty()) {
                        SlimefunItem before = SlimefunItem.getByItem(stack);
                        ItemStack restored = ItemStack.deserializeBytes(stack.serializeAsBytes());
                        require(stack.isSimilar(restored), "Item round trip: " + item.getId());
                        require(before == SlimefunItem.getByItem(restored), "Persistent item ID: " + item.getId());
                        items++;
                    }
                }
                require(items >= 500, "Core item registration: " + items);
                for (HeadTexture texture : HeadTexture.values()) {
                    SkullMeta meta = (SkullMeta) CustomHeads.getItemStack(texture.getTexture()).getItemMeta();
                    require(meta.getOwnerProfile() != null && meta.getOwnerProfile().getTextures().getSkin() != null,
                        "Head texture: " + texture.name());
                }
                Block block = Bukkit.getWorlds().getFirst().getBlockAt(0, 100, 0);
                block.setType(Material.PLAYER_HEAD);
                CustomHeads.setSkin(block, HeadTexture.CAPACITOR_100.getTexture(), false);
                require(((Skull) block.getState()).getOwnerProfile().getTextures().getSkin() != null, "Placed head texture");
                block.setType(Material.AIR);
                require(Slimefun.getLocalization().getLanguage("zh-CN") != null, "Simplified Chinese language");
                require(Slimefun.getLocalization().getLanguage("zh-TW") != null, "Traditional Chinese language");
                Bukkit.dispatchCommand(Bukkit.getConsoleSender(), "sf versions");
                getLogger().info("SLIMEFUN_SMOKE_OK minecraft=" + Bukkit.getMinecraftVersion() + " items=" + items);
            } catch (Throwable failure) {
                getLogger().log(java.util.logging.Level.SEVERE, "SLIMEFUN_SMOKE_FAILED", failure);
            } finally {
                Bukkit.shutdown();
            }
        }, 100L);
    }

    private static void require(boolean success, String description) {
        if (!success) {
            throw new IllegalStateException(description);
        }
    }
}
