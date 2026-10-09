package io.github.thebusybiscuit.slimefun4.core.services.sync;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.concurrent.*;

import org.bukkit.Bukkit;
import org.bukkit.configuration.ConfigurationSection;
import org.bukkit.configuration.file.YamlConfiguration;
import org.bukkit.entity.Player;
import org.bukkit.inventory.ItemStack;
import org.bukkit.plugin.ServicePriority;

import io.github.thebusybiscuit.slimefun4.api.gps.Waypoint;
import io.github.thebusybiscuit.slimefun4.api.player.PlayerBackpack;
import io.github.thebusybiscuit.slimefun4.api.player.PlayerProfile;
import io.github.thebusybiscuit.slimefun4.api.researches.Research;
import io.github.thebusybiscuit.slimefun4.implementation.Slimefun;
import io.github.thebusybiscuit.slimefun4.storage.data.PlayerData;
import moe.pigeon.slimefun.api.PlayerSyncBridge;

/** External profile participant. SQL ownership, commit and timeouts belong to PM-Sync. */
public final class PlayerSyncService implements PlayerSyncBridge {
    private enum Phase { LOADING, LOADED, READY, PREPARED, FAILED }
    private static final class Session {
        final Player player;
        volatile Phase phase = Phase.LOADING;
        PlayerProfile profile;
        boolean guideIssued;
        Set<String> unknownKeys = Set.of();
        Set<Integer> unknownIds = Set.of();
        YamlConfiguration localPoints;
        Set<String> loadedPointIds = Set.of();
        Session(Player player) { this.player = player; }
    }

    private final Slimefun plugin;
    private final Path root;
    private final String serverId;
    private final Map<UUID, Session> sessions = new ConcurrentHashMap<>();

    public PlayerSyncService(Slimefun plugin, Path root, String serverId) {
        if (serverId == null || !serverId.matches("[a-z0-9_-]{1,64}")) throw new IllegalArgumentException("player-sync.server-id is required (hub/survival)");
        this.plugin = plugin;
        this.root = root;
        this.serverId = serverId;
    }

    public void register() {
        Bukkit.getServicesManager().register(PlayerSyncBridge.class, this, plugin, ServicePriority.Normal);
        Bukkit.getPluginManager().registerEvents(new SyncInteractionGuard(this), plugin);
        plugin.getLogger().info("PM-Sync player profile participant enabled, server-id=" + serverId + "; SQL load required before interaction");
    }

    @Override public int apiVersion() { return 1; }

    @Override public void beginLoad(Player player) {
        mainThread();
        Session previous = sessions.get(player.getUniqueId());
        if (previous != null && previous.player == player) throw new IllegalStateException("Duplicate Slimefun beginLoad");
        if (previous != null && previous.profile != null) closeBackpacks(previous.profile);
        Slimefun.getRegistry().getPlayerProfiles().remove(player.getUniqueId());
        Slimefun.getRegistry().getCurrentlyResearchingPlayers().remove(player.getUniqueId());
        sessions.put(player.getUniqueId(), new Session(player));
    }

    @Override public boolean blocked(Player player) {
        Session session = sessions.get(player.getUniqueId());
        return session == null || session.player != player || session.phase != Phase.READY;
    }

    public Optional<PlayerProfile> find(UUID owner) {
        Session session = sessions.get(owner);
        return session != null && session.phase == Phase.READY ? Optional.of(session.profile) : Optional.empty();
    }

    public boolean usable(PlayerProfile profile) {
        Session session = sessions.get(profile.getUUID());
        return session != null && session.profile == profile && session.phase == Phase.READY;
    }

