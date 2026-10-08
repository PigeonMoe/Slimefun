package io.github.thebusybiscuit.slimefun4.core.services.localization;

import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.HashSet;
import java.util.Set;
import java.util.regex.Pattern;

import org.bukkit.configuration.file.YamlConfiguration;
import org.bukkit.inventory.ItemStack;
import org.bukkit.persistence.PersistentDataType;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.mockbukkit.mockbukkit.MockBukkit;
import org.mockbukkit.mockbukkit.entity.PlayerMock;

import io.github.thebusybiscuit.slimefun4.core.services.LocalizationService;
import io.github.thebusybiscuit.slimefun4.implementation.Slimefun;
import io.github.thebusybiscuit.slimefun4.implementation.SlimefunItems;

import static org.junit.jupiter.api.Assertions.*;

class TestChineseLocalization {
    private LocalizationService localization;
    private PlayerMock player;
    private Slimefun plugin;

    @BeforeEach
    void load() throws Exception {
        var server = MockBukkit.mock();
        plugin = MockBukkit.load(Slimefun.class);
        Slimefun.getCfg().setValue("options.enable-translations", true);
        Slimefun.getCfg().setValue("options.auto-detect-language", true);
        localization = new LocalizationService(plugin, "", "en");
        var field = Slimefun.class.getDeclaredField("local");
        field.setAccessible(true);
        field.set(plugin, localization);
        player = server.addPlayer();
    }

    @AfterEach
    void unload() {
        MockBukkit.unmock();
    }

    @Test
    void aliasesAndExplicitSelection() {
        assertEquals("zh-CN", LanguagePreset.normalize("zh_Hans_CN"));
        assertEquals("zh-TW", LanguagePreset.normalize("zh_HK"));
        assertEquals("zh-TW", LanguagePreset.normalize("zh-Hant-TW"));
        assertEquals("en", LanguagePreset.normalize("en_US"));
        assertSame(localization.getLanguage("zh-CN"), localization.getLanguage("zh_CN"));
        player.setLocale(java.util.Locale.SIMPLIFIED_CHINESE);
        assertEquals("zh-CN", localization.getLanguage(player).getId());
        player.getPersistentDataContainer().set(localization.getKey(), PersistentDataType.STRING, "zh-TW");
        assertEquals("zh-TW", localization.getLanguage(player).getId());
        player.getPersistentDataContainer().remove(localization.getKey());
        player.setLocale(java.util.Locale.forLanguageTag("unknown-XX"));
        assertEquals("en", localization.getLanguage(player).getId());
    }

    @Test
    void guideCopiesKeepIdentityAndDoNotModifyStoredItems() {
        ItemStack original = SlimefunItems.PORTABLE_CRAFTER.item();
        ItemStack before = original.clone();
        player.setLocale(java.util.Locale.SIMPLIFIED_CHINESE);
        ItemStack chinese = localization.localizeItem(player, original);
        assertTrue(chinese.getItemMeta().getDisplayName().contains("便携工作台"));
        assertEquals(original.getItemMeta().getPersistentDataContainer(), chinese.getItemMeta().getPersistentDataContainer());
        assertEquals(before, original);
        player.setLocale(java.util.Locale.TRADITIONAL_CHINESE);
        assertTrue(localization.localizeItem(player, original).getItemMeta().getDisplayName().contains("便攜工作臺"));
        assertNull(localization.localizeItem(player, null));
        assertEquals(new ItemStack(org.bukkit.Material.STONE), localization.localizeItem(player, new ItemStack(org.bukkit.Material.STONE)));
    }

    @Test
    void chineseResourcesCoverEnglishKeysAndPreservePlaceholders() throws Exception {
        Pattern placeholders = Pattern.compile("%[\\w-]+%");
        for (LanguageFile file : LanguageFile.values()) {
            YamlConfiguration english = read(file, "en");
            for (String language : new String[] {"zh-CN", "zh-TW"}) {
                YamlConfiguration translated = read(file, language);
                for (String key : english.getKeys(true)) {
                    if (english.isConfigurationSection(key)) {
                        continue;
                    }
                    assertTrue(translated.contains(key), language + ": " + file + ": " + key);
                    Set<String> expected = new HashSet<>();
                    Set<String> actual = new HashSet<>();
                    placeholders.matcher(String.valueOf(english.get(key))).results().forEach(m -> expected.add(m.group()));
                    placeholders.matcher(String.valueOf(translated.get(key))).results().forEach(m -> actual.add(m.group()));
                    assertEquals(expected, actual, language + ": " + file + ": " + key);
                }
            }
        }
    }

    @Test
    void guideSearchAcceptsChineseEnglishAndItemId() throws Exception {
        var item = io.github.thebusybiscuit.slimefun4.test.TestUtilities.mockSlimefunItem(
            plugin, "PORTABLE_CRAFTER", SlimefunItems.PORTABLE_CRAFTER.item());
        item.register(plugin);
        player.setLocale(java.util.Locale.SIMPLIFIED_CHINESE);
        var profile = io.github.thebusybiscuit.slimefun4.test.TestUtilities.awaitProfile(player);
        var guide = new io.github.thebusybiscuit.slimefun4.implementation.guide.SurvivalSlimefunGuide(false, false);
        for (String query : new String[] {"便携工作台", "Portable Crafter", "PORTABLE_CRAFTER"}) {
            guide.openSearch(profile, query, false);
            assertTrue(java.util.Arrays.stream(player.getOpenInventory().getTopInventory().getContents())
                .filter(java.util.Objects::nonNull)
                .anyMatch(stack -> stack.hasItemMeta() && stack.getItemMeta().getDisplayName().contains("便携工作台")), query);
        }
    }

    private YamlConfiguration read(LanguageFile file, String language) throws Exception {
        try (var stream = getClass().getResourceAsStream(file.getFilePath(language));
             var reader = new InputStreamReader(stream, StandardCharsets.UTF_8)) {
            return YamlConfiguration.loadConfiguration(reader);
        }
    }
}
