// SPDX-License-Identifier: GPL-3.0-only
package io.github.thebusybiscuit.slimefun4.core.services.sync;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.List;
import java.util.Set;
import java.util.UUID;
import org.bukkit.Material;
import org.bukkit.NamespacedKey;
import org.bukkit.inventory.ItemStack;
import org.bukkit.persistence.PersistentDataType;

public final class PaperSyncedProfilesFixture {
    public static final UUID OWNER = UUID.fromString("8c0e7dd0-005e-48a5-9d49-28afb237bc91");

    private PaperSyncedProfilesFixture() {}

    public static byte[] itemBytes() {
        ItemStack item = new ItemStack(Material.DIAMOND, 3);
        NamespacedKey marker = new NamespacedKey("migration_fixture", "original");
        item.editMeta(meta -> meta.getPersistentDataContainer().set(marker, PersistentDataType.STRING, "saved"));
        return item.serializeAsBytes();
    }

    public static byte[] section(UUID owner, byte[] paperItem) {
        var slots = new byte[9][];
        Arrays.setAll(slots, index -> index == 7 ? paperItem : new byte[0]);
        var bag = new ProfileSnapshot.Bag(14, List.of(slots));
        var profile = new ProfileSnapshot(owner, true, Set.of("slimefun:lumps", "addon:research"),
                Set.of(31), List.of(bag));
        return profile.encode();
    }

    public static byte[] envelope(byte[] sfp) throws Exception {
        var bytes = new ByteArrayOutputStream();
        var out = new DataOutputStream(bytes);
        out.writeInt(0x504d5302);
        out.writeDouble(15.5);
        out.writeInt(1);
        out.writeUTF("minecraft:stick");
        out.writeInt(2);
        out.writeUTF("other-profile");
        out.writeInt(2);
        out.writeByte(0);
        out.writeByte(5);
        out.writeUTF("slimefun");
        out.writeInt(sfp.length);
        out.write(sfp);
        // A non-Slimefun HuskSync payload is intentionally opaque to the exporter.
        out.writeInt(2);
        out.writeByte(12);
        out.writeByte(34);
        out.flush();
        bytes.write(MessageDigest.getInstance("SHA-256").digest(bytes.toByteArray()));
        return bytes.toByteArray();
    }
}
