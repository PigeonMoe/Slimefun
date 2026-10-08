package io.github.thebusybiscuit.slimefun4.utils;

import java.nio.charset.StandardCharsets;
import java.util.Base64;

import org.mockbukkit.mockbukkit.MockBukkit;
import org.bukkit.profile.PlayerProfile;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.DisplayName;
import org.junit.jupiter.api.Test;

class TestCustomHeads {
    @BeforeEach
    void load() {
        MockBukkit.mock();
    }

    @AfterEach
    void unload() {
        MockBukkit.unmock();
    }

    @Test
    @DisplayName("Hashes and legacy Base64 textures use the public profile API")
    void testTextures() {
        String hash = HeadTexture.CAPACITOR_100.getTexture();
        String json = "{\"textures\":{\"SKIN\":{\"url\":\"https://textures.minecraft.net/texture/" + hash + "\"}}}";
        String base64 = Base64.getEncoder().encodeToString(json.getBytes(StandardCharsets.UTF_8));
        PlayerProfile profile = CustomHeads.getProfile(hash);
        Assertions.assertNotNull(profile.getTextures().getSkin());
        Assertions.assertEquals(CustomHeads.getProfile(base64).getTextures().getSkin(), profile.getTextures().getSkin());
        Assertions.assertEquals(CustomHeads.getProfile(base64).getUniqueId(), CustomHeads.getProfile(hash).getUniqueId());
    }
}
