package io.github.thebusybiscuit.slimefun4.core.services.sync;

import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;
import org.bukkit.*;
import org.bukkit.configuration.file.YamlConfiguration;
import org.bukkit.inventory.ItemStack;
import org.bukkit.persistence.PersistentDataType;
import org.junit.jupiter.api.*;
import org.junit.jupiter.api.io.TempDir;
import org.mockbukkit.mockbukkit.*;
import org.mockbukkit.mockbukkit.entity.PlayerMock;
import io.github.thebusybiscuit.slimefun4.api.gps.Waypoint;
import io.github.thebusybiscuit.slimefun4.api.player.PlayerProfile;
import io.github.thebusybiscuit.slimefun4.api.researches.Research;
import io.github.thebusybiscuit.slimefun4.implementation.Slimefun;
import io.github.thebusybiscuit.slimefun4.implementation.listeners.BackpackListener;
import moe.pigeon.slimefun.api.PlayerSyncBridge;
import static org.junit.jupiter.api.Assertions.*;

class TestPlayerSyncService {
    @TempDir Path root;
    ServerMock server;
    Slimefun plugin;
    PlayerMock player;
    PlayerSyncService sync;

    @BeforeEach void setup() throws Exception {
        server = MockBukkit.mock();
        plugin = MockBukkit.load(Slimefun.class);
        player = server.addPlayer();
        sync = new PlayerSyncService(plugin, root, "survival");
        var field = Slimefun.class.getDeclaredField("playerSync"); field.setAccessible(true); field.set(plugin, sync);
        sync.register();
    }
    @AfterEach void cleanup() { MockBukkit.unmock(); }
    byte[] empty(boolean guide) { return new ProfileSnapshot(player.getUniqueId(), guide, Set.of(), Set.of(), List.of()).encode(); }
    void ready() throws Exception {
        sync.beginLoad(player);
        assertTrue(await(sync.apply(player, empty(true))));
        sync.onSyncLoaded(player);
    }
    <T> T await(CompletableFuture<T> future) throws Exception {
        long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(10);
        while (!future.isDone() && System.nanoTime() < deadline) { server.getScheduler().performOneTick(); Thread.sleep(5); }
        return future.get(1, TimeUnit.SECONDS);
    }

    @Test void registersExactCrossLoaderContractAndFailsClosedWithoutCoordinator() {
        assertSame(sync, server.getServicesManager().load(PlayerSyncBridge.class));
        assertEquals(1, sync.apiVersion());
        assertTrue(sync.blocked(player));
        assertFalse(PlayerProfile.get(player, p -> fail("No legacy async load is allowed")));
        assertFalse(PlayerProfile.request(player));
        assertTrue(PlayerProfile.find(player).isEmpty());
        assertThrows(IllegalArgumentException.class, () -> new PlayerSyncService(plugin, root, ""));
    }

    @Test void targetStaysFrozenUntilExplicitCompleteAndNeverRewritesLegacyFile() throws Exception {
        Path legacy = root.resolve("Players/" + player.getUniqueId() + ".yml");
        Files.createDirectories(legacy.getParent()); Files.writeString(legacy, "bad: [legacy is never read on target");
        sync.beginLoad(player);
        assertTrue(await(sync.apply(player, empty(true))));
        assertTrue(sync.blocked(player));
        assertTrue(PlayerProfile.find(player).isEmpty());
        assertThrows(IllegalStateException.class, () -> sync.capture(player));
        sync.onSyncLoaded(player);
        var profile = PlayerProfile.find(player).orElseThrow();
        profile.markDirty(); profile.save();
        assertTrue(profile.isDirty());
        assertTrue(Files.readString(legacy).startsWith("bad:"));
        assertThrows(UnsupportedOperationException.class, profile::getConfig);
    }

    @Test void failedTargetCannotUnlockOrCaptureAnEmptyProfile() {
        sync.beginLoad(player);
        assertThrows(CompletionException.class, () -> sync.apply(player, new byte[]{1}).join());
        assertTrue(sync.blocked(player));
        assertThrows(IllegalStateException.class, () -> sync.onSyncLoaded(player));
        assertThrows(IllegalStateException.class, () -> sync.abortTransfer(player));
        assertTrue(PlayerProfile.find(player).isEmpty());
    }

