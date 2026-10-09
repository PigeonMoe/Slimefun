package io.github.thebusybiscuit.slimefun4.core.services.sync;

import java.io.*;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.*;

/** Bounded, checksummed external profile; world-linked GPS data is deliberately absent. */
final class ProfileSnapshot {
    static final int MAX_BYTES = 8 * 1024 * 1024;
    static final int MAX_BACKPACKS = 4096;
    static final int MAX_RESEARCHES = 16384;
    static final int MAX_ITEM_BYTES = 1024 * 1024;
    private static final int MAGIC = 0x53465031; // SFP1
    record Bag(int id, List<byte[]> slots) {}

    final UUID owner;
    final boolean guideIssued;
    final Set<String> researches;
    final Set<Integer> legacyResearches;
    final List<Bag> bags;

    ProfileSnapshot(UUID owner, boolean guideIssued, Set<String> researches, Set<Integer> legacyResearches, List<Bag> bags) {
        this.owner = owner;
        this.guideIssued = guideIssued;
        this.researches = Set.copyOf(researches);
        this.legacyResearches = Set.copyOf(legacyResearches);
        this.bags = List.copyOf(bags);
    }

    byte[] encode() {
        try {
            ByteArrayOutputStream bytes = new ByteArrayOutputStream();
            DataOutputStream out = new DataOutputStream(bytes);
            out.writeInt(MAGIC);
            out.writeLong(owner.getMostSignificantBits());
            out.writeLong(owner.getLeastSignificantBits());
            out.writeBoolean(guideIssued);
            out.writeInt(researches.size());
            for (String key : new TreeSet<>(researches)) out.writeUTF(key);
            out.writeInt(legacyResearches.size());
            for (int id : new TreeSet<>(legacyResearches)) out.writeInt(id);
            out.writeInt(bags.size());
            for (Bag bag : bags.stream().sorted(Comparator.comparingInt(Bag::id)).toList()) {
                out.writeInt(bag.id());
                out.writeInt(bag.slots().size());
                for (byte[] item : bag.slots()) {
                    out.writeInt(item.length);
                    out.write(item);
                    if (bytes.size() > MAX_BYTES - 32) throw new IllegalArgumentException("Slimefun profile exceeds 8 MiB");
                }
            }
            out.flush();
            byte[] payload = bytes.toByteArray();
            bytes.write(digest(payload));
            byte[] result = bytes.toByteArray();
            decode(owner, result); // The writer must obey the same bounds as the reader.
            return result;
        } catch (IOException e) {
            throw new IllegalArgumentException("Cannot encode Slimefun profile", e);
        }
    }

    static ProfileSnapshot decode(UUID expectedOwner, byte[] bytes) {
        if (bytes == null || bytes.length < 64 || bytes.length > MAX_BYTES) throw new IllegalArgumentException("Missing/oversize Slimefun profile");
        byte[] payload = Arrays.copyOf(bytes, bytes.length - 32);
        if (!MessageDigest.isEqual(digest(payload), Arrays.copyOfRange(bytes, bytes.length - 32, bytes.length))) {
            throw new IllegalArgumentException("Slimefun profile checksum mismatch");
        }
        try (DataInputStream in = new DataInputStream(new ByteArrayInputStream(payload))) {
            if (in.readInt() != MAGIC) throw new IllegalArgumentException("Unknown Slimefun profile schema");
            UUID owner = new UUID(in.readLong(), in.readLong());
            if (!expectedOwner.equals(owner)) throw new IllegalArgumentException("Slimefun profile owner mismatch");
            int guideFlag = in.readUnsignedByte();
            if (guideFlag > 1) throw new IllegalArgumentException("Invalid first-guide flag");
            Set<String> keys = new HashSet<>();
            int count = count(in, MAX_RESEARCHES);
            for (int i = 0; i < count; i++) {
                String key = in.readUTF();
                if (key.length() > 512 || !key.matches("[a-z0-9._-]+:[a-z0-9/._-]+") || !keys.add(key)) {
                    throw new IllegalArgumentException("Invalid/duplicate research key");
                }
            }
            Set<Integer> legacy = new HashSet<>();
            count = count(in, MAX_RESEARCHES);
            for (int i = 0; i < count; i++) {
                if (!legacy.add(in.readInt())) throw new IllegalArgumentException("Duplicate legacy research");
            }
            List<Bag> bags = new ArrayList<>();
            Set<Integer> ids = new HashSet<>();
            count = count(in, MAX_BACKPACKS);
            for (int i = 0; i < count; i++) {
                int id = in.readInt();
                if (id < 0 || !ids.add(id)) throw new IllegalArgumentException("Invalid/duplicate backpack ID");
                int size = in.readInt();
                if (size < 9 || size > 54 || size % 9 != 0) throw new IllegalArgumentException("Invalid backpack size");
                List<byte[]> slots = new ArrayList<>(size);
                for (int slot = 0; slot < size; slot++) {
                    int length = count(in, MAX_ITEM_BYTES);
                    if (length > in.available()) throw new EOFException("Truncated backpack item");
                    slots.add(in.readNBytes(length));
                }
                bags.add(new Bag(id, List.copyOf(slots)));
            }
            if (in.available() != 0) throw new IllegalArgumentException("Trailing Slimefun profile data");
            return new ProfileSnapshot(owner, guideFlag == 1, keys, legacy, bags);
        } catch (IOException e) {
            throw new IllegalArgumentException("Malformed Slimefun profile", e);
        }
    }

    private static int count(DataInputStream in, int maximum) throws IOException {
        int value = in.readInt();
        if (value < 0 || value > maximum) throw new IllegalArgumentException("Invalid Slimefun profile count/length");
        return value;
    }

    private static byte[] digest(byte[] payload) {
        try {
            return MessageDigest.getInstance("SHA-256").digest(payload);
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException(e);
        }
    }
}
