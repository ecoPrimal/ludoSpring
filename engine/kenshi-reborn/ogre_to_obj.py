"""
ogre_to_obj.py — Convert Kenshi Ogre .mesh files to Wavefront .obj
Mirrors mesh3d.rs parsing logic exactly.

Usage:
  python ogre_to_obj.py <input.mesh> [output.obj]
  python ogre_to_obj.py --batch <mesh_dir> <output_dir>
"""

import struct
import sys
import os

def read_u16(f):
    d = f.read(2)
    if len(d) < 2: raise EOFError
    return struct.unpack('<H', d)[0]

def read_u32(f):
    d = f.read(4)
    if len(d) < 4: raise EOFError
    return struct.unpack('<I', d)[0]

def read_f32(f):
    d = f.read(4)
    if len(d) < 4: raise EOFError
    return struct.unpack('<f', d)[0]

def read_line_string(f):
    chars = []
    while True:
        b = f.read(1)
        if not b or b == b'\n':
            break
        chars.append(b)
    return b''.join(chars).decode('utf-8', errors='replace')

def parse_geometry(f, geo_data_len):
    geo_start = f.tell()
    geo_end = geo_start + geo_data_len

    vert_count = read_u32(f)
    positions = []
    normals = []
    elements = []
    vertex_size = 0

    while f.tell() + 6 <= geo_end:
        chunk_id = read_u16(f)
        chunk_len = read_u32(f)
        if chunk_len < 6:
            break
        data_start = f.tell()
        data_len = chunk_len - 6

        if chunk_id == 0x5100:
            decl_end = data_start + data_len
            while f.tell() + 6 <= decl_end:
                sub_id = read_u16(f)
                sub_len = read_u32(f)
                if sub_len < 6:
                    break
                sub_data_start = f.tell()
                if sub_id == 0x5110:
                    _source = read_u16(f)
                    etype = read_u16(f)
                    semantic = read_u16(f)
                    offset = read_u16(f)
                    _index = read_u16(f)
                    elements.append({'semantic': semantic, 'offset': offset, 'etype': etype})
                f.seek(sub_data_start + sub_len - 6)

        elif chunk_id == 0x5200:
            _bind = read_u16(f)
            vertex_size = read_u16(f)
            buf_end = data_start + data_len
            if f.tell() + 6 <= buf_end:
                buf_id = read_u16(f)
                buf_len = read_u32(f)
                if buf_id == 0x5210 and buf_len >= 6:
                    buf_data_start = f.tell()
                    pos_offset = None
                    norm_offset = None
                    for el in elements:
                        if el['semantic'] == 1: pos_offset = el['offset']
                        if el['semantic'] == 4: norm_offset = el['offset']

                    if pos_offset is not None:
                        stride = vertex_size
                        for i in range(vert_count):
                            vert_start = buf_data_start + i * stride
                            f.seek(vert_start + pos_offset)
                            x, y, z = read_f32(f), read_f32(f), read_f32(f)
                            positions.append((x, y, z))
                            if norm_offset is not None:
                                f.seek(vert_start + norm_offset)
                                nx, ny, nz = read_f32(f), read_f32(f), read_f32(f)
                                normals.append((nx, ny, nz))
                            else:
                                normals.append((0, 1, 0))

        f.seek(data_start + data_len)

    return positions, normals

