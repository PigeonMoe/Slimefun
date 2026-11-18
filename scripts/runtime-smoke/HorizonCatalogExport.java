package dev.pigeonmoe.slimefun.testing;

import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import io.github.thebusybiscuit.slimefun4.api.items.SlimefunItem;
import io.github.thebusybiscuit.slimefun4.api.researches.Research;
import io.github.thebusybiscuit.slimefun4.core.multiblocks.MultiBlockMachine;
import io.github.thebusybiscuit.slimefun4.implementation.Slimefun;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Base64;
import java.util.Comparator;
import java.util.List;
import org.bukkit.Bukkit;
import org.bukkit.Material;
import org.bukkit.inventory.ItemStack;

/** Exports the live Paper registries to a fixture used to compare the Horizon port. */
final class HorizonCatalogExport {
    private HorizonCatalogExport() {}

    static void write(Path path) throws IOException {
        var registry = Slimefun.getRegistry();
        JsonObject catalog = new JsonObject();
        catalog.addProperty("format", 1);
        catalog.addProperty("minecraft", Bukkit.getMinecraftVersion());
        catalog.addProperty("slimefun", Slimefun.instance().getPluginMeta().getVersion());

        JsonArray items = new JsonArray();
        JsonArray machines = new JsonArray();
        registry.getAllSlimefunItems().stream().sorted(Comparator.comparing(SlimefunItem::getId)).forEach(item -> {
            JsonObject record = new JsonObject();
            record.addProperty("id", item.getId());
            record.addProperty("behavior", item.getClass().getName());
            record.addProperty("group", item.getItemGroup().getKey().toString());
            record.addProperty("recipe_type", item.getRecipeType().getKey().toString());
            record.addProperty("hidden", item.isHidden());
            record.addProperty("enabled", !item.isDisabled());
            record.add("item", describe(item.getItem()));
            record.add("ingredients", describe(item.getRecipe()));
            record.add("output", describe(item.getRecipeOutput()));
            if (item.getResearch() != null) {
                record.addProperty("research", item.getResearch().getKey().toString());
            }
            items.add(record);
            if (item instanceof MultiBlockMachine machine) {
                machines.add(describe(machine));
            }
        });
        catalog.add("items", items);
        catalog.add("multiblocks", machines);

        JsonArray researches = new JsonArray();
        registry.getResearches().stream().sorted(Comparator.comparing(research -> research.getKey().toString()))
            .forEach(research -> researches.add(describe(research)));
        catalog.add("researches", researches);

        try (var writer = Files.newBufferedWriter(path)) {
            new GsonBuilder().serializeNulls().setPrettyPrinting().create().toJson(catalog, writer);
        }
    }

    private static JsonObject describe(MultiBlockMachine machine) {
        JsonObject record = new JsonObject();
        record.addProperty("id", machine.getId());
        record.addProperty("trigger", machine.getMultiBlock().getTriggerBlock().name());
        JsonArray layout = new JsonArray();
        for (Material material : machine.getMultiBlock().getStructure()) {
            if (material == null) {
                layout.add(JsonNull.INSTANCE);
            } else {
                layout.add(material.getKey().toString());
            }
        }
        record.add("layout", layout);

        // Keep registration order, empty slots, and repeated inputs. These affect the selected recipe.
        List<ItemStack[]> entries = machine.getRecipes();
        if (entries.size() % 2 != 0) {
            throw new IllegalStateException("Unpaired multiblock recipes for " + machine.getId());
        }
        JsonArray recipes = new JsonArray();
        for (int i = 0; i < entries.size(); i += 2) {
            JsonObject row = new JsonObject();
            row.add("inputs", describe(entries.get(i)));
            row.add("outputs", describe(entries.get(i + 1)));
            recipes.add(row);
        }
        record.add("recipes", recipes);
        return record;
    }

    @SuppressWarnings("deprecation")
    private static JsonObject describe(Research research) {
        JsonObject record = new JsonObject();
        record.addProperty("id", research.getKey().toString());
        record.addProperty("legacy_id", research.getID());
        record.addProperty("name", research.getUnlocalizedName());
        record.addProperty("cost", research.getCost());
        record.addProperty("enabled", research.isEnabled());
        return record;
    }

    private static JsonArray describe(ItemStack[] items) {
        JsonArray array = new JsonArray();
        if (items != null) {
            for (ItemStack item : items) {
                array.add(describe(item));
            }
        }
        return array;
    }

    private static JsonElement describe(ItemStack stack) {
        if (stack == null || stack.isEmpty()) {
            return JsonNull.INSTANCE;
        }
        JsonObject record = new JsonObject();
        record.addProperty("material", stack.getType().getKey().toString());
        record.addProperty("count", stack.getAmount());
        SlimefunItem identified = SlimefunItem.getByItem(stack);
        if (identified != null) {
            record.addProperty("slimefun_id", identified.getId());
        }
        record.addProperty("paper_nbt_gzip_base64", Base64.getEncoder().encodeToString(stack.serializeAsBytes()));
        return record;
    }
}