    @Override public boolean prepareTransfer(Player player) {
        mainThread();
        Session session = session(player);
        if (session.phase == Phase.PREPARED) return true;
        if (session.phase != Phase.READY) return false;
        // Let paid research finish before taking either the XP or external profile snapshot.
        if (Slimefun.getRegistry().getCurrentlyResearchingPlayers().contains(player.getUniqueId())) return false;
        closeBackpacks(session.profile);
        // Do not close unrelated plugins' menus during periodic checkpoints.
        if (me.mrCookieSlime.CSCoreLibPlugin.general.Inventory.MenuListener.hasOpenMenu(player)) {
            player.closeInventory();
        }
        try {
            saveLocalPoints(session);
            session.phase = Phase.PREPARED;
            return true;
        } catch (Exception e) {
            session.phase = Phase.FAILED;
            throw new IllegalStateException("Cannot save server-local Slimefun GPS data", e);
        }
    }

    @Override public byte[] capture(Player player) {
        mainThread();
        Session session = session(player);
        if (session.phase != Phase.PREPARED) throw new IllegalStateException("Slimefun capture requires prepareTransfer");
        try {
            return snapshot(session).encode();
        } catch (RuntimeException e) {
            session.phase = Phase.FAILED;
            throw e;
        }
    }

    @Override public CompletableFuture<byte[]> bootstrap(Player player) {
        mainThread();
        Session session = loading(player);
        return readAsync(root.resolve("Players").resolve(player.getUniqueId() + ".yml"))
            .thenCompose(text -> onMain(() -> {
                requireCurrent(session);
                try {
                    YamlConfiguration yaml = yaml(text);
                    Set<Integer> legacy = new HashSet<>();
                    ConfigurationSection research = section(yaml, "researches");
                    if (research != null) for (String id : research.getKeys(false)) {
                        Integer numeric = Integer.valueOf(id);
                        if (!(research.get(id) instanceof Boolean)) throw new IllegalArgumentException("Invalid legacy research value");
                        legacy.add(numeric); // Legacy contains(path) semantics, including false.
                    }
                    List<ProfileSnapshot.Bag> bags = new ArrayList<>();
                    ConfigurationSection backpacks = section(yaml, "backpacks");
                    if (backpacks != null) for (String id : backpacks.getKeys(false)) {
                        ConfigurationSection bag = section(backpacks, id);
                        if (bag == null || !bag.isInt("size")) throw new IllegalArgumentException("Invalid legacy backpack");
                        int size = bag.getInt("size");
                        if (size < 9 || size > 54 || size % 9 != 0) throw new IllegalArgumentException("Invalid legacy backpack size");
                        ConfigurationSection contents = section(bag, "contents");
                        if (contents != null) for (String slot : contents.getKeys(false)) {
                            int number = Integer.parseInt(slot);
                            if (!Integer.toString(number).equals(slot) || number < 0 || number >= size || !(contents.get(slot) instanceof ItemStack)) throw new IllegalArgumentException("Invalid legacy backpack slot");
                        }
                        List<byte[]> items = new ArrayList<>();
                        for (int slot = 0; slot < size; slot++) items.add(encodeItem(bag.getItemStack("contents." + slot)));
                        bags.add(new ProfileSnapshot.Bag(Integer.parseInt(id), items));
                    }
                    Set<String> keys = new HashSet<>();
                    for (Research r : Slimefun.getRegistry().getResearches()) if (legacy.contains(r.getID())) keys.add(r.getKey().toString());
                    // Preserve numeric IDs only when their addon is currently absent.
                    for (Research r : Slimefun.getRegistry().getResearches()) legacy.remove(r.getID());
                    return new ProfileSnapshot(player.getUniqueId(), player.hasPlayedBefore() || !text.isBlank(), keys, legacy, bags).encode();
                } catch (Exception e) {
                    session.phase = Phase.FAILED;
                    throw new IllegalArgumentException("Legacy Slimefun migration refused; original files untouched", e);
                }
            })).whenComplete((result, error) -> { if (error != null) session.phase = Phase.FAILED; });
    }