    @Test void completeBackpacksWithItemPdcAndUnknownResearchRoundTrip() throws Exception {
        byte[] initial = new ProfileSnapshot(player.getUniqueId(), true, Set.of("addon:unloaded"), Set.of(99999), List.of()).encode();
        sync.beginLoad(player); assertTrue(await(sync.apply(player, initial))); sync.onSyncLoaded(player);
        var profile = PlayerProfile.find(player).orElseThrow();
        var first = profile.createBackpack(9);
        var second = profile.createBackpack(54);
        profile.getPlayerData().removeBackpack(first);
        var third = profile.createBackpack(9);
        assertEquals(2, third.getId()); // Removing one bag must never collide with another existing ID.
        ItemStack item = new ItemStack(Material.DIAMOND_SWORD);
        var meta = item.getItemMeta(); meta.setDisplayName("同步剑");
        meta.getPersistentDataContainer().set(new NamespacedKey("addon", "payload"), PersistentDataType.STRING, "不丢失");
        item.setItemMeta(meta); second.getInventory().setItem(53, item);
        assertTrue(sync.prepareTransfer(player));
        byte[] captured = sync.capture(player);
        var decoded = ProfileSnapshot.decode(player.getUniqueId(), captured);
        assertEquals(Set.of("addon:unloaded"), decoded.researches);
        assertEquals(Set.of(99999), decoded.legacyResearches);
        sync.sourceCommitted(player);
        assertTrue(sync.blocked(player));
        sync.beginLoad(player); assertTrue(await(sync.apply(player, captured))); sync.onSyncLoaded(player);
        var restored = PlayerProfile.find(player).orElseThrow().getBackpack(1).orElseThrow().getInventory().getItem(53);
        assertEquals(item, restored);
        assertEquals("不丢失", restored.getItemMeta().getPersistentDataContainer().get(new NamespacedKey("addon", "payload"), PersistentDataType.STRING));
        assertEquals(2, PlayerProfile.find(player).orElseThrow().getPlayerData().getBackpacks().size());
    }

    @Test void legacyBootstrapMigratesDuplicateResearchIdAndAllBagSlotsWithoutChangingOriginal() throws Exception {
        var one = new Research(new NamespacedKey("addon", "one"), 173, "One", 1);
        var two = new Research(new NamespacedKey("addon", "two"), 173, "Two", 1);
        Slimefun.getRegistry().getResearches().addAll(List.of(one, two));
        Path file = root.resolve("Players/" + player.getUniqueId() + ".yml"); Files.createDirectories(file.getParent());
        YamlConfiguration yaml = new YamlConfiguration();
        yaml.set("researches.173", false); yaml.set("researches.98765", true);
        yaml.set("backpacks.17.size", 9); yaml.set("backpacks.17.contents.8", new ItemStack(Material.DIAMOND, 12));
        yaml.save(file.toFile()); String before = Files.readString(file);
        sync.beginLoad(player); byte[] migrated = await(sync.bootstrap(player));
        assertTrue(sync.blocked(player)); assertEquals(before, Files.readString(file));
        var decoded = ProfileSnapshot.decode(player.getUniqueId(), migrated);
        assertEquals(Set.of("addon:one", "addon:two"), decoded.researches);
        assertEquals(Set.of(98765), decoded.legacyResearches); assertTrue(decoded.guideIssued);
        assertTrue(await(sync.apply(player, migrated))); sync.onSyncLoaded(player);
        var profile = PlayerProfile.find(player).orElseThrow();
        assertEquals(Set.of(one, two), profile.getResearches());
        assertEquals(12, profile.getBackpack(17).orElseThrow().getInventory().getItem(8).getAmount());
    }

    @Test void invalidLegacyYamlOrBackpackDoesNotBecomeEmptyMigration() throws Exception {
        Path file = root.resolve("Players/" + player.getUniqueId() + ".yml"); Files.createDirectories(file.getParent());
        Files.writeString(file, "backpacks:\n  '0':\n    size: 99\n");
        sync.beginLoad(player);
        assertThrows(ExecutionException.class, () -> await(sync.bootstrap(player)));
        assertTrue(sync.blocked(player)); assertTrue(PlayerProfile.find(player).isEmpty());
        Files.writeString(file, "bad: [broken");
        // A failed bootstrap cannot be retried inside the same SQL session.
        assertThrows(IllegalStateException.class, () -> sync.bootstrap(player));
    }

    @Test void inFlightOldLoadCannotReplaceNewSessionOrWriteOldProfile() throws Exception {
        ready(); var old = PlayerProfile.find(player).orElseThrow();
        var next = new PlayerMock(server, "new-session", player.getUniqueId());
        sync.beginLoad(next);
        assertTrue(sync.blocked(player)); assertFalse(old.isAccessible());
        assertFalse(PlayerProfile.get(player, p -> fail("Old Player identity must not access a new profile")));
        assertThrows(IllegalStateException.class, old::markDirty);
        assertTrue(await(sync.apply(next, empty(true)))); sync.onSyncLoaded(next);
        assertThrows(IllegalStateException.class, () -> sync.capture(player));
        assertThrows(IllegalStateException.class, () -> sync.sourceCommitted(player));
    }

