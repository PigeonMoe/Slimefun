// SPDX-License-Identifier: GPL-3.0-only
package moe.pigeon.horizon.migration;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.Map;
import org.bukkit.configuration.serialization.ConfigurationSerialization;
import org.bukkit.configuration.serialization.SerializableAs;
import org.bukkit.inventory.ItemStack;

public final class PaperStorageExporterTest {
    private static final String MODERN = """
            preset: ELECTRIC_FURNACE
            '19':
              ==: org.bukkit.inventory.ItemStack
              schema_version: 1
              id: minecraft:copper_ore
              DataVersion: 5013
              count: 3
            """;

    private PaperStorageExporterTest() {}

    public static void main(String[] args) throws Exception {
        Path root = Files.createTempDirectory("slimefun-paper-export-test-");
        try {
            Path source = Files.createDirectory(root.resolve("source"));
            Path menus = Files.createDirectory(source.resolve("stored-inventories"));
            Path blocks = Files.createDirectories(source.resolve("stored-blocks").resolve("world"));
            Path chunks = Files.createDirectory(source.resolve("stored-chunks"));
            Path original = menus.resolve("world;1;72;-10.sfi");
            Files.writeString(original, MODERN);
            Files.writeString(blocks.resolve("0_-1.sfb"), "Paper metadata");
            String oil = "world;Chunk;-1;0: '{\"slimefun-oil\":\"0\"}'\n";
            Files.writeString(chunks.resolve("chunks.sfc"), oil);

            Path output = root.resolve("output");
            Path exported = PaperStorageExporter.exportSnapshot(source, output);
            if (!output.toRealPath().equals(exported)
                    || !MODERN.equals(Files.readString(output.resolve(source.relativize(original))))
                    || !MODERN.equals(Files.readString(original))
                    || !oil.equals(Files.readString(output.resolve("stored-chunks/chunks.sfc")))
                    || !"Paper metadata".equals(Files.readString(
                            output.resolve("stored-blocks/world/0_-1.sfb")))) {
                throw new AssertionError("Snapshot export changed source or modern contents.");
            }
            assertRefused(source, output);
            assertRefused(source, source.resolve("unexpected-nested-output"));

            Path duplicate = menus.resolve("world;2;72;-10.sfi");
            String malformed = MODERN + "'19':\n";
            Files.writeString(duplicate, malformed);
            Path rejected = root.resolve("rejected");
            assertRefused(source, rejected);
            if (Files.exists(rejected) || !malformed.equals(Files.readString(duplicate))) {
                throw new AssertionError("An invalid snapshot published output or altered source.");
            }
            try (var children = Files.list(root)) {
                if (children.anyMatch(path -> path.getFileName().toString().startsWith(".paper-storage-export-"))) {
                    throw new AssertionError("An invalid snapshot left partial staging output.");
                }
            }
            verifyLegacyNormalization();
        } finally {
            try (var paths = Files.walk(root)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) {
                    Files.delete(path);
                }
            }
        }
    }

    private static void verifyLegacyNormalization() throws Exception {
        String alias = "org.bukkit.inventory.ItemStack";
        ConfigurationSerialization.registerClass(LegacyStack.class, alias);
        try {
            String legacy = """
                    preset: ELECTRIC_FURNACE
                    '19':
                      ==: org.bukkit.inventory.ItemStack
                      type: STONE
                      amount: 4
                    """;
            String normalized = PaperStorageExporter.normalize(legacy);
            if (!normalized.contains("schema_version: 1")
                    || !normalized.contains("id: minecraft:stone")
                    || !normalized.contains("count: 4")
                    || !normalized.contains("plugin_byte:1b")
                    || !normalized.equals(PaperStorageExporter.normalize(normalized))) {
                throw new AssertionError("The decoded legacy slot was not exported as components.");
            }
            String profile = """
                    researches:
                      '6': true
                    backpacks:
                      '3':
                        size: 18
                        contents:
                          '17':
                            ==: org.bukkit.inventory.ItemStack
                            type: STONE
                            amount: 4
                    """;
            String exportedProfile = PaperStorageExporter.normalizePlayer(profile);
            if (!exportedProfile.contains("schema_version: 1")
                    || !exportedProfile.contains("researches:")
                    || !exportedProfile.contains("count: 4")
                    || !exportedProfile.equals(PaperStorageExporter.normalizePlayer(exportedProfile))) {
                throw new AssertionError("The backpack slot and sibling profile data changed.");
            }
        } finally {
            ConfigurationSerialization.registerClass(ItemStack.class, alias);
        }
    }

    // Fixture for Bukkit's old-item deserializer. The real server delegate is
    // exercised on Paper, where it upgrades items through Minecraft's registry.
    @SerializableAs("org.bukkit.inventory.ItemStack")
    public static final class LegacyStack extends ItemStack {
        private final int count;

        private LegacyStack(int count) {
            this.count = count;
        }

        public static LegacyStack deserialize(Map<String, Object> data) {
            if (!"STONE".equals(data.get("type")) || !(data.get("amount") instanceof Integer count)) {
                throw new IllegalArgumentException("Malformed legacy test stack.");
            }
            return new LegacyStack(count);
        }

        @Override
        public Map<String, Object> serialize() {
            return Map.of("schema_version", 1, "id", "minecraft:stone", "DataVersion", 5013,
                    "count", count, "components", Map.of("minecraft:custom_data", "{plugin_byte:1b}"));
        }
    }

    private static void assertRefused(Path source, Path output) throws IOException {
        try {
            PaperStorageExporter.exportSnapshot(source, output);
        } catch (IOException expected) {
            return;
        }
        throw new AssertionError("Unexpectedly exported to " + output);
    }
}
