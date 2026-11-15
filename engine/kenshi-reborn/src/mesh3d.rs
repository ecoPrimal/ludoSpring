// mesh3d.rs — 3D mesh loader and renderer for Ogre v1.100
// Extracts vertex positions + indices, uploads to GPU, renders in 3D.

use std::fs;
use std::io::{Read, Seek, SeekFrom};

pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub name: String,
}

fn read_u16(f: &mut fs::File) -> std::io::Result<u16> {
    let mut b = [0u8; 2]; f.read_exact(&mut b)?; Ok(u16::from_le_bytes(b))
}
fn read_u32(f: &mut fs::File) -> std::io::Result<u32> {
    let mut b = [0u8; 4]; f.read_exact(&mut b)?; Ok(u32::from_le_bytes(b))
}
fn read_f32(f: &mut fs::File) -> std::io::Result<f32> {
    let mut b = [0u8; 4]; f.read_exact(&mut b)?; Ok(f32::from_le_bytes(b))
}
fn read_line_string(f: &mut fs::File) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop { f.read_exact(&mut byte)?; if byte[0] == b'\n' { break; } bytes.push(byte[0]); }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

struct VertexElement {
    semantic: u16,
    offset: u16,
    etype: u16,
}

fn parse_geometry(f: &mut fs::File, geo_data_len: u64) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    let geo_start = f.stream_position().unwrap();
    let geo_end = geo_start + geo_data_len;

    let vert_count = read_u32(f).unwrap_or(0) as usize;
    let mut positions = Vec::with_capacity(vert_count);
    let mut normals = Vec::with_capacity(vert_count);
    let mut elements = Vec::new();
    let mut vertex_size: u16 = 0;

    while f.stream_position().unwrap_or(geo_end) + 6 <= geo_end {
        let chunk_id = match read_u16(f) { Ok(id) => id, Err(_) => break };
        let chunk_len = match read_u32(f) { Ok(l) => l as u64, Err(_) => break };
        if chunk_len < 6 { break; }
        let data_start = f.stream_position().unwrap();
        let data_len = chunk_len - 6;

        match chunk_id {
            0x5100 => {
                // M_GEOMETRY_VERTEX_DECLARATION — contains VERTEX_ELEMENT sub-chunks
                let decl_end = data_start + data_len;
                while f.stream_position().unwrap_or(decl_end) + 6 <= decl_end {
                    let sub_id = match read_u16(f) { Ok(id) => id, Err(_) => break };
                    let sub_len = match read_u32(f) { Ok(l) => l as u64, Err(_) => break };
                    if sub_len < 6 { break; }
                    let sub_data_start = f.stream_position().unwrap();

                    if sub_id == 0x5110 {
                        // M_GEOMETRY_VERTEX_ELEMENT: source(u16) type(u16) semantic(u16) offset(u16) index(u16)
                        let _source = read_u16(f).unwrap_or(0);
                        let etype = read_u16(f).unwrap_or(0);
                        let semantic = read_u16(f).unwrap_or(0);
                        let offset = read_u16(f).unwrap_or(0);
                        let _index = read_u16(f).unwrap_or(0);
                        elements.push(VertexElement { semantic, offset, etype });
                    }

                    let _ = f.seek(SeekFrom::Start(sub_data_start + sub_len - 6));
                }
            }
            0x5200 => {
                // M_GEOMETRY_VERTEX_BUFFER: bind_index(u16) vertex_size(u16) then sub-chunk with data
                let _bind = read_u16(f).unwrap_or(0);
                vertex_size = read_u16(f).unwrap_or(0);

                // Read the VERTEX_BUFFER_DATA sub-chunk
                let buf_end = data_start + data_len;
                if f.stream_position().unwrap_or(buf_end) + 6 <= buf_end {
                    let buf_id = read_u16(f).unwrap_or(0);
                    let buf_len = read_u32(f).unwrap_or(0) as u64;

                    if buf_id == 0x5210 && buf_len >= 6 {
                        let buf_data_len = buf_len - 6;
                        let buf_data_start = f.stream_position().unwrap();

                        // Find position and normal offsets
                        let mut pos_offset: Option<u16> = None;
                        let mut norm_offset: Option<u16> = None;
                        for el in &elements {
                            if el.semantic == 1 { pos_offset = Some(el.offset); } // VES_POSITION
                            if el.semantic == 4 { norm_offset = Some(el.offset); } // VES_NORMAL
                        }

                        if let Some(p_off) = pos_offset {
                            // Read all vertices
                            let stride = vertex_size as u64;
                            for i in 0..vert_count {
                                let vert_start = buf_data_start + (i as u64) * stride;

                                // Position
                                let _ = f.seek(SeekFrom::Start(vert_start + p_off as u64));
                                let x = read_f32(f).unwrap_or(0.0);
                                let y = read_f32(f).unwrap_or(0.0);
                                let z = read_f32(f).unwrap_or(0.0);
                                positions.push([x, y, z]);

                                // Normal
                                if let Some(n_off) = norm_offset {
                                    let _ = f.seek(SeekFrom::Start(vert_start + n_off as u64));
                                    let nx = read_f32(f).unwrap_or(0.0);
                                    let ny = read_f32(f).unwrap_or(0.0);
                                    let nz = read_f32(f).unwrap_or(0.0);
                                    normals.push([nx, ny, nz]);
                                } else {
                                    normals.push([0.0, 1.0, 0.0]);
                                }
                            }
                        }

                        let _ = f.seek(SeekFrom::Start(buf_data_start + buf_data_len));
                    }
                }
            }
            _ => {}
        }

        let _ = f.seek(SeekFrom::Start(data_start + data_len));
    }

    (positions, normals)
}

