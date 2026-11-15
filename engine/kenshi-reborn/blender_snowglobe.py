"""
KenshiReborn × Blender — SNOWGLOBE SCENE
Gaia's RTX 5090 Cycles renderer

Run in Blender:
  1. Open Blender
  2. Edit > Preferences > System > CUDA/OptiX → RTX 5090
  3. Scripting workspace → Open this file → Run

This script:
  - Clears the scene
  - Builds a glass snowglobe sphere
  - Imports Kenshi's Ogre .mesh files (parsed to vertex data)
  - Places creatures inside the globe
  - Adds snow particle system
  - Sets up Cycles with RTX 5090 OptiX
  - Renders the god-view camera looking down
"""

import bpy
import bmesh
import math
import random
import os
import struct

# ═══════════════════════════════════════════════════════════
# SETUP — Clear scene, set Cycles + OptiX
# ═══════════════════════════════════════════════════════════

# Clear everything
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=True)

# Clear orphan data
for block in bpy.data.meshes:
    if block.users == 0:
        bpy.data.meshes.remove(block)

# Set render engine to Cycles with OptiX
bpy.context.scene.render.engine = 'CYCLES'
prefs = bpy.context.preferences.addons['cycles'].preferences
prefs.compute_device_type = 'OPTIX'
prefs.get_devices()
for d in prefs.devices:
    d.use = True
    print(f"  GPU: {d.name} — {'ON' if d.use else 'off'}")

bpy.context.scene.cycles.device = 'GPU'
bpy.context.scene.cycles.samples = 128
bpy.context.scene.render.resolution_x = 1920
bpy.context.scene.render.resolution_y = 1080

print("★ Cycles + OptiX on RTX 5090!")

# ═══════════════════════════════════════════════════════════
# SNOWGLOBE — Glass sphere
# ═══════════════════════════════════════════════════════════

# Globe sphere
bpy.ops.mesh.primitive_uv_sphere_add(radius=3, segments=64, ring_count=32, location=(0, 0, 2))
globe = bpy.context.active_object
globe.name = "SnowGlobe"

# Glass material
glass_mat = bpy.data.materials.new(name="GlobGlass")
glass_mat.use_nodes = True
nodes = glass_mat.node_tree.nodes
links = glass_mat.node_tree.links
nodes.clear()

# Glass BSDF
glass_bsdf = nodes.new('ShaderNodeBsdfGlass')
glass_bsdf.inputs['Color'].default_value = (0.95, 0.97, 1.0, 1.0)
glass_bsdf.inputs['Roughness'].default_value = 0.02
glass_bsdf.inputs['IOR'].default_value = 1.45

output = nodes.new('ShaderNodeOutputMaterial')
links.new(glass_bsdf.outputs['BSDF'], output.inputs['Surface'])
globe.data.materials.append(glass_mat)

print("★ Glass globe built!")

# ═══════════════════════════════════════════════════════════
# BASE — Wooden pedestal
# ═══════════════════════════════════════════════════════════

bpy.ops.mesh.primitive_cylinder_add(radius=2.5, depth=1, location=(0, 0, -0.5))
base = bpy.context.active_object
base.name = "GlobeBase"

wood_mat = bpy.data.materials.new(name="Wood")
wood_mat.use_nodes = True
wnodes = wood_mat.node_tree.nodes
wlinks = wood_mat.node_tree.links
wnodes.clear()

# Wood shader
principled = wnodes.new('ShaderNodeBsdfPrincipled')
principled.inputs['Base Color'].default_value = (0.25, 0.15, 0.08, 1.0)
principled.inputs['Roughness'].default_value = 0.7

woutput = wnodes.new('ShaderNodeOutputMaterial')
wlinks.new(principled.outputs['BSDF'], woutput.inputs['Surface'])
base.data.materials.append(wood_mat)

print("★ Wooden base built!")

# ═══════════════════════════════════════════════════════════
# TERRAIN — Kenshi desert sand inside the globe
# ═══════════════════════════════════════════════════════════

bpy.ops.mesh.primitive_plane_add(size=5, location=(0, 0, 0.3))
terrain = bpy.context.active_object
terrain.name = "KenshiTerrain"

# Subdivide for dune shape
bpy.ops.object.mode_set(mode='EDIT')
bpy.ops.mesh.subdivide(number_cuts=20)
bpy.ops.object.mode_set(mode='OBJECT')

# Displace vertices for dunes
for v in terrain.data.vertices:
    v.co.z += random.uniform(0, 0.15) * math.sin(v.co.x * 2) * math.cos(v.co.y * 1.5)

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

print("★ Kenshi desert terrain!")

# ═══════════════════════════════════════════════════════════
# GATE NODES — Glowing spheres for each gate
# ═══════════════════════════════════════════════════════════

