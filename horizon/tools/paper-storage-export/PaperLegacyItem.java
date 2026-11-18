// SPDX-License-Identifier: GPL-3.0-only
package moe.pigeon.horizon.migration;

import com.mojang.serialization.Dynamic;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import net.minecraft.SharedConstants;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.nbt.NbtOps;
import net.minecraft.nbt.Tag;
import net.minecraft.nbt.TagParser;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.util.datafix.fixes.References;
import org.bukkit.configuration.serialization.ConfigurationSerialization;
import org.bukkit.inventory.ItemStack;
import org.bukkit.inventory.meta.ItemMeta;

final class PaperLegacyItem {
    private static final int FIRST_COMPONENT_VERSION = 3837;
    private static final String CUSTOM_DATA = "minecraft:custom_data";

    private PaperLegacyItem() {}

    static Map<String, Object> normalize(Map<?, ?> input) throws Exception {
        if (input.containsKey("schema_version")) {
            throw new IOException("Unsupported Paper item schema " + input.get("schema_version"));
        }
        if (!(input.get("type") instanceof String type)) {
            throw new IOException("The legacy item has no material.");
        }
        if (input.containsKey("v") && !(input.get("v") instanceof Integer)) {
            throw new IOException("The legacy item has an invalid data version.");
        }
        Map<Object, Object> raw = new LinkedHashMap<>(input);
        CompoundTag oldTag = new CompoundTag();
        CompoundTag custom = new CompoundTag();
        int version = input.get("v") instanceof Integer number ? number : -1;
        if (raw.get("meta") instanceof Map<?, ?> source) {
            if (raw.get("enchantments") != null) {
                throw new IOException("The legacy item contains both enchantments and ItemMeta.");
            }
            Map<Object, Object> meta = new LinkedHashMap<>(source);
            CompoundTag internal = readCompressed(meta.get("internal"), "internal");
            custom = readCompressed(meta.get("custom"), "custom");
            readCompressed(meta.get("unhandled"), "unhandled");
            if (version >= 0 && version < FIRST_COMPONENT_VERSION) {
                if (type.equals("PLAYER_HEAD") && "SKULL".equals(meta.get("meta-type"))) {
                    Tag profile = internal.remove("SkullProfile");
                    Object owner = meta.get("skull-owner");
                    if (profile != null) {
                        if (!(profile instanceof CompoundTag ownerTag)
                                || !Set.of("Name", "Id", "Properties").containsAll(ownerTag.keySet())
                                || (owner != null && !ownerTag.getString("Name").orElse("").equals(owner))) {
                            throw new IOException("The legacy skull profile is ambiguous.");
                        }
                        oldTag.put("SkullOwner", profile);
                        meta.remove("skull-owner");
                    } else if (owner instanceof String name) {
                        oldTag.putString("SkullOwner", name);
                        meta.remove("skull-owner");
                    }
                } else if (type.endsWith("_SPAWN_EGG") && "SPAWN_EGG".equals(meta.get("meta-type"))) {
                    // Old Paper stored the spawn egg's native tag in the same envelope as plugin NBT.
                    oldTag = internal;
                    internal = new CompoundTag();
                    Tag entity = oldTag.remove("entity-tag");
                    if (entity != null && oldTag.put("EntityTag", entity) != null) {
                        throw new IOException("The legacy spawn egg has two entity tags.");
                    }
                }
            }
            if (internal.isEmpty()) {
                meta.remove("internal");
            } else if (oldTag.contains("SkullOwner")) {
                ByteArrayOutputStream bytes = new ByteArrayOutputStream();
                NbtIo.writeCompressed(internal, bytes);
                meta.put("internal", Base64.getEncoder().encodeToString(bytes.toByteArray()));
            }
            Object decoded = decodeValue(meta, 0);
            if (!(decoded instanceof ItemMeta itemMeta)) {
                throw new IOException("Paper could not read the legacy ItemMeta.");
            }
            if (!internal.isEmpty() && !internal.equals(readCompressed(
                    itemMeta.serialize().get("internal"), "resaved internal"))) {
                throw new IOException("Paper cannot preserve legacy internal tags " + internal.keySet()
                        + ". Convert this item with a compatible Paper version first.");
            }
            raw.put("meta", itemMeta);
        } else if (raw.get("meta") != null) {
            throw new IOException("The legacy ItemMeta must be a mapping.");
        }

        if (!(decodeValue(raw, 0) instanceof ItemStack stack)) {
            throw new IOException("Paper could not deserialize the legacy item.");
        }
        Map<String, Object> encoded = new LinkedHashMap<>(stack.serialize());
        if (!Integer.valueOf(1).equals(encoded.get("schema_version"))
                || !(encoded.get("id") instanceof String id)) {
            throw new IOException("Paper did not serialize the legacy item using schema 1.");
        }
        if ("minecraft:air".equals(id)) {
            if (!Set.of("AIR", "CAVE_AIR", "VOID_AIR").contains(type) || raw.get("meta") != null) {
                throw new IOException("Paper decoded a nonempty legacy item as air.");
            }
        } else if (!raw.getOrDefault("amount", 1).equals(encoded.get("count"))) {
            throw new IOException("Paper changed the legacy item count.");
        }
        verifyAndMergeTags(encoded, id, version, oldTag, custom);
        encoded.put("==", "org.bukkit.inventory.ItemStack");
        return encoded;
    }

