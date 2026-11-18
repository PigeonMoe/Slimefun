// SPDX-License-Identifier: GPL-3.0-only
package moe.pigeon.horizon.migration;

import io.github.thebusybiscuit.slimefun4.core.services.sync.PaperSyncedProfilesFixture;
import java.io.IOException;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HexFormat;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.logging.Logger;
import net.minecraft.SharedConstants;
import net.minecraft.commands.Commands;
import net.minecraft.nbt.TagParser;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.WorldLoader;
import net.minecraft.server.packs.repository.ServerPacksSource;
import net.minecraft.server.permissions.LevelBasedPermissionSet;
import net.minecraft.util.Util;
import net.minecraft.world.level.WorldDataConfiguration;
import org.bukkit.Bukkit;
import org.bukkit.Material;
import org.bukkit.NamespacedKey;
import org.bukkit.Server;
import org.bukkit.configuration.serialization.ConfigurationSerialization;
import org.bukkit.craftbukkit.CraftRegistry;
import org.bukkit.craftbukkit.inventory.CraftItemFactory;
import org.bukkit.craftbukkit.inventory.SerializableMeta;
import org.bukkit.craftbukkit.util.CraftMagicNumbers;
import org.bukkit.inventory.ItemStack;
import org.bukkit.persistence.PersistentDataType;
import org.yaml.snakeyaml.Yaml;

public final class PaperStorageExporterLegacyTest {
    private PaperStorageExporterLegacyTest() {}

    public static void main(String[] args) throws Exception {
        initializePaper();
        Path fixtures = Path.of(args[0]);
        String skull = Files.readString(fixtures.resolve("paper-1.16.1-skull.sfi"));
        String egg = Files.readString(fixtures.resolve("paper-1.20.4-spawn-egg.sfi"));
        Path source = Files.createDirectory(Path.of("legacy-source"));
        Path stored = Files.createDirectory(source.resolve("stored-inventories"));
        Path skullFile = stored.resolve("world;1;64;2.sfi");
        Path eggFile = stored.resolve("world;2;64;2.sfi");
        Files.writeString(skullFile, skull);
        String broken = egg.replaceFirst("internal: .*", "internal: AAAA");
        Files.writeString(eggFile, broken);
        Path result = Path.of("legacy-exported");
        verifyRefused(source, result, "Invalid compressed NBT", eggFile, broken);
        String unknown = egg.replace("meta-type: SPAWN_EGG", "meta-type: UNRECOGNIZED");
        Files.writeString(eggFile, unknown);
        verifyRefused(source, result, "Paper could not deserialize ItemMeta", eggFile, unknown);

        Files.writeString(eggFile, egg);
        PaperStorageExporter.exportSnapshot(source, result);
        Path repeated = Path.of("legacy-repeated");
        PaperStorageExporter.exportSnapshot(source, repeated);
        for (Path file : new Path[] {skullFile, eggFile}) {
            Path relative = source.relativize(file);
            if (Files.mismatch(result.resolve(relative), repeated.resolve(relative)) != -1) {
                throw new AssertionError("Repeating the export changed " + relative);
            }
        }
        String profile = component(result.resolve(source.relativize(skullFile)), "19", "minecraft:profile");
        if (!TagParser.parseCompoundFully(profile).equals(TagParser.parseCompoundFully(
                "{id:[I;698614827,95112306,-1805616477,-1251481382],name:\"DerFrZocker\"}"))) {
            throw new AssertionError("The historic skull owner or UUID changed: " + profile);
        }
        String pluginData = component(result.resolve(source.relativize(eggFile)), "24", "minecraft:custom_data");
        if (!TagParser.parseCompoundFully(pluginData).equals(TagParser.parseCompoundFully(
                "{Boss_NbtTag:{Boss_V4:\"LavaLurker\"}}"))) {
            throw new AssertionError("The historic plugin NBT changed: " + pluginData);
        }
        if (!skull.equals(Files.readString(skullFile)) || !egg.equals(Files.readString(eggFile))) {
            throw new AssertionError("The source snapshot changed during export.");
        }

        verifySyncedProfiles();
    }

