#!/usr/bin/env python3
"""Генератор facts.json / block-models.json / block-tags.json из textures/*.png.
Использует только реальные блоки Minecraft 1.0.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TEX_DIR = ROOT / "textures"

# === Суффиксы граней — отбрасываем для группировки ===
SUFFIXES = ["_top", "_side", "_bottom", "_front", "_back", "_inner", "_outer",
            "_stage", "_powered", "_lit", "_open", "_closed", "_off", "_on",
            "_overlay", "_snowed", "_fern", "_curve"]

# === Мусорные имена — полностью игнорируем ===
JUNK_PREFIX = ("sprite_", "unknown_", "fire_placeholder_", "portal_placeholder")
JUNK_NAMES = {"test"}

# === Карта: канонический блок → (top_texture, side_texture, bottom_texture) ===
# Это ЕДИНСТВЕННЫЙ источник правды. Всё, что не в этой карте — не существует.
BLOCK_MAP = {
    # камни и земля
    "stone":              ("stone", "stone", "stone"),
    "cobblestone":        ("cobblestone", "cobblestone", "cobblestone"),
    "mossy_cobblestone":  ("cobblestone_mossy", "cobblestone_mossy", "cobblestone_mossy"),
    "dirt":               ("dirt", "dirt", "dirt"),
    "grass_block":        ("grass_top", "grass_side", "dirt"),
    "sand":               ("sand", "sand", "sand"),
    "gravel":             ("gravel", "gravel", "gravel"),
    "bedrock":            ("bedrock", "bedrock", "bedrock"),
    "clay":               ("clay", "clay", "clay"),
    "obsidian":           ("obsidian", "obsidian", "obsidian"),
    "netherrack":         ("netherrack", "netherrack", "netherrack"),
    "soul_sand":          ("soul_sand", "soul_sand", "soul_sand"),
    "glowstone":          ("glowstone", "glowstone", "glowstone"),
    "end_stone":          ("end_stone", "end_stone", "end_stone"),
    "mycelium":           ("mycelium", "mycelium", "mycelium"),
    "sponge":             ("sponge", "sponge", "sponge"),
    "snow":               ("snow", "snow", "snow"),
    "ice":                ("ice", "ice", "ice"),

    # руды и металлы
    "coal_ore":           ("coal_ore", "coal_ore", "coal_ore"),
    "iron_ore":           ("iron_ore", "iron_ore", "iron_ore"),
    "gold_ore":           ("gold_ore", "gold_ore", "gold_ore"),
    "diamond_ore":        ("diamond_ore", "diamond_ore", "diamond_ore"),
    "redstone_ore":       ("redstone_ore", "redstone_ore", "redstone_ore"),
    "lapis_ore":          ("lapis_ore", "lapis_ore", "lapis_ore"),
    "coal_block":         ("coal_block", "coal_block", "coal_block"),  # если есть
    "iron_block":         ("block_iron", "block_iron", "block_iron"),
    "gold_block":         ("block_gold", "block_gold", "block_gold"),
    "diamond_block":      ("block_diamond", "block_diamond", "block_diamond"),
    "lapis_block":        ("lapis_block", "lapis_block", "lapis_block"),

    # кирпичи и камень
    "bricks":             ("brick", "brick", "brick"),
    "stone_bricks":       ("stonebrick", "stonebrick", "stonebrick"),
    "sandstone":          ("sandstone_top", "sandstone_side", "sandstone_top"),
    "stone_slab":         ("stone_slab_top", "stone_slab_side", "stone_slab_top"),

    # дерево
    "oak_log":            ("log_oak_top", "log_oak", "log_oak_top"),
    "oak_planks":         ("planks_oak", "planks_oak", "planks_oak"),
    "oak_leaves":         ("leaves_oak", "leaves_oak", "leaves_oak"),
    "bookshelf":          ("bookshelf", "bookshelf", "bookshelf"),

    # функциональные
    "glass":              ("glass", "glass", "glass"),
    "crafting_table":     ("crafting_table_top", "crafting_table_side", "crafting_table_top"),
    "furnace":            ("furnace_top", "furnace_side", "furnace_top"),
    "chest":              ("chest_top", "chest_side", "chest_top"),
    "tnt":                ("tnt_top", "tnt_side", "tnt_bottom"),
    "cactus":             ("cactus_top", "cactus_side", "cactus_bottom"),
    "pumpkin":            ("pumpkin_top", "pumpkin_side", "pumpkin_top"),
    "jack_o_lantern":     ("lit_pumpkin", "lit_pumpkin", "lit_pumpkin"),
    "melon":              ("melon_top", "melon_side", "melon_top"),
    "jukebox":            ("jukebox_top", "jukebox_top", "jukebox_top"),
    "noteblock":          ("noteblock", "noteblock", "noteblock"),
    "dispenser":          ("dispenser_front", "dispenser_front", "dispenser_front"),
    "spawner":            ("mob_spawner", "mob_spawner", "mob_spawner"),
    "dragon_egg":         ("dragon_egg", "dragon_egg", "dragon_egg"),

    # растения
    "tall_grass":         ("tallgrass", "tallgrass", "tallgrass"),
    "fern":               ("tallgrass_fern", "tallgrass_fern", "tallgrass_fern"),
    "dead_bush":          ("deadbush", "deadbush", "deadbush"),
    "dandelion":          ("flower_dandelion", "flower_dandelion", "flower_dandelion"),
    "poppy":              ("flower_rose", "flower_rose", "flower_rose"),
    "brown_mushroom":     ("mushroom_brown", "mushroom_brown", "mushroom_brown"),
    "red_mushroom":       ("mushroom_red", "mushroom_red", "mushroom_red"),
    "sugar_cane":         ("reeds", "reeds", "reeds"),
    "lily_pad":           ("waterlily", "waterlily", "waterlily"),
    "oak_sapling":        ("sapling_oak", "sapling_oak", "sapling_oak"),
    "spruce_sapling":     ("sapling_spruce", "sapling_spruce", "sapling_spruce"),
    "jungle_sapling":     ("sapling_jungle", "sapling_jungle", "sapling_jungle"),

    # редстоун
    "redstone_wire":      ("redstone_dust", "redstone_dust", "redstone_dust"),
    "redstone_torch":     ("redstone_torch_on", "redstone_torch_on", "redstone_torch_on"),
    "redstone_torch_off": ("redstone_torch_off", "redstone_torch_off", "redstone_torch_off"),
    "lever":              ("lever", "lever", "lever"),
    "repeater":           ("repeater_on", "repeater_on", "repeater_on"),
    "repeater_off":       ("repeater_off", "repeater_off", "repeater_off"),
    "rail":               ("rail", "rail", "rail"),
    "powered_rail":       ("rail", "rail", "rail"),  # в 1.0 нет отдельной текстуры
    "torch":              ("torch", "torch", "torch"),
    "trapdoor":           ("trapdoor", "trapdoor", "trapdoor"),
    "fence":              ("planks_oak", "planks_oak", "planks_oak"),

    # шерсть (все 16 цветов, но у нас в textures только white — остальные будут white)
    "white_wool":         ("white_wool", "white_wool", "white_wool"),
}

def available(name: str) -> bool:
    return (TEX_DIR / f"{name}.png").exists()

def pick(candidates: list[str]) -> str | None:
    for c in candidates:
        if available(c):
            return c
    return None

def main():
    # Фильтруем BLOCK_MAP: только те блоки, у которых есть все текстуры
    real_blocks = {}
    for name, (top, side, bottom) in BLOCK_MAP.items():
        t = pick([top]) or pick([name])
        s = pick([side]) or pick([name])
        b = pick([bottom]) or pick([name])
        if t and s and b:
            real_blocks[name] = {"top": t, "side": s, "bottom": b}
        else:
            print(f"[Gen] пропуск {name}: нет текстур ({t}/{s}/{b})")

    # facts.json
    states, flags, defaults = [], [], {}
    states.append(["minecraft:air", True, False, "", "minecraft:air"])
    flags.append([True, False])
    defaults["minecraft:air"] = 0
    for i, name in enumerate(sorted(real_blocks.keys()), start=1):
        full = f"minecraft:{name}"
        states.append([full, False, False, "", full])
        flags.append([False, False])
        defaults[full] = i
    (ROOT / "facts.json").write_text(
        json.dumps({"states": states, "defaults": defaults, "flags": flags},
                   separators=(",", ":")),
        encoding="utf-8")
    print(f"[Gen] facts.json: {len(states)} блоков")

    # block-models.json
    (ROOT / "block-models.json").write_text(
        json.dumps(real_blocks, indent=2), encoding="utf-8")
    print(f"[Gen] block-models.json: {len(real_blocks)} моделей")

    # horizon-resources.json
    (ROOT / "horizon-resources.json").write_text(
        json.dumps({"data_version": 3465}), encoding="utf-8")
    print("[Gen] horizon-resources.json")

    # block-tags.json (только то, что есть в real_blocks)
    present = set(real_blocks)
    def tag(names): return [f"minecraft:{n}" for n in names if n in present]
    tags = {
        "minecraft:blocks_motion_in_heightmap": {
            "values": tag(["stone","dirt","grass_block","sand","gravel",
                           "coal_ore","iron_ore","gold_ore","diamond_ore",
                           "cobblestone","mossy_cobblestone","clay","bedrock"])
        },
        "minecraft:blocks_motion_in_heightmap_no_leaves": {
            "values": tag(["stone","dirt","grass_block","sand","gravel",
                           "cobblestone","mossy_cobblestone","clay","bedrock"])
        },
        "minecraft:blocks_fluid_flow": {"values": tag(["water","lava"])},
    }
    (ROOT / "block-tags.json").write_text(
        json.dumps(tags, indent=2), encoding="utf-8")
    print("[Gen] block-tags.json")

if __name__ == "__main__":
    main()