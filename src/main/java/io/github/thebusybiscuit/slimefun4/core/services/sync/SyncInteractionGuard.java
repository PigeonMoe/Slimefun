package io.github.thebusybiscuit.slimefun4.core.services.sync;

import org.bukkit.entity.Player;
import org.bukkit.event.*;
import org.bukkit.event.inventory.*;
import org.bukkit.event.player.*;
import io.github.thebusybiscuit.slimefun4.api.events.PlayerPreResearchEvent;
import io.github.thebusybiscuit.slimefun4.api.events.SlimefunGuideOpenEvent;

/** Fail closed even when the coordinator is missing, disabled or still loading SQL. */
final class SyncInteractionGuard implements Listener {
    private final PlayerSyncService sync;
    SyncInteractionGuard(PlayerSyncService sync) { this.sync = sync; }

    @EventHandler(priority = EventPriority.LOWEST) public void interact(PlayerInteractEvent e) {
        if (sync.blocked(e.getPlayer())) e.setCancelled(true);
    }
    @EventHandler(priority = EventPriority.LOWEST) public void click(InventoryClickEvent e) {
        if (e.getWhoClicked() instanceof Player p && sync.blocked(p)) e.setCancelled(true);
    }
    @EventHandler(priority = EventPriority.LOWEST) public void drag(InventoryDragEvent e) {
        if (e.getWhoClicked() instanceof Player p && sync.blocked(p)) e.setCancelled(true);
    }
    @EventHandler(priority = EventPriority.LOWEST) public void open(InventoryOpenEvent e) {
        if (e.getPlayer() instanceof Player p && sync.blocked(p)) e.setCancelled(true);
    }
    @EventHandler(priority = EventPriority.LOWEST) public void guide(SlimefunGuideOpenEvent e) {
        if (sync.blocked(e.getPlayer())) e.setCancelled(true);
    }
    @EventHandler(priority = EventPriority.LOWEST) public void research(PlayerPreResearchEvent e) {
        if (sync.blocked(e.getPlayer())) e.setCancelled(true);
    }
    @EventHandler(priority = EventPriority.LOWEST) public void command(PlayerCommandPreprocessEvent e) {
        String command = e.getMessage().split(" ", 2)[0].toLowerCase(java.util.Locale.ROOT);
        if (sync.blocked(e.getPlayer()) && java.util.Set.of("/sf", "/slimefun", "/slimefun:sf", "/slimefun:slimefun").contains(command)) e.setCancelled(true);
    }
}