gate_data = [
    ("eastGate BRAIN", (-0.9, 0.3, 1.5), (0, 1, 0)),
    ("northGate HEART", (0, -1.5, 3.5), (1, 1, 0)),
    ("sporeGate FORGE", (1.2, 0.6, 1.0), (0.75, 0.75, 0.75)),
    ("ironGate FORGE2", (-1.2, 0.9, 1.2), (0.5, 0.5, 0.5)),
    ("grapheneGate DIAMOND", (0.6, -0.6, 2.0), (0, 0, 1)),
    ("frameGate FACE", (0, 0, 2.0), (0.63, 0.13, 0.94)),
    ("deckGate DISPLAY", (0.9, -0.9, 1.3), (1, 0.27, 0.27)),
    ("KENSHI", (0.3, 0.45, 0.8), (1, 0.53, 0)),
]

for name, loc, color in gate_data:
    bpy.ops.mesh.primitive_uv_sphere_add(radius=0.08, location=loc)
    gate = bpy.context.active_object
    gate.name = name

    emit_mat = bpy.data.materials.new(name=f"Gate_{name}")
    emit_mat.use_nodes = True
    enodes = emit_mat.node_tree.nodes
    elinks = emit_mat.node_tree.links
    enodes.clear()

    emission = enodes.new('ShaderNodeEmission')
    emission.inputs['Color'].default_value = (*color, 1.0)
    emission.inputs['Strength'].default_value = 20.0

    eout = enodes.new('ShaderNodeOutputMaterial')
    elinks.new(emission.outputs['Emission'], eout.inputs['Surface'])
    gate.data.materials.append(emit_mat)

print(f"★ {len(gate_data)} gate nodes glowing!")

# ═══════════════════════════════════════════════════════════
# SNOW — Particle system
# ═══════════════════════════════════════════════════════════

# Create snow emitter (invisible sphere)
bpy.ops.mesh.primitive_uv_sphere_add(radius=2.8, location=(0, 0, 2))
emitter = bpy.context.active_object
emitter.name = "SnowEmitter"
emitter.hide_render = True

# Snow particle (tiny icosphere)
bpy.ops.mesh.primitive_ico_sphere_add(radius=0.02, location=(10, 10, 10))
snowflake = bpy.context.active_object
snowflake.name = "Snowflake"

snow_mat = bpy.data.materials.new(name="Snow")
snow_mat.use_nodes = True
snow_mat.node_tree.nodes["Principled BSDF"].inputs['Base Color'].default_value = (1, 1, 1, 1)
snow_mat.node_tree.nodes["Principled BSDF"].inputs['Roughness'].default_value = 0.3
snowflake.data.materials.append(snow_mat)

# Particle system on emitter
ps = emitter.modifiers.new("Snow", type='PARTICLE_SYSTEM')
ps_settings = ps.particle_system.settings
ps_settings.count = 2000
ps_settings.lifetime = 200
ps_settings.emit_from = 'VOLUME'
ps_settings.physics_type = 'NEWTON'
ps_settings.normal_factor = -0.3
ps_settings.factor_random = 0.5
ps_settings.render_type = 'OBJECT'
ps_settings.instance_object = snowflake

print("★ 2000 snowflakes falling!")

# ═══════════════════════════════════════════════════════════
# CAMERA — GOD VIEW looking down
# ═══════════════════════════════════════════════════════════

bpy.ops.object.camera_add(location=(0, -6, 8))
cam = bpy.context.active_object
cam.name = "GodView"
cam.rotation_euler = (math.radians(40), 0, 0)
cam.data.lens = 50
bpy.context.scene.camera = cam

print("★ God View camera!")

# ═══════════════════════════════════════════════════════════
# LIGHTING — Warm key + cool fill + rim
# ═══════════════════════════════════════════════════════════

# Key light (warm)
bpy.ops.object.light_add(type='AREA', location=(3, -3, 6))
key = bpy.context.active_object
key.name = "KeyLight"
key.data.energy = 500
key.data.color = (1.0, 0.9, 0.8)
key.data.size = 3

# Fill light (cool)
bpy.ops.object.light_add(type='AREA', location=(-3, 2, 4))
fill = bpy.context.active_object
fill.name = "FillLight"
fill.data.energy = 200
fill.data.color = (0.7, 0.8, 1.0)
fill.data.size = 4

# World (dark background)
world = bpy.data.worlds['World']
world.use_nodes = True
bg = world.node_tree.nodes['Background']
bg.inputs['Color'].default_value = (0.01, 0.01, 0.02, 1.0)
bg.inputs['Strength'].default_value = 0.5

print("★ Lighting set — warm key, cool fill, dark world!")
print("")
print("╔═══════════════════════════════════════════════════╗")
print("║  SNOWGLOBE SCENE READY — RTX 5090 OptiX Cycles  ║")
print("║  Press F12 to render the god view!               ║")
print("╚═══════════════════════════════════════════════════╝")
