package io.github.thebusybiscuit.slimefun4.utils;

import java.net.MalformedURLException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.UUID;

import javax.annotation.Nonnull;

import org.apache.commons.lang.Validate;
import org.bukkit.Bukkit;
import org.bukkit.Material;
import org.bukkit.block.Block;
import org.bukkit.block.Skull;
import org.bukkit.inventory.ItemStack;
import org.bukkit.inventory.meta.SkullMeta;
import org.bukkit.profile.PlayerProfile;
import org.bukkit.profile.PlayerTextures;

import com.google.gson.JsonParser;

/**
 * Applies custom head textures through the public Bukkit profile API.
 * No CraftBukkit fields, authlib subclasses or server mappings are required.
 *
 * @author PigeonMoe
 */
public final class CustomHeads {

    private CustomHeads() {}

    /** Creates a head from a Minecraft texture hash or a Base64 texture property. */
    public static @Nonnull ItemStack getItemStack(@Nonnull String texture) {
        ItemStack item = new ItemStack(Material.PLAYER_HEAD);
        SkullMeta meta = (SkullMeta) item.getItemMeta();
        meta.setOwnerProfile(getProfile(texture));
        item.setItemMeta(meta);
        return item;
    }

    /** Updates a placed player head without replacing its rotation or block data. */
    public static void setSkin(@Nonnull Block block, @Nonnull String texture, boolean applyPhysics) {
        Validate.isTrue(block.getState() instanceof Skull, "The block must be a player head");
        Skull skull = (Skull) block.getState();
        skull.setOwnerProfile(getProfile(texture));
        skull.update(true, applyPhysics);
    }

    /** Encode a resolved public profile for the existing contributor texture cache. */
    public static @Nonnull String encodeProfile(@Nonnull PlayerProfile profile) {
        Validate.notNull(profile.getTextures().getSkin(), "The profile must have a skin");
        com.google.gson.JsonObject skin = new com.google.gson.JsonObject();
        skin.addProperty("url", profile.getTextures().getSkin().toExternalForm());
        com.google.gson.JsonObject textures = new com.google.gson.JsonObject();
        textures.add("SKIN", skin);
        com.google.gson.JsonObject root = new com.google.gson.JsonObject();
        root.add("textures", textures);
        return Base64.getEncoder().encodeToString(root.toString().getBytes(StandardCharsets.UTF_8));
    }

    /** Creates a deterministic profile without making a network request. */
    public static @Nonnull PlayerProfile getProfile(@Nonnull String texture) {
        Validate.notNull(texture, "The texture must not be null");
        String url;
        if (texture.matches("[a-fA-F0-9]+")) {
            url = "https://textures.minecraft.net/texture/" + texture;
        } else {
            String json = new String(Base64.getDecoder().decode(texture), StandardCharsets.UTF_8);
            url = JsonParser.parseString(json).getAsJsonObject().getAsJsonObject("textures")
                .getAsJsonObject("SKIN").get("url").getAsString();
        }

        URI uri = URI.create(url);
        Validate.isTrue("textures.minecraft.net".equals(uri.getHost()), "Expected a Minecraft texture URL");
        Validate.isTrue("http".equals(uri.getScheme()) || "https".equals(uri.getScheme()), "Expected an HTTP texture URL");
        PlayerProfile profile = Bukkit.createPlayerProfile(UUID.nameUUIDFromBytes(url.getBytes(StandardCharsets.UTF_8)));
        PlayerTextures textures = profile.getTextures();
        try {
            textures.setSkin(uri.toURL());
        } catch (MalformedURLException e) {
            throw new IllegalArgumentException("Invalid texture URL", e);
        }
        profile.setTextures(textures);
        return profile;
    }
}