    @Override public CompletableFuture<Boolean> apply(Player player, byte[] bytes) {
        mainThread();
        Session session = loading(player);
        final ProfileSnapshot snapshot;
        try {
            snapshot = ProfileSnapshot.decode(player.getUniqueId(), bytes);
        } catch (RuntimeException e) {
            session.phase = Phase.FAILED;
            return CompletableFuture.failedFuture(e);
        }
        Path localFile = pointsPath(player.getUniqueId());
        return readAsync(localFile, false).thenCompose(text -> text == null
            ? readAsync(root.resolve("waypoints").resolve(player.getUniqueId() + ".yml"))
            : CompletableFuture.completedFuture(text))
            .thenCompose(text -> onMain(() -> {
                requireCurrent(session);
                try {
                    Set<Research> researches = new HashSet<>();
                    Set<String> unknownKeys = new HashSet<>(snapshot.researches);
                    Set<Integer> unknownIds = new HashSet<>(snapshot.legacyResearches);
                    for (Research r : Slimefun.getRegistry().getResearches()) {
                        if (snapshot.researches.contains(r.getKey().toString()) || snapshot.legacyResearches.contains(r.getID())) researches.add(r);
                        unknownKeys.remove(r.getKey().toString());
                        unknownIds.remove(r.getID());
                    }
                    Map<Integer, PlayerBackpack> bags = new HashMap<>();
                    for (ProfileSnapshot.Bag bag : snapshot.bags) {
                        HashMap<Integer, ItemStack> items = new HashMap<>();
                        for (int slot = 0; slot < bag.slots().size(); slot++) {
                            byte[] item = bag.slots().get(slot);
                            if (item.length != 0) {
                                ItemStack restored = ItemStack.deserializeBytes(item);
                                if (restored == null || restored.getType().isAir() || restored.getAmount() <= 0) throw new IllegalArgumentException("Invalid nonempty backpack item");
                                items.put(slot, restored);
                            }
                        }
                        bags.put(bag.id(), PlayerBackpack.load(player.getUniqueId(), bag.id(), bag.slots().size(), items));
                    }
                    YamlConfiguration points = yaml(text);
                    Set<Waypoint> waypoints = new HashSet<>();
                    Set<String> loadedIds = new HashSet<>();
                    for (String id : points.getKeys(false)) {
                        if (!points.isConfigurationSection(id) || !points.isString(id + ".world")) throw new IllegalArgumentException("Invalid local GPS waypoint");
                        var world = points.isString(id + ".world_uuid")
                            ? Bukkit.getWorld(UUID.fromString(points.getString(id + ".world_uuid")))
                            : Bukkit.getWorld(points.getString(id + ".world"));
                        if (world == null) continue; // Keep unmounted worlds verbatim; never resolve a different same-name world.
                        ConfigurationSection point = points.getConfigurationSection(id);
                        for (String coordinate : List.of("x", "y", "z")) {
                            if (!(point.get(coordinate) instanceof Number) || !Double.isFinite(point.getDouble(coordinate))) throw new IllegalArgumentException("Invalid GPS coordinate");
                        }
                        for (String angle : List.of("yaw", "pitch")) {
                            if (point.contains(angle) && (!(point.get(angle) instanceof Number) || !Float.isFinite((float) point.getDouble(angle)))) throw new IllegalArgumentException("Invalid GPS angle");
                        }
                        var location = new org.bukkit.Location(world, point.getDouble("x"), point.getDouble("y"), point.getDouble("z"),
                            (float) point.getDouble("yaw"), (float) point.getDouble("pitch"));
                        if (!points.isString(id + ".name")) throw new IllegalArgumentException("Invalid local GPS location");
                        waypoints.add(new Waypoint(player.getUniqueId(), id, location, points.getString(id + ".name")));
                        loadedIds.add(id);
                    }
                    PlayerProfile profile = PlayerProfile.fromSync(player, new PlayerData(researches, bags, waypoints));
                    session.profile = profile;
                    session.guideIssued = snapshot.guideIssued;
                    session.unknownKeys = unknownKeys;
                    session.unknownIds = unknownIds;
                    session.localPoints = points;
                    session.loadedPointIds = loadedIds;
                    Slimefun.getRegistry().getPlayerProfiles().put(player.getUniqueId(), profile);
                    session.phase = Phase.LOADED;
                    return true;
                } catch (Exception e) {
                    session.phase = Phase.FAILED;
                    throw new IllegalArgumentException("Cannot apply complete Slimefun profile", e);
                }
            })).whenComplete((result, error) -> { if (error != null) session.phase = Phase.FAILED; });
    }

