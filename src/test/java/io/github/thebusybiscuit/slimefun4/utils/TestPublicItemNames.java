package io.github.thebusybiscuit.slimefun4.utils;

import io.github.bakedlibs.dough.items.ItemUtils;
import org.bukkit.Material;
import org.bukkit.inventory.ItemStack;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockbukkit.mockbukkit.MockBukkit;
import static org.junit.jupiter.api.Assertions.*;

class TestPublicItemNames {
    @BeforeEach void load() { MockBukkit.mock(); }
    @AfterEach void unload() { MockBukkit.unmock(); }
    @Test void namesDoNotDependOnServerVersionParsing() {
        assertEquals("null", ItemUtils.getItemName(null));
        ItemStack named = new ItemStack(Material.STONE);
        var meta = named.getItemMeta();
        meta.setDisplayName("测试名称");
        named.setItemMeta(meta);
        assertEquals("测试名称", ItemUtils.getItemName(named));
    }
}
