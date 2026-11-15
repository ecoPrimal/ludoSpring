"""
Inject Kenshi creatures + characters + weapons into the Blender snowglobe
Import OBJ files, scale them, place inside the globe, set materials
"""

import bpy
import os
import math
import random

OBJ_DIR = r"C:\Users\mokke\Desktop\ludoSpring\engine\kenshi-reborn\obj_export"

ICONIC_CREATURES = [
    ("animals", "bull.obj", (0.5, 0.8, 0.6), "Bull"),
    ("animals", "Crab.obj", (-0.8, -0.3, 0.5), "Crab"),
    ("animals", "dog.obj", (1.0, -0.5, 0.5), "Bonedog"),
    ("animals", "Crimper.obj", (-1.5, 1.0, 0.8), "Crimper"),
    ("animals", "antilop250.obj", (0.3, -1.2, 0.5), "Antilop"),
    ("animals", "elephant_turtle.obj", (1.5, 0.5, 0.5), "ElephantTurtle"),
    ("animals", "skimmer.obj", (-0.5, 1.5, 1.2), "Skimmer"),
    ("animals", "cage_beast.obj", (0.8, 1.5, 0.5), "CageBeast"),
    ("animals", "land_bat.obj", (-1.0, -1.5, 1.5), "LandBat"),
    ("animals", "frog_gorilla.obj", (1.3, -1.0, 0.5), "Gorillo"),
    ("animals", "whale2.obj", (0.0, 0.0, 2.5), "Leviathan"),
    ("animals", "pack_beast.obj", (-1.3, -0.8, 0.5), "PackBeast"),
    ("animals", "flesh_insect.obj", (0.5, -0.5, 1.0), "SkinSpider"),
    ("animals", "swamp_duck.obj", (-0.3, 0.3, 0.8), "SwampTurtle"),
    ("animals", "robobird.obj", (0.0, 1.0, 2.0), "SecuritySpider"),
    ("characters", "human_male.obj", (-0.2, -0.2, 0.4), "HumanMale"),
    ("characters", "Skeleton_MkII.obj", (0.2, 0.2, 0.4), "SkeletonMkII"),
    ("characters", "Fishman.obj", (-0.5, 0.5, 0.4), "Fishman"),
    ("characters", "Hive_Queen.obj", (0.5, -0.8, 0.4), "HiveQueen"),
    ("weapons", "katana04FX.obj", (0.0, -0.5, 0.8), "Katana"),
    ("weapons", "sabre01.obj", (0.3, 0.5, 0.8), "Sabre"),
]

CREATURE_COLORS = [
    (0.6, 0.4, 0.2),   # Brown (bull)
    (0.8, 0.15, 0.1),  # Red (crab)
    (0.5, 0.35, 0.2),  # Tan (bonedog)
    (0.2, 0.5, 0.2),   # Green (crimper)
    (0.7, 0.6, 0.4),   # Sandy (antilop)
    (0.4, 0.5, 0.3),   # Olive (elephant turtle)
    (0.3, 0.3, 0.6),   # Blue (skimmer)
    (0.5, 0.3, 0.5),   # Purple (cage beast)
    (0.3, 0.2, 0.15),  # Dark brown (land bat)
    (0.35, 0.5, 0.25), # Moss (gorillo)
    (0.15, 0.2, 0.4),  # Deep blue (leviathan)
    (0.55, 0.45, 0.3), # Beige (pack beast)
    (0.7, 0.2, 0.2),   # Crimson (skin spider)
    (0.3, 0.4, 0.3),   # Swamp green (swamp turtle)
    (0.4, 0.4, 0.45),  # Metal (security spider)
    (0.6, 0.5, 0.4),   # Skin tone (human)
    (0.5, 0.5, 0.55),  # Steel (skeleton)
    (0.4, 0.6, 0.5),   # Aqua (fishman)
    (0.6, 0.3, 0.5),   # Hive purple (queen)
    (0.7, 0.7, 0.75),  # Silver (katana)
    (0.65, 0.6, 0.55), # Blade (sabre)
]

loaded = 0
failed = 0

for i, (subdir, filename, location, label) in enumerate(ICONIC_CREATURES):
    obj_path = os.path.join(OBJ_DIR, subdir, filename)
    if not os.path.exists(obj_path):
        print(f"  SKIP {label}: {obj_path} not found")
        failed += 1
        continue

    bpy.ops.wm.obj_import(filepath=obj_path)

    obj = bpy.context.selected_objects[0] if bpy.context.selected_objects else None
    if not obj:
        print(f"  SKIP {label}: no object imported")
        failed += 1
        continue

    obj.name = label

    dims = obj.dimensions
    max_dim = max(dims.x, dims.y, dims.z)
    if max_dim > 0:
        if label == "Leviathan":
            scale = 1.5 / max_dim
        elif label in ("Crimper", "CageBeast", "PackBeast"):
            scale = 0.8 / max_dim
        elif label in ("Katana", "Sabre"):
            scale = 0.3 / max_dim
        elif label in ("HumanMale", "SkeletonMkII", "Fishman", "HiveQueen"):
            scale = 0.4 / max_dim
        else:
            scale = 0.5 / max_dim
        obj.scale = (scale, scale, scale)

    obj.location = location

    color = CREATURE_COLORS[i] if i < len(CREATURE_COLORS) else (0.5, 0.5, 0.5)
    mat = bpy.data.materials.new(name=f"Mat_{label}")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs['Base Color'].default_value = (*color, 1.0)
        bsdf.inputs['Roughness'].default_value = 0.6
    obj.data.materials.clear()
    obj.data.materials.append(mat)

    loaded += 1
    verts = len(obj.data.vertices)
    print(f"  [{loaded}] {label}: {verts} verts at {location}")

print(f"\nLoaded {loaded} creatures, {failed} skipped")
print("Press F12 to render the FULL BESTIARY snowglobe!")
