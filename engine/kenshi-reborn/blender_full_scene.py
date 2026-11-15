"""
KenshiReborn FULL SCENE — Snowglobe + 21 Creatures
One script to rule them all. Cycles/OptiX on RTX 5090.
"""

import bpy
import bmesh
import math
import random
import os

OBJ_DIR = r"C:\Users\mokke\Desktop\ludoSpring\engine\kenshi-reborn\obj_export"
BLEND_OUT = r"C:\Users\mokke\Desktop\ludoSpring\engine\kenshi-reborn\kenshi_snowglobe.blend"

# ════════════════════════════════════════
# CLEAR + SETUP
# ════════════════════════════════════════
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=True)
for block in bpy.data.meshes:
    if block.users == 0:
        bpy.data.meshes.remove(block)

bpy.context.scene.render.engine = 'CYCLES'
prefs = bpy.context.preferences.addons['cycles'].preferences
prefs.compute_device_type = 'OPTIX'
prefs.get_devices()
for d in prefs.devices:
    d.use = True
bpy.context.scene.cycles.device = 'GPU'
bpy.context.scene.cycles.samples = 128
bpy.context.scene.render.resolution_x = 1920
bpy.context.scene.render.resolution_y = 1080
print("Cycles + OptiX READY")

# ════════════════════════════════════════
# GLASS GLOBE
# ════════════════════════════════════════
bpy.ops.mesh.primitive_uv_sphere_add(radius=5, segments=64, ring_count=32, location=(0, 0, 3.5))
globe = bpy.context.active_object
globe.name = "SnowGlobe"
glass_mat = bpy.data.materials.new(name="GlobGlass")
glass_mat.use_nodes = True
nodes = glass_mat.node_tree.nodes
links = glass_mat.node_tree.links
nodes.clear()
glass_bsdf = nodes.new('ShaderNodeBsdfGlass')
glass_bsdf.inputs['Color'].default_value = (0.95, 0.97, 1.0, 1.0)
glass_bsdf.inputs['Roughness'].default_value = 0.02
glass_bsdf.inputs['IOR'].default_value = 1.45
output = nodes.new('ShaderNodeOutputMaterial')
links.new(glass_bsdf.outputs['BSDF'], output.inputs['Surface'])
globe.data.materials.append(glass_mat)

# ════════════════════════════════════════
# WOODEN BASE
# ════════════════════════════════════════
bpy.ops.mesh.primitive_cylinder_add(radius=4, depth=1.5, location=(0, 0, -1.5))
base = bpy.context.active_object
base.name = "GlobeBase"
wood_mat = bpy.data.materials.new(name="Wood")
wood_mat.use_nodes = True
wnodes = wood_mat.node_tree.nodes
wlinks = wood_mat.node_tree.links
wnodes.clear()
principled = wnodes.new('ShaderNodeBsdfPrincipled')
principled.inputs['Base Color'].default_value = (0.25, 0.15, 0.08, 1.0)
principled.inputs['Roughness'].default_value = 0.7
woutput = wnodes.new('ShaderNodeOutputMaterial')
wlinks.new(principled.outputs['BSDF'], woutput.inputs['Surface'])
base.data.materials.append(wood_mat)

# ════════════════════════════════════════
# KENSHI DESERT TERRAIN
# ════════════════════════════════════════
bpy.ops.mesh.primitive_plane_add(size=8, location=(0, 0, 0.3))
terrain = bpy.context.active_object
terrain.name = "KenshiTerrain"
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.subdivide(number_cuts=20)
bpy.ops.object.mode_set(mode='OBJECT')
for v in terrain.data.vertices:
    v.co.z += random.uniform(0, 0.2) * math.sin(v.co.x * 1.5) * math.cos(v.co.y * 1.2)
sand_mat = bpy.data.materials.new(name="KenshiSand")
sand_mat.use_nodes = True
snodes = sand_mat.node_tree.nodes
slinks = sand_mat.node_tree.links
snodes.clear()
sand_bsdf = snodes.new('ShaderNodeBsdfPrincipled')
sand_bsdf.inputs['Base Color'].default_value = (0.72, 0.56, 0.32, 1.0)
sand_bsdf.inputs['Roughness'].default_value = 0.9
soutput = snodes.new('ShaderNodeOutputMaterial')
slinks.new(sand_bsdf.outputs['BSDF'], soutput.inputs['Surface'])
terrain.data.materials.append(sand_mat)

