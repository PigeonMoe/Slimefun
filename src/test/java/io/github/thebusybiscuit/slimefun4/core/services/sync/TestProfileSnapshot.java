package io.github.thebusybiscuit.slimefun4.core.services.sync;

import java.util.*;
import java.security.MessageDigest;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

class TestProfileSnapshot {
    private final UUID owner = UUID.randomUUID();
    private ProfileSnapshot empty() { return new ProfileSnapshot(owner, false, Set.of("addon:missing_research"), Set.of(123456), List.of()); }

    @Test void roundTripPreservesUnloadedAddonResearchAndFirstGuideState() {
        var decoded = ProfileSnapshot.decode(owner, empty().encode());
        assertEquals(empty().researches, decoded.researches);
        assertEquals(Set.of(123456), decoded.legacyResearches);
        assertFalse(decoded.guideIssued);
    }

    @Test void includesEverySlotAndSparseBackpackIdentity() {
        List<byte[]> slots = new ArrayList<>();
        for (int i = 0; i < 54; i++) slots.add(i == 53 ? new byte[]{1, 2, 3, 4} : new byte[0]);
        var snapshot = new ProfileSnapshot(owner, true, Set.of(), Set.of(), List.of(new ProfileSnapshot.Bag(42, slots)));
        var decoded = ProfileSnapshot.decode(owner, snapshot.encode());
        assertEquals(42, decoded.bags.getFirst().id());
        assertEquals(54, decoded.bags.getFirst().slots().size());
        assertArrayEquals(slots.get(53), decoded.bags.getFirst().slots().get(53));
        assertTrue(decoded.guideIssued);
    }

    @Test void rejectsCorruptionTruncationNullOversizeAndWrongOwner() {
        byte[] data = empty().encode();
        byte[] corrupt = data.clone(); corrupt[24] ^= 1;
        assertThrows(IllegalArgumentException.class, () -> ProfileSnapshot.decode(owner, corrupt));
        assertThrows(IllegalArgumentException.class, () -> ProfileSnapshot.decode(owner, Arrays.copyOf(data, data.length - 1)));
        assertThrows(IllegalArgumentException.class, () -> ProfileSnapshot.decode(owner, null));
        assertThrows(IllegalArgumentException.class, () -> ProfileSnapshot.decode(owner, new byte[ProfileSnapshot.MAX_BYTES + 1]));
        assertThrows(IllegalArgumentException.class, () -> ProfileSnapshot.decode(UUID.randomUUID(), data));
    }

    @Test void rejectsUnknownSchemaNegativeCountsAndTrailingBytesEvenWithValidDigest() throws Exception {
        byte[] bytes = empty().encode();
        byte[] schema = bytes.clone(); schema[0] = 0;
        assertThrows(IllegalArgumentException.class, () -> ProfileSnapshot.decode(owner, rehash(schema)));
        byte[] negativeCount = bytes.clone(); Arrays.fill(negativeCount, 21, 25, (byte) 0xff);
        assertThrows(IllegalArgumentException.class, () -> ProfileSnapshot.decode(owner, rehash(negativeCount)));
        byte[] trailing = new byte[bytes.length + 1];
        System.arraycopy(bytes, 0, trailing, 0, bytes.length - 32);
        assertThrows(IllegalArgumentException.class, () -> ProfileSnapshot.decode(owner, rehash(trailing)));
    }

    @Test void writerRejectsInvalidOrDuplicateBackpacksAndOversizeItems() {
        List<byte[]> slots = Collections.nCopies(9, new byte[0]);
        assertThrows(IllegalArgumentException.class, () -> new ProfileSnapshot(owner, false, Set.of(), Set.of(), List.of(new ProfileSnapshot.Bag(-1, slots))).encode());
        var bag = new ProfileSnapshot.Bag(1, slots);
        assertThrows(IllegalArgumentException.class, () -> new ProfileSnapshot(owner, false, Set.of(), Set.of(), List.of(bag, bag)).encode());
        assertThrows(IllegalArgumentException.class, () -> new ProfileSnapshot(owner, false, Set.of(), Set.of(), List.of(new ProfileSnapshot.Bag(0, List.of()))).encode());
        var items = new ArrayList<>(slots); items.set(0, new byte[ProfileSnapshot.MAX_ITEM_BYTES + 1]);
        assertThrows(IllegalArgumentException.class, () -> new ProfileSnapshot(owner, false, Set.of(), Set.of(), List.of(new ProfileSnapshot.Bag(0, items))).encode());
    }

    @Test void writerRejectsInvalidResearchKeys() {
        assertThrows(IllegalArgumentException.class, () -> new ProfileSnapshot(owner, false, Set.of("NOT A KEY"), Set.of(), List.of()).encode());
    }

    private byte[] rehash(byte[] data) throws Exception {
        byte[] hash = MessageDigest.getInstance("SHA-256").digest(Arrays.copyOf(data, data.length - 32));
        System.arraycopy(hash, 0, data, data.length - 32, 32);
        return data;
    }
}