    private static void verifyAndMergeTags(Map<String, Object> encoded, String id, int version,
            CompoundTag oldTag, CompoundTag custom) throws Exception {
        if (oldTag.isEmpty() && custom.isEmpty()) {
            return;
        }
        Map<Object, Object> components = encoded.get("components") instanceof Map<?, ?> current
                ? new LinkedHashMap<>(current) : new LinkedHashMap<>();
        if (!custom.isEmpty()) {
            Object saved = components.get(CUSTOM_DATA);
            if (!(saved instanceof String snbt) || !contains(TagParser.parseCompoundFully(snbt), custom)) {
                throw new IOException("Paper did not preserve the legacy custom tag.");
            }
        }
        if (!oldTag.isEmpty()) {
            CompoundTag item = new CompoundTag();
            item.putString("id", id);
            item.putByte("Count", (byte) 1);
            item.put("tag", oldTag);
            Tag result = DataFixers.getDataFixer().update(References.ITEM_STACK,
                    new Dynamic<>(NbtOps.INSTANCE, item), version,
                    SharedConstants.getCurrentVersion().dataVersion().version()).getValue();
            CompoundTag fixed = result.asCompound()
                    .filter(value -> id.equals(value.getString("id").orElse(null)))
                    .flatMap(value -> value.getCompound("components"))
                    .filter(value -> !value.isEmpty())
                    .orElseThrow(() -> new IOException("Minecraft could not upgrade the legacy native tags."));
            if (oldTag.contains("SkullOwner") && !fixed.contains("minecraft:profile")) {
                throw new IOException("Minecraft could not upgrade the legacy skull profile.");
            }
            for (String key : fixed.keySet()) {
                Tag value = fixed.get(key);
                Object previous = components.get(key);
                if (CUSTOM_DATA.equals(key) && previous instanceof String snbt) {
                    CompoundTag combined = TagParser.parseCompoundFully(snbt);
                    CompoundTag extra = value.asCompound().orElseThrow();
                    for (String field : extra.keySet()) {
                        Tag old = combined.put(field, extra.get(field));
                        if (old != null && !old.equals(extra.get(field))) {
                            throw new IOException("Conflicting legacy custom tag " + field);
                        }
                    }
                    value = combined;
                } else if (previous instanceof String snbt
                        && !TagParser.parseCompoundFully("{value:" + snbt + "}").get("value").equals(value)) {
                    throw new IOException("Conflicting legacy item component " + key);
                }
                components.put(key, value.toString());
            }
        }
        encoded.put("components", components);
        ItemStack.deserialize(encoded);
    }

    private static boolean contains(CompoundTag saved, CompoundTag expected) {
        return expected.keySet().stream().allMatch(key -> expected.get(key).equals(saved.get(key)));
    }

    private static CompoundTag readCompressed(Object raw, String field) throws IOException {
        if (raw == null) {
            return new CompoundTag();
        }
        try {
            if (!(raw instanceof String encoded)) {
                throw new IOException("Expected a base64 string.");
            }
            byte[] bytes = Base64.getDecoder().decode(encoded);
            return NbtIo.readCompressed(new ByteArrayInputStream(bytes), NbtAccounter.create(8 << 20));
        } catch (IOException | RuntimeException ex) {
            throw new IOException("Invalid compressed NBT in legacy ItemMeta field " + field, ex);
        }
    }

    private static Object decodeValue(Object raw, int depth) throws IOException {
        if (depth >= 100) {
            throw new IOException("The legacy item exceeds the Bukkit metadata nesting limit.");
        }
        if (raw instanceof Map<?, ?> mapping) {
            Map<String, Object> values = new LinkedHashMap<>();
            for (var entry : mapping.entrySet()) {
                String key = String.valueOf(entry.getKey());
                if (values.containsKey(key)) {
                    throw new IOException("Repeated legacy metadata field " + key);
                }
                values.put(key, decodeValue(entry.getValue(), depth + 1));
            }
            if (!values.containsKey("==")) {
                return values;
            }
            Object decoded = ConfigurationSerialization.deserializeObject(values);
            if (decoded == null) {
                throw new IOException("Paper could not deserialize " + values.get("=="));
            }
            return decoded;
        }
        if (raw instanceof List<?> list) {
            List<Object> values = new ArrayList<>(list.size());
            for (Object entry : list) {
                values.add(decodeValue(entry, depth + 1));
            }
            return values;
        }
        return raw;
    }
}