pub fn load_mesh(path: &str) -> Option<MeshData> {
    let mut f = fs::File::open(path).ok()?;
    let file_name = std::path::Path::new(path)
        .file_stem()?.to_string_lossy().to_string();

    // Header
    let header_id = read_u16(&mut f).ok()?;
    if header_id != 0x1000 { return None; }
    let _version = read_line_string(&mut f).ok()?;

    // Mesh
    let mesh_id = read_u16(&mut f).ok()?;
    if mesh_id != 0x3000 { return None; }
    let mesh_len = read_u32(&mut f).ok()? as u64;
    let mesh_start = f.stream_position().ok()?;
    let mesh_end = mesh_start + mesh_len;

    let mut _skeletal = [0u8; 1];
    f.read_exact(&mut _skeletal).ok()?;

    let mut all_positions = Vec::new();
    let mut all_normals = Vec::new();
    let mut all_indices = Vec::new();
    let mut shared_positions = Vec::new();
    let mut shared_normals = Vec::new();

    while f.stream_position().ok()? < mesh_end {
        let pos = f.stream_position().ok()?;
        if pos + 6 > mesh_end { break; }

        let chunk_id = match read_u16(&mut f) { Ok(id) => id, Err(_) => break };
        let chunk_len = match read_u32(&mut f) { Ok(l) => l as u64, Err(_) => break };
        if chunk_len < 6 { break; }
        let data_len = chunk_len - 6;
        let data_start = f.stream_position().ok()?;

        match chunk_id {
            0x4000 => {
                // M_SUBMESH
                let _mat = read_line_string(&mut f).ok().unwrap_or_default();
                let use_shared = { let mut b = [0u8;1]; f.read_exact(&mut b).ok(); b[0] != 0 };
                let index_count = read_u32(&mut f).ok().unwrap_or(0);
                let is_32bit = { let mut b = [0u8;1]; f.read_exact(&mut b).ok(); b[0] != 0 };

                // Read indices
                let base_vertex = if use_shared { 0 } else { all_positions.len() as u32 };

                for _ in 0..index_count {
                    let idx = if is_32bit {
                        read_u32(&mut f).unwrap_or(0)
                    } else {
                        read_u16(&mut f).unwrap_or(0) as u32
                    };
                    all_indices.push(base_vertex + idx);
                }

                // Parse sub-chunks for dedicated geometry
                let submesh_end = data_start + data_len;
                while f.stream_position().ok().unwrap_or(submesh_end) + 6 <= submesh_end {
                    let sub_id = match read_u16(&mut f) { Ok(id) => id, Err(_) => break };
                    let sub_len = match read_u32(&mut f) { Ok(l) => l as u64, Err(_) => break };
                    if sub_len < 6 { break; }
                    let sub_data_start = f.stream_position().ok().unwrap_or(0);
                    let sub_data_len = sub_len - 6;

                    if sub_id == 0x5000 && !use_shared {
                        let (p, n) = parse_geometry(&mut f, sub_data_len);
                        all_positions.extend_from_slice(&p);
                        all_normals.extend_from_slice(&n);
                    }

                    let _ = f.seek(SeekFrom::Start(sub_data_start + sub_data_len));
                }
            }
            0x5000 => {
                // Shared geometry at mesh level
                let (p, n) = parse_geometry(&mut f, data_len);
                shared_positions = p;
                shared_normals = n;
            }
            _ => {}
        }

        let _ = f.seek(SeekFrom::Start(data_start + data_len));
    }

    // If we have shared geometry, prepend it
    if !shared_positions.is_empty() {
        let mut merged_pos = shared_positions;
        merged_pos.extend_from_slice(&all_positions);
        all_positions = merged_pos;

        let mut merged_norm = shared_normals;
        merged_norm.extend_from_slice(&all_normals);
        all_normals = merged_norm;
    }

    if all_positions.is_empty() { return None; }

    // Diagnostic: verify vertex data
    let max_idx = all_indices.iter().copied().max().unwrap_or(0) as usize;
    let zero_normals = all_normals.iter().filter(|n| n[0] == 0.0 && n[1] == 0.0 && n[2] == 0.0).count();
    println!("  Loaded: {} ({} verts, {} tris)",
        file_name, all_positions.len(), all_indices.len() / 3);
    println!("  Max index: {} (valid: {}), zero normals: {}/{}",
        max_idx, max_idx < all_positions.len(), zero_normals, all_normals.len());
    if !all_positions.is_empty() {
        println!("  Vert[0]: [{:.2}, {:.2}, {:.2}]", all_positions[0][0], all_positions[0][1], all_positions[0][2]);
        let last = all_positions.len() - 1;
        println!("  Vert[{}]: [{:.2}, {:.2}, {:.2}]", last, all_positions[last][0], all_positions[last][1], all_positions[last][2]);
    }
    if all_indices.len() >= 3 {
        println!("  Tri[0]: [{}, {}, {}]", all_indices[0], all_indices[1], all_indices[2]);
    }

    Some(MeshData {
        positions: all_positions,
        normals: all_normals,
        indices: all_indices,
        name: file_name,
    })
}