    @Override public void onSyncLoaded(Player player) {
        mainThread();
        Session session = session(player);
        if (session.phase != Phase.LOADED) throw new IllegalStateException("Slimefun profile has not completed loading");
        try {
            if (!session.guideIssued) {
                if (Slimefun.getCfg().getBoolean("guide.receive-on-first-join")
                    && Slimefun.getWorldSettingsService().isWorldEnabled(player.getWorld())) {
                    var guide = io.github.thebusybiscuit.slimefun4.core.guide.SlimefunGuide.getItem(
                        io.github.thebusybiscuit.slimefun4.core.guide.SlimefunGuide.getDefaultMode()).clone();
                    player.getInventory().addItem(guide);
                }
                session.guideIssued = true; // First network initialization, never each backend's first join.
            }
            session.phase = Phase.READY;
        } catch (RuntimeException e) {
            session.phase = Phase.FAILED;
            throw e;
        }
    }

    @Override public void abortTransfer(Player player) {
        mainThread();
        Session session = session(player);
        if (session.phase != Phase.PREPARED) throw new IllegalStateException("Cannot resume an unprepared/failed Slimefun profile");
        session.phase = Phase.READY;
    }

    @Override public void sourceCommitted(Player player) {
        mainThread();
        Session session = session(player);
        if (session.phase != Phase.PREPARED) throw new IllegalStateException("Slimefun source was not prepared");
        sessions.remove(player.getUniqueId(), session);
        Slimefun.getRegistry().getPlayerProfiles().remove(player.getUniqueId(), session.profile);
    }

    private ProfileSnapshot snapshot(Session session) {
        Set<String> keys = new HashSet<>(session.unknownKeys);
        for (Research r : session.profile.getPlayerData().getResearches()) keys.add(r.getKey().toString());
        List<ProfileSnapshot.Bag> bags = new ArrayList<>();
        session.profile.getPlayerData().getBackpacks().forEach((id, bag) -> {
            if (id != bag.getId() || bag.getSize() != bag.getInventory().getSize() || !bag.getOwnerId().equals(session.player.getUniqueId())) throw new IllegalArgumentException("Backpack owner/ID mismatch");
            List<byte[]> slots = new ArrayList<>();
            for (ItemStack item : bag.getInventory().getContents()) slots.add(encodeItem(item));
            bags.add(new ProfileSnapshot.Bag(id, slots));
        });
        return new ProfileSnapshot(session.player.getUniqueId(), session.guideIssued, keys, session.unknownIds, bags);
    }

    private static byte[] encodeItem(ItemStack item) {
        return item == null || item.getType().isAir() ? new byte[0] : item.serializeAsBytes();
    }

    private Path pointsPath(UUID owner) {
        return root.resolve("player-sync").resolve(serverId).resolve("waypoints").resolve(owner + ".yml");
    }

