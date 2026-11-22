// SPDX-License-Identifier: GPL-3.0-only
package io.github.thebusybiscuit.slimefun4.core.services.sync;

import java.io.BufferedInputStream;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.DataInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.Arrays;
import java.util.Comparator;
import java.util.HashSet;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import org.bukkit.inventory.ItemStack;
import org.yaml.snakeyaml.Yaml;

public final class PaperSyncedProfiles {
    // PM-Sync SnapshotEnvelope v2. The inner SFP1 schema is shared with PlayerSyncService.
    private static final int PMS_V2 = 0x504d5302;
    private static final int MAX_SNAPSHOT = 64 << 20;

    private PaperSyncedProfiles() {}

    public static void exportRows(Path source, Path output) throws IOException {
        Path players = Files.createDirectory(output.resolve("Players"));
        Path profiles = Files.createDirectory(output.resolve("SFP1"));
        Set<UUID> owners = new HashSet<>();
        try (InputStream input = new BufferedInputStream(Files.newInputStream(source))) {
            byte[] line;
            int row = 0;
            while ((line = readRow(input)) != null) {
                row++;
                try {
                    String text = new String(line, StandardCharsets.US_ASCII);
                    int first = text.indexOf('\t');
                    int second = first < 0 ? -1 : text.indexOf('\t', first + 1);
                    if (first != 36 || second < 0) {
                        throw new IOException("Expected UUID, revision and hex-encoded snapshot.");
                    }
                    String ownerText = text.substring(0, first);
                    UUID owner = UUID.fromString(ownerText);
                    if (!owner.toString().equals(ownerText)) {
                        throw new IOException("Expected a canonical player UUID.");
                    }
                    if (!owners.add(owner)) {
                        throw new IOException("The snapshot has a repeated player UUID.");
                    }
                    String revisionText = text.substring(first + 1, second);
                    long revision = Long.parseLong(revisionText);
                    if (revision < 1 || !Long.toString(revision).equals(revisionText)) {
                        throw new IOException("Expected a positive PM-Sync revision.");
                    }
                    if (text.length() - (second + 1) > MAX_SNAPSHOT * 2) {
                        throw new IOException("The PM-Sync snapshot exceeds 64 MiB.");
                    }
                    byte[] blob = HexFormat.of().parseHex(text, second + 1, text.length());
                    byte[] section = slimefunSection(blob);
                    ProfileSnapshot profile = ProfileSnapshot.decode(owner, section);
                    Files.writeString(players.resolve(owner + ".yml"), new Yaml().dump(profileMap(profile, revision)),
                            StandardOpenOption.CREATE_NEW);
                    Files.write(profiles.resolve(ownerText.replace("-", "") + ".sfp1"), section,
                            StandardOpenOption.CREATE_NEW);
                } catch (IOException | RuntimeException error) {
                    throw new IOException("PM-Sync row " + row + ": " + error.getMessage(), error);
                }
            }
        }
        if (owners.isEmpty()) {
            throw new IOException("The PM-Sync export has no committed snapshots.");
        }
    }

    private static byte[] readRow(InputStream input) throws IOException {
        var line = new ByteArrayOutputStream();
        int character;
        while ((character = input.read()) >= 0 && character != '\n') {
            if (line.size() == MAX_SNAPSHOT * 2 + 64) {
                throw new IOException("A PM-Sync export row exceeds the bounded snapshot size.");
            }
            line.write(character);
        }
        if (line.size() == 0 && character < 0) {
            return null;
        }
        byte[] contents = line.toByteArray();
        return contents.length > 0 && contents[contents.length - 1] == '\r'
                ? Arrays.copyOf(contents, contents.length - 1) : contents;
    }

