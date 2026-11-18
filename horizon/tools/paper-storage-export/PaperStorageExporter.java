// SPDX-License-Identifier: GPL-3.0-only
package moe.pigeon.horizon.migration;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.command.ConsoleCommandSender;
import org.bukkit.inventory.ItemStack;
import org.bukkit.plugin.java.JavaPlugin;
import org.yaml.snakeyaml.LoaderOptions;
import org.yaml.snakeyaml.Yaml;
import org.yaml.snakeyaml.constructor.SafeConstructor;

public final class PaperStorageExporter extends JavaPlugin {
    private static final String ITEM_CLASS = "org.bukkit.inventory.ItemStack";

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (!(sender instanceof ConsoleCommandSender)) {
            sender.sendMessage("Use the local Paper server console to export a storage snapshot.");
            return true;
        }
        if (args.length != 2) {
            return false;
        }
        try {
            Path output = exportSnapshot(Path.of(args[0]), Path.of(args[1]));
            sender.sendMessage("Exported the Slimefun snapshot to " + output);
        } catch (IOException | RuntimeException failure) {
            sender.sendMessage("Could not export the Slimefun snapshot. " + failure.getMessage());
        }
        return true;
    }

    static Path exportSnapshot(Path sourcePath, Path outputPath) throws IOException {
        Path source = sourcePath.toRealPath();
        Path absolute = outputPath.toAbsolutePath().normalize();
        if (absolute.getParent() == null) {
            throw new IOException("Choose a new output directory below an existing parent.");
        }
        Path parent = absolute.getParent().toRealPath();
        Path output = parent.resolve(absolute.getFileName());
        if (output.startsWith(source) || source.startsWith(output)) {
            throw new IOException("Choose a separate directory for the exported snapshot.");
        }
        if (Files.exists(output, LinkOption.NOFOLLOW_LINKS)) {
            throw new IOException("Output already exists: " + output);
        }
        if (!Files.isDirectory(source.resolve("stored-inventories"))
                && !Files.isDirectory(source.resolve("universal-inventories"))
                && !Files.isDirectory(source.resolve("Players"))) {
            throw new IOException("The snapshot must include a Slimefun inventory or Players directory.");
        }
        Path stage = Files.createTempDirectory(parent, ".paper-storage-export-");
        try {
            copyDirectory(source, stage, "stored-blocks");
            copyDirectory(source, stage, "stored-inventories");
            copyDirectory(source, stage, "universal-inventories");
            copyDirectory(source, stage, "Players");
            return Files.move(stage, output);
        } finally {
            if (Files.exists(stage)) {
                try (var paths = Files.walk(stage)) {
                    for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) {
                        Files.delete(path);
                    }
                }
            }
        }
    }

    private static void copyDirectory(Path root, Path stage, String directory)
            throws IOException {
        Path source = root.resolve(directory);
        if (!Files.exists(source, LinkOption.NOFOLLOW_LINKS)) {
            return;
        }
        try (var paths = Files.walk(source)) {
            for (Path from : paths.sorted().toList()) {
                Path relative = root.relativize(from);
                Path to = stage.resolve(relative);
                if (Files.isDirectory(from, LinkOption.NOFOLLOW_LINKS)) {
                    Files.createDirectory(to);
                } else if (!Files.isRegularFile(from, LinkOption.NOFOLLOW_LINKS)) {
                    throw new IOException("Unsupported snapshot path: " + relative);
                } else {
                    String name = from.getFileName().toString();
                    try {
                        if (directory.endsWith("-inventories") && name.endsWith(".sfi")) {
                            Files.writeString(to, normalize(Files.readString(from)));
                        } else if (directory.equals("Players") && name.matches(
                                "[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}){3}-[0-9a-fA-F]{12}\\.yml")) {
                            Files.writeString(to, normalizePlayer(Files.readString(from)));
                        } else {
                            Files.copy(from, to);
                        }
                    } catch (Exception failure) {
                        throw new IOException(relative + " could not be exported. "
                                + failure.getMessage(), failure);
                    }
                }
            }
        }
    }

    static String normalize(String input) throws Exception {
        Map<?, ?> document = readDocument(input);
        if (!(document.get("preset") instanceof String preset) || preset.isBlank()) {
            throw new IOException("The inventory has no Slimefun preset.");
        }
        Map<?, ?> normalized = normalizeSlots(document, 54, "", true);
        return normalized == document ? input : new Yaml().dump(normalized);
    }

    static String normalizePlayer(String input) throws Exception {
        Map<?, ?> document = readDocument(input);
        Object raw = document.get("backpacks");
        if (raw == null) {
            return input;
        }
        if (!(raw instanceof Map<?, ?> backpacks)) {
            throw new IOException("The player's backpacks must be a mapping.");
        }
        Map<Object, Object> updated = new LinkedHashMap<>(backpacks);
        boolean changed = false;
        Set<String> seen = new HashSet<>();
        for (var entry : backpacks.entrySet()) {
            String id = String.valueOf(entry.getKey());
            if (!id.matches("0|[1-9][0-9]*") || !seen.add(id)
                    || !(entry.getValue() instanceof Map<?, ?> data)
                    || !(data.get("size") instanceof Integer size)
                    || size < 9 || size > 54 || size % 9 != 0) {
                throw new IOException("Invalid backpack record " + id);
            }
            Object contents = data.get("contents");
            if (contents == null) {
                continue;
            }
            if (!(contents instanceof Map<?, ?> inventory)) {
                throw new IOException("Invalid contents for backpack " + id);
            }
            Map<?, ?> normalized = normalizeSlots(inventory, size, "backpacks." + id + ".contents.", false);
            if (normalized != inventory) {
                Map<Object, Object> pack = new LinkedHashMap<>(data);
                pack.put("contents", normalized);
                updated.put(entry.getKey(), pack);
                changed = true;
            }
        }
        if (!changed) {
            return input;
        }
        Map<Object, Object> profile = new LinkedHashMap<>(document);
        profile.put("backpacks", updated);
        return new Yaml().dump(profile);
    }

    private static Map<?, ?> readDocument(String input) throws IOException {
        LoaderOptions options = new LoaderOptions();
        options.setAllowDuplicateKeys(false);
        options.setCodePointLimit(64 << 20);
        options.setNestingDepthLimit(100);
        Object parsed = new Yaml(new SafeConstructor(options)).load(input);
        if (parsed == null) {
            return Map.of();
        }
        if (!(parsed instanceof Map<?, ?> data)) {
            throw new IOException("The storage file must be a YAML mapping.");
        }
        return data;
    }

    private static Map<?, ?> normalizeSlots(Map<?, ?> data, int size, String prefix, boolean isMenu)
            throws Exception {
        Map<Object, Object> normalized = new LinkedHashMap<>(data);
        boolean changed = false;
        Set<String> seen = new HashSet<>();
        for (var entry : data.entrySet()) {
            String key = String.valueOf(entry.getKey());
            if (isMenu && key.equals("preset")) {
                continue;
            }
            if (!key.matches("0|[1-9]|[1-4][0-9]|5[0-3]")
                    || Integer.parseInt(key) >= size || !seen.add(key)) {
                throw new IOException("Invalid or repeated Paper item slot " + key);
            }
            Object value = entry.getValue();
            if (value == null) {
                continue;
            }
            if (!(value instanceof Map<?, ?> item) || !ITEM_CLASS.equals(item.get("=="))) {
                throw new IOException("Slot " + key + " is not a serialized Bukkit ItemStack.");
            }
            if (Integer.valueOf(1).equals(item.get("schema_version"))) {
                continue;
            }
            try {
                normalized.put(entry.getKey(), PaperLegacyItem.normalize(item));
                changed = true;
            } catch (Exception failure) {
                throw new IOException("Could not export item slot " + prefix + key + ". "
                        + failure.getMessage(), failure);
            }
        }
        return changed ? normalized : data;
    }
}