    private void saveLocalPoints(Session session) throws Exception {
        YamlConfiguration points = yaml(session.localPoints.saveToString());
        for (String id : session.loadedPointIds) points.set(id, null);
        Set<String> newIds = new HashSet<>();
        for (Waypoint point : session.profile.getPlayerData().getWaypoints()) {
            var location = point.getLocation();
            if (!session.player.getUniqueId().equals(point.getOwnerId()) || location.getWorld() == null
                || !Double.isFinite(location.getX()) || !Double.isFinite(location.getY()) || !Double.isFinite(location.getZ())) {
                throw new IllegalArgumentException("Invalid GPS owner/location");
            }
            Map<String, Object> value = new LinkedHashMap<>();
            value.put("world", location.getWorld().getName());
            value.put("world_uuid", location.getWorld().getUID().toString());
            value.put("world_key", location.getWorld().getKey().toString());
            value.put("x", location.getX()); value.put("y", location.getY()); value.put("z", location.getZ());
            value.put("yaw", location.getYaw()); value.put("pitch", location.getPitch());
            value.put("name", point.getName());
            if (points.contains(point.getId())) throw new IllegalArgumentException("GPS ID collides with an unmounted world's waypoint");
            points.set(point.getId(), null);
            points.createSection(point.getId(), value);
            newIds.add(point.getId());
        }
        Path destination = pointsPath(session.player.getUniqueId());
        String serialized = points.saveToString();
        if (serialized.equals(session.localPoints.saveToString()) && Files.isRegularFile(destination)) return;
        Files.createDirectories(destination.getParent());
        Path temporary = Files.createTempFile(destination.getParent(), "profile-", ".tmp");
        try {
            Files.writeString(temporary, serialized, StandardCharsets.UTF_8);
            try (var channel = java.nio.channels.FileChannel.open(temporary, StandardOpenOption.WRITE)) { channel.force(true); }
            Files.move(temporary, destination, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
        } finally {
            Files.deleteIfExists(temporary);
        }
        session.localPoints = points;
        session.loadedPointIds = newIds;
    }

    private static ConfigurationSection section(ConfigurationSection yaml, String path) {
        Object value = yaml.get(path);
        if (value != null && !(value instanceof ConfigurationSection)) throw new IllegalArgumentException("Expected section: " + path);
        return yaml.getConfigurationSection(path);
    }

    private static YamlConfiguration yaml(String text) throws Exception {
        YamlConfiguration yaml = new YamlConfiguration();
        yaml.loadFromString(text); // Unlike loadConfiguration, parse errors must not turn into an empty profile.
        return yaml;
    }

    private CompletableFuture<String> readAsync(Path path) { return readAsync(path, true); }

    private CompletableFuture<String> readAsync(Path path, boolean missingEmpty) {
        CompletableFuture<String> result = new CompletableFuture<>();
        Bukkit.getScheduler().runTaskAsynchronously(plugin, () -> {
            try {
                var attributes = Files.readAttributes(path, java.nio.file.attribute.BasicFileAttributes.class);
                if (!attributes.isRegularFile() || attributes.size() > ProfileSnapshot.MAX_BYTES) throw new IOException("Invalid/oversize Slimefun data file");
                result.complete(Files.readString(path, StandardCharsets.UTF_8));
            } catch (NoSuchFileException e) { result.complete(missingEmpty ? "" : null); }
            catch (Exception e) { result.completeExceptionally(e); }
        });
        return result;
    }

    private <T> CompletableFuture<T> onMain(Callable<T> action) {
        CompletableFuture<T> result = new CompletableFuture<>();
        Runnable task = () -> { try { result.complete(action.call()); } catch (Exception e) { result.completeExceptionally(e); } };
        if (Bukkit.isPrimaryThread()) task.run();
        else Bukkit.getScheduler().runTask(plugin, task);
        return result;
    }

    private Session loading(Player player) {
        Session session = session(player);
        if (session.phase != Phase.LOADING) throw new IllegalStateException("Slimefun profile is not awaiting SQL load");
        return session;
    }

    private Session session(Player player) {
        Session session = sessions.get(player.getUniqueId());
        if (session == null || session.player != player) throw new IllegalStateException("Stale/missing Slimefun session");
        return session;
    }

    private void requireCurrent(Session session) {
        if (sessions.get(session.player.getUniqueId()) != session || session.phase != Phase.LOADING) throw new IllegalStateException("Stale Slimefun profile load");
    }

    private static void closeBackpacks(PlayerProfile profile) {
        for (PlayerBackpack bag : profile.getPlayerData().getBackpacks().values()) {
            for (var viewer : new ArrayList<>(bag.getInventory().getViewers())) viewer.closeInventory();
        }
    }

    private static void mainThread() {
        if (!Bukkit.isPrimaryThread()) throw new IllegalStateException("Slimefun sync bridge requires the server thread");
    }
}
