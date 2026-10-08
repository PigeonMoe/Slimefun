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
                org.bukkit.configuration.file.YamlConfiguration catalog = new org.bukkit.configuration.file.YamlConfiguration();
                for (SlimefunItem item : Slimefun.getRegistry().getAllSlimefunItems()) {
                    ItemStack stack = item.getItem();
                    if (!stack.isEmpty()) {
                        SlimefunItem before = SlimefunItem.getByItem(stack);
                        ItemStack restored = ItemStack.deserializeBytes(stack.serializeAsBytes());
                        require(stack.isSimilar(restored), "Item round trip: " + item.getId());
                        require(before == SlimefunItem.getByItem(restored), "Persistent item ID: " + item.getId());
                        catalog.set(item.getId() + ".name", stack.getItemMeta().getDisplayName());
                        items++;
                    }
                }
                catalog.save("probe-items.yml");
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
                org.bukkit.persistence.PersistentDataContainer selection = Bukkit.getItemFactory().getItemMeta(Material.STONE).getPersistentDataContainer();
                org.bukkit.entity.Player chinese = (org.bukkit.entity.Player) java.lang.reflect.Proxy.newProxyInstance(
                    getClass().getClassLoader(), new Class<?>[] {org.bukkit.entity.Player.class}, (proxy, method, arguments) -> switch (method.getName()) {
                        case "getPersistentDataContainer" -> selection;
                        case "locale" -> java.util.Locale.SIMPLIFIED_CHINESE;
                        default -> throw new UnsupportedOperationException(method.getName());
                    });
                // Explicit selection works without client locale support as well.
                selection.set(Slimefun.getLocalization().getKey(), org.bukkit.persistence.PersistentDataType.STRING, "zh-CN");
                ItemStack canonical = io.github.thebusybiscuit.slimefun4.implementation.SlimefunItems.PORTABLE_CRAFTER.item();
                ItemStack translated = Slimefun.getLocalization().localizeItem(chinese, canonical);
                require(translated.getItemMeta().getDisplayName().contains("便携工作台"), "Chinese guide name");
                require(SlimefunItem.getByItem(canonical) == SlimefunItem.getByItem(translated), "Localized item identity");
                require(!canonical.getItemMeta().getDisplayName().contains("便携"), "Canonical template unchanged");
                for (String language : new String[] {"zh-CN", "zh-TW"}) {
                    selection.set(Slimefun.getLocalization().getKey(), org.bukkit.persistence.PersistentDataType.STRING, language);
                    try (var reader = new java.io.InputStreamReader(Slimefun.class.getResourceAsStream("/languages/" + language + "/items.yml"), java.nio.charset.StandardCharsets.UTF_8)) {
                        var names = org.bukkit.configuration.file.YamlConfiguration.loadConfiguration(reader);
                        for (SlimefunItem registered : Slimefun.getRegistry().getAllSlimefunItems()) {
                            ItemStack original = registered.getItem();
                            if (!original.isEmpty()) {
                                require(names.isString(registered.getId() + ".name"), "Missing " + language + " name: " + registered.getId());
                                ItemStack localized = Slimefun.getLocalization().localizeItem(chinese, original);
                                require(SlimefunItem.getByItem(localized) == SlimefunItem.getByItem(original), "Translated ID: " + registered.getId());
                            }
                        }
                    }
                }
                require(io.github.thebusybiscuit.slimefun4.libraries.dough.items.ItemUtils.getItemName(new ItemStack(Material.STONE)).equals("Stone"), "Public vanilla item names");
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