    private static void verifySyncedProfiles() throws Exception {
        UUID owner = UUID.fromString("8c0e7dd0-005e-48a5-9d49-28afb237bc91");
        UUID wrong = UUID.fromString("fc514949-28a0-493e-8648-89a247186e15");
        ItemStack paperItem = new ItemStack(Material.DIAMOND, 3);
        NamespacedKey marker = new NamespacedKey("migration_fixture", "original");
        paperItem.editMeta(meta -> meta.getPersistentDataContainer().set(marker, PersistentDataType.STRING, "saved"));
        byte[] archived = PaperSyncedProfilesFixture.envelope(owner, paperItem.serializeAsBytes());
        String row = sqlRow(owner, archived);
        Path source = Path.of("snapshot.tsv");
        Path output = Path.of("snapshot-exported");
        Path repeated = Path.of("snapshot-repeated");
        byte[] corrupt = archived.clone();
        corrupt[10] ^= 1;
        byte[] invalidItem = PaperSyncedProfilesFixture.envelope(wrong, new byte[] {1, 2, 3});
        for (String invalid : List.of(row + sqlRow(owner, archived), sqlRow(wrong, archived),
                sqlRow(owner, corrupt), row + sqlRow(wrong, invalidItem))) {
            Files.writeString(source, invalid);
            try {
                PaperStorageExporter.exportSyncedProfiles(source, output);
                throw new AssertionError("Invalid or conflicting SQL rows were published.");
            } catch (IOException expected) {
                if (!expected.getMessage().startsWith("PM-Sync row ") || Files.exists(output)
                        || !invalid.equals(Files.readString(source))) {
                    throw new AssertionError("The rejected PM-Sync export touched its input or destination.", expected);
                }
            }
        }

        Files.writeString(source, row);
        PaperStorageExporter.exportSyncedProfiles(source, output);
        PaperStorageExporter.exportSyncedProfiles(source, repeated);
        Path profile = Path.of("Players", owner + ".yml");
        if (Files.mismatch(output.resolve(profile), repeated.resolve(profile)) != -1) {
            throw new AssertionError("An unchanged PM-Sync export published different bytes.");
        }
        Map<?, ?> exported = new Yaml().load(Files.readString(output.resolve(profile)));
        Map<?, ?> bags = (Map<?, ?>) exported.get("backpacks");
        Map<?, ?> bag = (Map<?, ?>) bags.get("14");
        Map<?, ?> slots = (Map<?, ?>) bag.get("contents");
        if (!row.equals(Files.readString(source)) || !Integer.valueOf(9).equals(bag.get("size"))
                || !slots.keySet().equals(java.util.Set.of("7"))
                || !List.of("addon:research", "slimefun:lumps").equals(exported.get("pm-research-keys"))
                || !Map.of("31", true).equals(exported.get("researches"))) {
            throw new AssertionError("The SQL export changed the owners, research or slot placement.");
        }
        Map<?, ?> stack = (Map<?, ?>) slots.get("7");
        Map<?, ?> components = (Map<?, ?>) stack.get("components");
        if (!Integer.valueOf(3).equals(stack.get("count"))
                || !TagParser.parseCompoundFully((String) components.get("minecraft:custom_data")).equals(
                        TagParser.parseCompoundFully("{PublicBukkitValues:{\"migration_fixture:original\":\"saved\"}}"))) {
            throw new AssertionError("Native Paper ItemStack bytes lost their count or persistent data.");
        }
    }

    private static String sqlRow(UUID owner, byte[] value) {
        return owner + "\t18\t" + HexFormat.of().formatHex(value) + "\n";
    }

    private static void verifyRefused(Path source, Path result, String error, Path file, String original)
            throws IOException {
        try {
            PaperStorageExporter.exportSnapshot(source, result);
            throw new AssertionError("A failed legacy slot published a partial snapshot.");
        } catch (IOException expected) {
            if (!expected.getMessage().contains("slot 24") || !expected.getMessage().contains(error)) {
                throw new AssertionError("The error does not identify the failed slot and cause.", expected);
            }
        }
        try (var files = Files.list(Path.of("."))) {
            if (Files.exists(result) || !original.equals(Files.readString(file))
                    || files.anyMatch(path -> path.getFileName().toString().startsWith(".paper-storage-export-"))) {
                throw new AssertionError("A refused legacy slot changed the source or left partial output.");
            }
        }
    }

    private static String component(Path file, String slot, String component) throws Exception {
        String data = Files.readString(file);
        if (!data.equals(PaperStorageExporter.normalize(data))) {
            throw new AssertionError("An exported inventory changed when normalized again.");
        }
        Map<?, ?> root = new Yaml().load(data);
        if (root.get(slot) instanceof Map<?, ?> item && Integer.valueOf(1).equals(item.get("schema_version"))
                && item.get("components") instanceof Map<?, ?> components
                && components.get(component) instanceof String snbt) {
            return snbt;
        }
        throw new AssertionError("No typed " + component + " in slot " + slot);
    }

    private static void initializePaper() {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
        var packs = new WorldLoader.PackConfig(ServerPacksSource.createVanillaTrustedRepository(),
                WorldDataConfiguration.DEFAULT, false, true);
        var loading = new WorldLoader.InitConfig(packs, Commands.CommandSelection.DEDICATED,
                LevelBasedPermissionSet.ALL_PERMISSIONS);
        var registries = WorldLoader.load(loading,
                context -> new WorldLoader.DataLoadOutput<>(null, context.datapackDimensions()),
                (resources, datapack, layers, unused) -> layers.compositeAccess(),
                Util.backgroundExecutor(), Runnable::run).join();
        CraftRegistry.setMinecraftRegistry(registries);

        // No dedicated server, worlds, network listeners, or EULA are needed for the Paper codecs.
        Server fixture = (Server) Proxy.newProxyInstance(Server.class.getClassLoader(),
                new Class<?>[] {Server.class}, (proxy, method, arguments) -> switch (method.getName()) {
                    case "getUnsafe" -> CraftMagicNumbers.INSTANCE;
                    case "getItemFactory" -> CraftItemFactory.instance();
                    case "getName", "getVersion", "getBukkitVersion" -> "Paper migration fixture";
                    case "getLogger" -> Logger.getLogger("Paper migration fixture");
                    default -> throw new UnsupportedOperationException(method.toString());
                });
        Bukkit.setServer(fixture);
        ConfigurationSerialization.registerClass(SerializableMeta.class);
    }
}