    @Test void obsoleteAsyncCompletionDoesNotInstallProfile() throws Exception {
        sync.beginLoad(player); var pending = sync.apply(player, empty(true));
        var next = new PlayerMock(server, "new-session", player.getUniqueId()); sync.beginLoad(next);
        assertThrows(ExecutionException.class, () -> await(pending));
        assertTrue(Slimefun.getRegistry().getPlayerProfiles().isEmpty()); assertTrue(sync.blocked(next));
        assertTrue(await(sync.apply(next, empty(true)))); sync.onSyncLoaded(next);
        assertFalse(sync.blocked(next));
    }

    @Test void researchDrainPrecedesFreezeAndCheckpointAbortRestoresOnlyPreparedSession() throws Exception {
        ready(); var profile = PlayerProfile.find(player).orElseThrow();
        Slimefun.getRegistry().getCurrentlyResearchingPlayers().add(player.getUniqueId());
        assertFalse(sync.prepareTransfer(player)); assertFalse(sync.blocked(player));
        Slimefun.getRegistry().getCurrentlyResearchingPlayers().remove(player.getUniqueId());
        assertTrue(sync.prepareTransfer(player)); assertTrue(sync.blocked(player));
        assertThrows(IllegalStateException.class, profile::markDirty);
        sync.abortTransfer(player); assertFalse(sync.blocked(player)); profile.markDirty();
        assertThrows(IllegalStateException.class, () -> sync.abortTransfer(player));
    }

    @Test void gpsIsServerScopedAndUnmountedWorldRecordsSurvive() throws Exception {
        World world = server.addSimpleWorld("survival-world");
        Path file = root.resolve("waypoints/" + player.getUniqueId() + ".yml"); Files.createDirectories(file.getParent());
        YamlConfiguration yaml = new YamlConfiguration();
        yaml.set("unmounted.world", "archived-world"); yaml.set("unmounted.name", "归档点"); yaml.save(file.toFile());
        ready(); var profile = PlayerProfile.find(player).orElseThrow();
        profile.addWaypoint(new Waypoint(player.getUniqueId(), "home", new Location(world, 1, 2, 3), "家"));
        assertTrue(sync.prepareTransfer(player)); byte[] captured = sync.capture(player);
        Path saved = root.resolve("player-sync/survival/waypoints/" + player.getUniqueId() + ".yml");
        var stored = YamlConfiguration.loadConfiguration(saved.toFile());
        assertEquals("archived-world", stored.getString("unmounted.world"));
        assertEquals("survival-world", stored.getString("home.world"), stored.saveToString());
        sync.sourceCommitted(player);
        PlayerSyncService hub = new PlayerSyncService(plugin, root, "hub");
        var field = Slimefun.class.getDeclaredField("playerSync"); field.setAccessible(true); field.set(plugin, hub);
        hub.beginLoad(player); assertTrue(await(hub.apply(player, captured))); hub.onSyncLoaded(player);
        assertTrue(PlayerProfile.find(player).orElseThrow().getWaypoints().isEmpty());
        assertEquals("survival-world", YamlConfiguration.loadConfiguration(saved.toFile()).getString("home.world"));
    }

    @Test void borrowedBackpackDeniedAndOwnedOpenChecksCurrentState() throws Exception {
        ready(); var bag = PlayerProfile.find(player).orElseThrow().createBackpack(9);
        PlayerMock other = server.addPlayer(); bag.open(other); server.getScheduler().performOneTick();
        assertNotSame(bag.getInventory(), other.getOpenInventory().getTopInventory());
        bag.open(player); server.getScheduler().performOneTick(); assertSame(bag.getInventory(), player.getOpenInventory().getTopInventory());
        assertTrue(sync.prepareTransfer(player)); assertNotSame(bag.getInventory(), player.getOpenInventory().getTopInventory());
        bag.open(player); server.getScheduler().performOneTick(); assertNotSame(bag.getInventory(), player.getOpenInventory().getTopInventory());
        var token = new ItemStack(Material.CHEST); var meta = token.getItemMeta(); meta.setLore(List.of(ChatColor.GRAY + "ID: " + other.getUniqueId() + "#0")); token.setItemMeta(meta);
        assertFalse(BackpackListener.isOwner(player, token));
    }

    @Test void disconnectedSourceRetainsCompleteProfileUntilDurableCommit() throws Exception {
        ready(); var profile = PlayerProfile.find(player).orElseThrow(); profile.createBackpack(9);
        assertTrue(player.disconnect());
        assertTrue(sync.prepareTransfer(player));
        assertEquals(1, ProfileSnapshot.decode(player.getUniqueId(), sync.capture(player)).bags.size());
        assertSame(profile, Slimefun.getRegistry().getPlayerProfiles().get(player.getUniqueId()));
        sync.sourceCommitted(player);
        assertFalse(Slimefun.getRegistry().getPlayerProfiles().containsKey(player.getUniqueId()));
        assertTrue(sync.blocked(player));
    }