def load_ogre_mesh(path):
  try:
    with open(path, 'rb') as f:
        header_id = read_u16(f)
        if header_id != 0x1000:
            return None
        _version = read_line_string(f)

        mesh_id = read_u16(f)
        if mesh_id != 0x3000:
            return None
        mesh_len = read_u32(f)
        mesh_start = f.tell()
        mesh_end = mesh_start + mesh_len

        _skeletal = f.read(1)

        all_positions = []
        all_normals = []
        all_indices = []
        shared_positions = []
        shared_normals = []

        while f.tell() < mesh_end:
            pos = f.tell()
            if pos + 6 > mesh_end:
                break
            try:
                chunk_id = read_u16(f)
                chunk_len = read_u32(f)
            except EOFError:
                break
            if chunk_len < 6:
                break
            data_len = chunk_len - 6
            data_start = f.tell()

            if chunk_id == 0x4000:
                _mat = read_line_string(f)
                use_shared = struct.unpack('B', f.read(1))[0] != 0
                index_count = read_u32(f)
                is_32bit = struct.unpack('B', f.read(1))[0] != 0

                base_vertex = 0 if use_shared else len(all_positions)

                for _ in range(index_count):
                    if is_32bit:
                        idx = read_u32(f)
                    else:
                        idx = read_u16(f)
                    all_indices.append(base_vertex + idx)

                submesh_end = data_start + data_len
                while f.tell() + 6 <= submesh_end:
                    sub_id = read_u16(f)
                    sub_len = read_u32(f)
                    if sub_len < 6:
                        break
                    sub_data_start = f.tell()
                    sub_data_len = sub_len - 6
                    if sub_id == 0x5000 and not use_shared:
                        p, n = parse_geometry(f, sub_data_len)
                        all_positions.extend(p)
                        all_normals.extend(n)
                    f.seek(sub_data_start + sub_data_len)

            elif chunk_id == 0x5000:
                p, n = parse_geometry(f, data_len)
                shared_positions = p
                shared_normals = n

            f.seek(data_start + data_len)

        if shared_positions:
            all_positions = shared_positions + all_positions
            all_normals = shared_normals + all_normals

        if not all_positions:
            return None

        name = os.path.splitext(os.path.basename(path))[0]
        return {
            'name': name,
            'positions': all_positions,
            'normals': all_normals,
            'indices': all_indices,
        }
  except (EOFError, struct.error, Exception) as e:
        return None

def write_obj(mesh_data, out_path):
    with open(out_path, 'w') as f:
        f.write(f"# Kenshi Ogre .mesh to OBJ by Gaia\n")
        f.write(f"# {mesh_data['name']}: {len(mesh_data['positions'])} verts, {len(mesh_data['indices'])//3} tris\n")
        f.write(f"o {mesh_data['name']}\n\n")

        for x, y, z in mesh_data['positions']:
            f.write(f"v {x:.6f} {y:.6f} {z:.6f}\n")
        f.write("\n")

        for nx, ny, nz in mesh_data['normals']:
            f.write(f"vn {nx:.6f} {ny:.6f} {nz:.6f}\n")
        f.write("\n")

        indices = mesh_data['indices']
        for i in range(0, len(indices), 3):
            if i + 2 < len(indices):
                a, b, c = indices[i]+1, indices[i+1]+1, indices[i+2]+1
                f.write(f"f {a}//{a} {b}//{b} {c}//{c}\n")

    print(f"  OK {mesh_data['name']}: {len(mesh_data['positions'])} verts -> {out_path}")

def batch_convert(mesh_dir, out_dir):
    os.makedirs(out_dir, exist_ok=True)
    count = 0
    for fname in sorted(os.listdir(mesh_dir)):
        if not fname.lower().endswith('.mesh'):
            continue
        mesh_path = os.path.join(mesh_dir, fname)
        data = load_ogre_mesh(mesh_path)
        if data:
            obj_name = os.path.splitext(fname)[0] + '.obj'
            write_obj(data, os.path.join(out_dir, obj_name))
            count += 1
        else:
            print(f"  SKIP {fname}: failed to parse")
    return count

if __name__ == '__main__':
    if len(sys.argv) < 2:
        print("Usage: python ogre_to_obj.py <input.mesh> [output.obj]")
        print("       python ogre_to_obj.py --batch <mesh_dir> <output_dir>")
        sys.exit(1)

    if sys.argv[1] == '--batch':
        mesh_dir = sys.argv[2]
        out_dir = sys.argv[3] if len(sys.argv) > 3 else 'obj_export'
        print("=== OGRE -> OBJ BATCH CONVERTER ===")
        n = batch_convert(mesh_dir, out_dir)
        print(f"\n{n} meshes converted!")
    else:
        mesh_path = sys.argv[1]
        out_path = sys.argv[2] if len(sys.argv) > 2 else mesh_path.replace('.mesh', '.obj')
        data = load_ogre_mesh(mesh_path)
        if data:
            write_obj(data, out_path)
        else:
            print(f"Failed to parse {mesh_path}")