    private static byte[] slimefunSection(byte[] bytes) throws IOException {
        if (bytes.length < 32 || bytes.length > MAX_SNAPSHOT) {
            throw new IOException("The PM-Sync snapshot size is invalid.");
        }
        int length = bytes.length - 32;
        MessageDigest sha;
        try {
            sha = MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException error) {
            throw new AssertionError(error);
        }
        sha.update(bytes, 0, length);
        if (!MessageDigest.isEqual(sha.digest(), Arrays.copyOfRange(bytes, length, bytes.length))) {
            throw new IOException("PM-Sync snapshot checksum mismatch.");
        }
        try (var input = new DataInputStream(new ByteArrayInputStream(bytes, 0, length))) {
            if (input.readInt() != PMS_V2) {
                throw new IOException("Unknown PM-Sync snapshot version.");
            }
            double health = input.readDouble();
            if (!Double.isFinite(health) || health < 0) {
                throw new IOException("Invalid PM-Sync snapshot health.");
            }
            int recipes = count(input, 100_000);
            for (int recipe = 0; recipe < recipes; recipe++) {
                input.readUTF();
            }
            int sections = count(input, 32);
            Set<String> names = new HashSet<>();
            byte[] slimefun = null;
            for (int index = 0; index < sections; index++) {
                String name = input.readUTF();
                int bytesInSection = count(input, input.available());
                if (!name.matches("[a-z0-9_-]{1,64}") || bytesInSection == 0 || !names.add(name)) {
                    throw new IOException("Invalid or repeated PM-Sync profile section.");
                }
                if (name.equals("slimefun")) {
                    if (bytesInSection > ProfileSnapshot.MAX_BYTES) {
                        throw new IOException("The Slimefun profile exceeds 8 MiB.");
                    }
                    slimefun = input.readNBytes(bytesInSection);
                } else {
                    input.skipNBytes(bytesInSection);
                }
            }
            int nativeProfile = input.readInt();
            if (nativeProfile <= 0 || nativeProfile != input.available()) {
                throw new IOException("Invalid PM-Sync native profile length.");
            }
            if (slimefun == null) {
                throw new IOException("The PM-Sync snapshot has no Slimefun profile.");
            }
            return slimefun;
        }
    }

    private static int count(DataInputStream input, int maximum) throws IOException {
        int count = input.readInt();
        if (count < 0 || count > maximum) {
            throw new IOException("Invalid PM-Sync snapshot count or length.");
        }
        return count;
    }

    private static Map<String, Object> profileMap(ProfileSnapshot profile, long revision) throws IOException {
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("pm-snapshot-revision", revision);
        record.put("pm-guide-issued", profile.guideIssued);
        record.put("pm-research-keys", profile.researches.stream().sorted().toList());
        Map<String, Boolean> oldResearch = new LinkedHashMap<>();
        profile.legacyResearches.stream().sorted().forEach(id -> oldResearch.put(Integer.toString(id), true));
        record.put("researches", oldResearch);

        Map<String, Object> backpacks = new LinkedHashMap<>();
        for (ProfileSnapshot.Bag bag : profile.bags.stream().sorted(Comparator.comparingInt(ProfileSnapshot.Bag::id)).toList()) {
            Map<String, Object> contents = new LinkedHashMap<>();
            for (int slot = 0; slot < bag.slots().size(); slot++) {
                byte[] encoded = bag.slots().get(slot);
                if (encoded.length == 0) {
                    continue;
                }
                try {
                    // Check decompressed size before Paper performs DataFixer conversion.
                    NbtIo.readCompressed(new ByteArrayInputStream(encoded), NbtAccounter.create(16 << 20));
                    ItemStack item = ItemStack.deserializeBytes(encoded);
                    if (item == null || item.getType().isAir() || item.getAmount() <= 0) {
                        throw new IOException("A nonempty Paper slot decoded as empty.");
                    }
                    Map<String, Object> stack = new LinkedHashMap<>(item.serialize());
                    if (!Integer.valueOf(1).equals(stack.get("schema_version"))) {
                        throw new IOException("Paper did not serialize this item with components.");
                    }
                    stack.put("==", "org.bukkit.inventory.ItemStack");
                    contents.put(Integer.toString(slot), stack);
                } catch (IOException | RuntimeException error) {
                    throw new IOException("Backpack " + bag.id() + " slot " + slot + " failed Paper decoding.", error);
                }
            }
            Map<String, Object> entry = new LinkedHashMap<>();
            entry.put("size", bag.slots().size());
            entry.put("contents", contents);
            backpacks.put(Integer.toString(bag.id()), entry);
        }
        record.put("backpacks", backpacks);
        return record;
    }
}