    @Test void brokenLegacyYamlFailsClosedWithoutTouchingFile() throws Exception {
        Path file = root.resolve("Players/" + player.getUniqueId() + ".yml"); Files.createDirectories(file.getParent());
        String broken = "backpacks: [broken"; Files.writeString(file, broken);
        sync.beginLoad(player);
        assertThrows(ExecutionException.class, () -> await(sync.bootstrap(player)));
        assertEquals(broken, Files.readString(file)); assertTrue(sync.blocked(player));
    }

    @Test void serverLocalGpsCanBeReloadedWithoutPullingOtherServerData() throws Exception {
        var world = server.addSimpleWorld("survival-world");
        ready(); var profile = PlayerProfile.find(player).orElseThrow();
        profile.addWaypoint(new Waypoint(player.getUniqueId(), "home", new Location(world, 1, 2, 3), "家"));
        assertTrue(sync.prepareTransfer(player)); var bytes = sync.capture(player); sync.sourceCommitted(player);
        sync.beginLoad(player); assertTrue(await(sync.apply(player, bytes))); sync.onSyncLoaded(player);
        var restored = PlayerProfile.find(player).orElseThrow().getWaypoints().getFirst();
        assertEquals(world.getUID(), restored.getLocation().getWorld().getUID());
        assertEquals(1, restored.getLocation().getX()); assertEquals("家", restored.getName());
    }

    @Test void oldResearchTimerCannotClearNewSessionDrainMarker() throws Exception {
        ready(); Slimefun.getRegistry().setResearchingEnabled(true);
        var research = new Research(new NamespacedKey("addon", "delayed"), 777, "Delayed", 1);
        research.unlock(player, false);
        assertTrue(Slimefun.getRegistry().getCurrentlyResearchingPlayers().contains(player.getUniqueId()));
        var next = new PlayerMock(server, "new-session", player.getUniqueId());
        sync.beginLoad(next); assertTrue(await(sync.apply(next, empty(true)))); sync.onSyncLoaded(next);
        Slimefun.getRegistry().getCurrentlyResearchingPlayers().add(player.getUniqueId());
        server.getScheduler().performTicks(105);
        assertTrue(Slimefun.getRegistry().getCurrentlyResearchingPlayers().contains(player.getUniqueId()));
        assertFalse(sync.prepareTransfer(next));
        assertTrue(Slimefun.getRegistry().getPlayerProfiles().get(player.getUniqueId()).getResearches().isEmpty());
    }

    @Test void quitDuringResearchCompletesPaidUnlockBeforeOfflineCapture() throws Exception {
        ready(); Slimefun.getRegistry().setResearchingEnabled(true);
        var research = new Research(new NamespacedKey("addon", "offline_delayed"), 778, "Offline delayed", 1);
        var profile = PlayerProfile.find(player).orElseThrow();
        research.unlock(player, false); player.disconnect();
        assertFalse(sync.prepareTransfer(player));
        server.getScheduler().performTicks(105);
        assertFalse(Slimefun.getRegistry().getCurrentlyResearchingPlayers().contains(player.getUniqueId()));
        assertTrue(profile.hasUnlocked(research));
        assertTrue(sync.prepareTransfer(player));
        assertTrue(ProfileSnapshot.decode(player.getUniqueId(), sync.capture(player)).researches.contains("addon:offline_delayed"));
    }

    @Test void firstGuideIsIssuedOnlyAfterFullLoadAndNeverAgainOnAnotherBackend() throws Exception {
        Slimefun.getCfg().setValue("guide.receive-on-first-join", true);
        server.addSimpleWorld("world");
        // The test setup doesn't load guide implementations; use the real survival guide.
        Slimefun.getRegistry().getSlimefunGuide(io.github.thebusybiscuit.slimefun4.core.guide.SlimefunGuideMode.SURVIVAL_MODE);
        sync.beginLoad(player); assertTrue(await(sync.apply(player, empty(false))));
        assertTrue(player.getInventory().isEmpty()); sync.onSyncLoaded(player);
        assertEquals(1, Arrays.stream(player.getInventory().getContents()).filter(Objects::nonNull).count());
        assertTrue(sync.prepareTransfer(player)); byte[] captured = sync.capture(player); assertTrue(ProfileSnapshot.decode(player.getUniqueId(), captured).guideIssued);
        sync.sourceCommitted(player); sync.beginLoad(player); assertTrue(await(sync.apply(player, captured))); sync.onSyncLoaded(player);
        assertEquals(1, Arrays.stream(player.getInventory().getContents()).filter(Objects::nonNull).count());
    }
}