# ════════════════════════════════════════
# GATE NODES (glowing spheres)
# ════════════════════════════════════════
gate_data = [
    ("eastGate", (-2, 0.5, 5), (0, 1, 0), 30),
    ("northGate", (0, -3, 7), (1, 1, 0), 40),
    ("sporeGate", (2.5, 1, 2), (0.75, 0.75, 0.75), 20),
    ("ironGate", (-2.5, 2, 3), (0.5, 0.5, 0.5), 20),
    ("grapheneGate", (1, -1, 4), (0, 0, 1), 25),
    ("frameGate", (0, 0, 3.5), (0.63, 0.13, 0.94), 25),
    ("deckGate", (1.5, -2, 2.5), (1, 0.27, 0.27), 15),
]
for name, loc, color, strength in gate_data:
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.12, location=loc)
    g = bpy.context.active_object
    g.name = name
    emit_mat = bpy.data.materials.new(name=f"Gate_{name}")
    emit_mat.use_nodes = True
    en = emit_mat.node_tree.nodes
    el = emit_mat.node_tree.links
    en.clear()
    emission = en.new('ShaderNodeEmission')
    emission.inputs['Color'].default_value = (*color, 1.0)
    emission.inputs['Strength'].default_value = strength
    eout = en.new('ShaderNodeOutputMaterial')
    el.new(emission.outputs['Emission'], eout.inputs['Surface'])
    g.data.materials.append(emit_mat)
print(f"{len(gate_data)} gates glowing")

# ════════════════════════════════════════
# SNOW PARTICLES
# ════════════════════════════════════════
bpy.ops.mesh.primitive_uv_sphere_add(radius=4.5, location=(0, 0, 3.5))
emitter = bpy.context.active_object
emitter.name = "SnowEmitter"
emitter.hide_render = True
bpy.ops.mesh.primitive_ico_sphere_add(radius=0.03, location=(20, 20, 20))
snowflake = bpy.context.active_object
snowflake.name = "Snowflake"
snow_mat = bpy.data.materials.new(name="Snow")
snow_mat.use_nodes = True
snow_mat.node_tree.nodes["Principled BSDF"].inputs['Base Color'].default_value = (1, 1, 1, 1)
snowflake.data.materials.append(snow_mat)
ps = emitter.modifiers.new("Snow", type='PARTICLE_SYSTEM')
pss = ps.particle_system.settings
pss.count = 3000
pss.lifetime = 250
pss.emit_from = 'VOLUME'
pss.physics_type = 'NEWTON'
pss.normal_factor = -0.4
pss.factor_random = 0.6
pss.render_type = 'OBJECT'
pss.instance_object = snowflake
print("3000 snowflakes")

# ════════════════════════════════════════
# INJECT CREATURES
# ════════════════════════════════════════
CREATURES = [
    ("animals", "bull.obj", (1.0, 1.5, 0.5), 0.6, (0.6, 0.4, 0.2), "Bull"),
    ("animals", "Crab.obj", (-1.5, -0.5, 0.5), 0.5, (0.8, 0.15, 0.1), "Crab"),
    ("animals", "dog.obj", (2.0, -1.0, 0.5), 0.5, (0.5, 0.35, 0.2), "Bonedog"),
    ("animals", "Crimper.obj", (-3.0, 2.0, 0.7), 0.8, (0.2, 0.5, 0.2), "Crimper"),
    ("animals", "antilop250.obj", (0.5, -2.5, 0.5), 0.5, (0.7, 0.6, 0.4), "Antilop"),
    ("animals", "elephant_turtle.obj", (3.0, 0.5, 0.5), 0.6, (0.4, 0.5, 0.3), "ElephantTurtle"),
    ("animals", "skimmer.obj", (-1.0, 3.0, 2.0), 0.5, (0.3, 0.3, 0.6), "Skimmer"),
    ("animals", "cage_beast.obj", (1.5, 3.0, 0.5), 0.7, (0.5, 0.3, 0.5), "CageBeast"),
    ("animals", "land_bat.obj", (-2.0, -3.0, 2.5), 0.5, (0.3, 0.2, 0.15), "LandBat"),
    ("animals", "frog_gorilla.obj", (2.5, -2.0, 0.5), 0.6, (0.35, 0.5, 0.25), "Gorillo"),
    ("animals", "whale2.obj", (0.0, 0.0, 5.0), 1.5, (0.15, 0.2, 0.4), "Leviathan"),
    ("animals", "pack_beast.obj", (-2.5, -1.5, 0.5), 0.7, (0.55, 0.45, 0.3), "PackBeast"),
    ("animals", "flesh_insect.obj", (1.0, -1.0, 1.5), 0.4, (0.7, 0.2, 0.2), "SkinSpider"),
    ("animals", "swamp_duck.obj", (-0.5, 0.5, 1.2), 0.5, (0.3, 0.4, 0.3), "SwampTurtle"),
    ("animals", "robobird.obj", (0.0, 2.0, 3.5), 0.4, (0.4, 0.4, 0.45), "SecuritySpider"),
    ("animals", "Big-Bones.obj", (-3.0, -2.0, 0.5), 1.0, (0.8, 0.75, 0.65), "BigBones"),
    ("animals", "goat.obj", (3.5, -1.5, 0.5), 0.4, (0.6, 0.5, 0.4), "Goat"),
    ("animals", "robot_worker.obj", (2.0, 2.5, 0.5), 0.5, (0.45, 0.45, 0.5), "RobotWorker"),
    ("characters", "human_male.obj", (-0.3, -0.3, 0.4), 0.4, (0.6, 0.5, 0.4), "HumanMale"),
    ("characters", "Skeleton_MkII.obj", (0.3, 0.3, 0.4), 0.4, (0.5, 0.5, 0.55), "SkeletonMkII"),
    ("characters", "Fishman.obj", (-1.0, 1.0, 0.4), 0.4, (0.4, 0.6, 0.5), "Fishman"),
]

