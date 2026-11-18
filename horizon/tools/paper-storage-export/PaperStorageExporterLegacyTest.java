// SPDX-License-Identifier: GPL-3.0-only
package moe.pigeon.horizon.migration;

import java.io.IOException;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
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
import org.bukkit.Server;
import org.bukkit.configuration.serialization.ConfigurationSerialization;
import org.bukkit.craftbukkit.CraftRegistry;
import org.bukkit.craftbukkit.inventory.CraftItemFactory;
import org.bukkit.craftbukkit.inventory.SerializableMeta;
import org.bukkit.craftbukkit.util.CraftMagicNumbers;
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
