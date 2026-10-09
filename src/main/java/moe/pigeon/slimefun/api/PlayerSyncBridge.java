package moe.pigeon.slimefun.api;

import java.util.concurrent.CompletableFuture;
import org.bukkit.entity.Player;

/**
 * GPLv3 Slimefun-owned lifecycle service. Coordinators may discover this FQCN via
 * Bukkit's ServicesManager and invoke it reflectively without bundling this API.
 * All entry points run on the server thread. Completion does not unlock a player.
 */
public interface PlayerSyncBridge {
    int apiVersion();
    void beginLoad(Player player);
    boolean blocked(Player player);
    boolean prepareTransfer(Player player);
    byte[] capture(Player player);
    CompletableFuture<byte[]> bootstrap(Player player);
    CompletableFuture<Boolean> apply(Player player, byte[] snapshot);
    void onSyncLoaded(Player player);
    void abortTransfer(Player player);
    void sourceCommitted(Player player);
}