loaded = 0
for subdir, filename, loc, target_size, color, label in CREATURES:
    obj_path = os.path.join(OBJ_DIR, subdir, filename)
    if not os.path.exists(obj_path):
        print(f"  SKIP {label}")
        continue

    bpy.ops.wm.obj_import(filepath=obj_path)
    obj = bpy.context.selected_objects[0] if bpy.context.selected_objects else None
    if not obj:
        continue

    obj.name = label
    dims = obj.dimensions
    max_dim = max(dims.x, dims.y, dims.z)
    if max_dim > 0:
        s = target_size / max_dim
        obj.scale = (s, s, s)
    obj.location = loc

    mat = bpy.data.materials.new(name=f"Mat_{label}")
    mat.use_nodes = True
    bsdf = mat.node_tree.nodes.get("Principled BSDF")
    if bsdf:
        bsdf.inputs['Base Color'].default_value = (*color, 1.0)
        bsdf.inputs['Roughness'].default_value = 0.6
        bsdf.inputs['Metallic'].default_value = 0.1
    obj.data.materials.clear()
    obj.data.materials.append(mat)
    loaded += 1
    print(f"  [{loaded}] {label}: {len(obj.data.vertices)} verts")

print(f"\n{loaded} creatures injected into snowglobe!")

# ════════════════════════════════════════
# CAMERA — GOD VIEW
# ════════════════════════════════════════
bpy.ops.object.camera_add(location=(0, -10, 14))
cam = bpy.context.active_object
cam.name = "GodView"
cam.rotation_euler = (math.radians(35), 0, 0)
cam.data.lens = 50
bpy.context.scene.camera = cam

# ════════════════════════════════════════
# LIGHTING
# ════════════════════════════════════════
bpy.ops.object.light_add(type='AREA', location=(5, -5, 10))
key = bpy.context.active_object
key.name = "KeyLight"
key.data.energy = 800
key.data.color = (1.0, 0.9, 0.8)
key.data.size = 5

bpy.ops.object.light_add(type='AREA', location=(-5, 3, 7))
fill = bpy.context.active_object
fill.name = "FillLight"
fill.data.energy = 300
fill.data.color = (0.7, 0.8, 1.0)
fill.data.size = 6

world = bpy.data.worlds['World']
world.use_nodes = True
bg = world.node_tree.nodes['Background']
bg.inputs['Color'].default_value = (0.01, 0.01, 0.02, 1.0)
bg.inputs['Strength'].default_value = 0.3

# ════════════════════════════════════════
# SAVE
# ════════════════════════════════════════
bpy.ops.wm.save_as_mainfile(filepath=BLEND_OUT)
print(f"\nSaved: {BLEND_OUT}")
print("FULL KENSHI SNOWGLOBE: globe + terrain + gates + snow + creatures!")
print("Open in Blender -> F12 to render on RTX 5090!")
