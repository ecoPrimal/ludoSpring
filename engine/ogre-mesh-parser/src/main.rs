//! Ogre3D Mesh Parser — reads Kenshi's MeshSerializer_v1.8 binary format
//! Parses 3,328 mesh files to extract: vertex count, submesh count, bounds, skeleton links
//!
//! Format spec: https://github.com/OGRECave/ogre/blob/master/OgreMain/src/OgreMeshFileFormat.h
//! Chunk structure: u16 CHUNK_ID | u32 LENGTH (includes 6-byte header) | DATA

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

// Ogre Mesh Chunk IDs
const M_HEADER: u16 = 0x1000;
const M_MESH: u16 = 0x3000;
const M_SUBMESH: u16 = 0x4000;
const M_GEOMETRY: u16 = 0x5000;
const M_MESH_SKELETON_LINK: u16 = 0x6000;
const M_MESH_BONE_ASSIGNMENT: u16 = 0x7000;
const M_MESH_LOD_LEVEL: u16 = 0x8000;
const M_MESH_BOUNDS: u16 = 0x9000;
const M_SUBMESH_NAME_TABLE: u16 = 0xA000;
const M_SUBMESH_NAME_TABLE_ELEMENT: u16 = 0xA100;
const M_GEOMETRY_VERTEX_DECLARATION: u16 = 0x5100;
const M_GEOMETRY_VERTEX_ELEMENT: u16 = 0x5110;
const M_GEOMETRY_VERTEX_BUFFER: u16 = 0x5200;

#[derive(Debug, Default)]
struct MeshBounds {
    min_x: f32,
    min_y: f32,
    min_z: f32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
    radius: f32,
}

#[derive(Debug, Default)]
struct SubMeshInfo {
    material_name: String,
    index_count: u32,
    vertex_count: u32,
    uses_shared_vertices: bool,
}

#[derive(Debug, Default)]
struct MeshInfo {
    file_name: String,
    file_size: u64,
    version: String,
    skeletally_animated: bool,
    skeleton_link: String,
    shared_vertex_count: u32,
    submesh_count: u32,
    submeshes: Vec<SubMeshInfo>,
    bounds: Option<MeshBounds>,
    total_vertices: u32,
    total_indices: u32,
    has_lod: bool,
    has_bone_assignments: bool,
}

fn read_u16(reader: &mut impl Read) -> io::Result<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_le_bytes(buf))
}

fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn read_f32(reader: &mut impl Read) -> io::Result<f32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(f32::from_le_bytes(buf))
}

fn read_bool(reader: &mut impl Read) -> io::Result<bool> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    Ok(buf[0] != 0)
}

fn read_cstring(reader: &mut impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        reader.read_exact(&mut buf)?;
        if buf[0] == 0 {
            break;
        }
        bytes.push(buf[0]);
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn read_line_string(reader: &mut impl Read) -> io::Result<String> {
    let mut bytes = Vec::new();
    let mut buf = [0u8; 1];
    loop {
        reader.read_exact(&mut buf)?;
        if buf[0] == b'\n' || buf[0] == 0 {
            break;
        }
        bytes.push(buf[0]);
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn parse_mesh(path: &Path) -> io::Result<MeshInfo> {
    let file = File::open(path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut info = MeshInfo {
        file_name: path.file_name().unwrap().to_string_lossy().to_string(),
        file_size,
        ..Default::default()
    };

    // Ogre file header: uint16 HEADER_ID + version_string (NO length field!)
    let header_id = read_u16(&mut reader)?;
    if header_id != M_HEADER {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Not an Ogre mesh"));
    }
    info.version = read_line_string(&mut reader)?;

    // M_MESH chunk: uint16 ID + uint32 LENGTH + data
    let mesh_id = read_u16(&mut reader)?;
    if mesh_id != M_MESH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Expected M_MESH (0x3000), got 0x{:04X}", mesh_id),
        ));
    }
    let mesh_len = read_u32(&mut reader)?;
    let mesh_data_start = reader.stream_position()?;
    let mesh_end = mesh_data_start + (mesh_len as u64).saturating_sub(6);

    info.skeletally_animated = read_bool(&mut reader)?;

    // Parse sub-chunks within M_MESH
    let mut chunk_count = 0u32;
    while reader.stream_position()? < mesh_end {
        let pos = reader.stream_position()?;
        let chunk_id = match read_u16(&mut reader) {
            Ok(id) => id,
            Err(_) => break,
        };
        let chunk_len = match read_u32(&mut reader) {
            Ok(l) => l,
            Err(_) => break,
        };
        // Safety: prevent infinite loops from bad data
        if chunk_len < 6 {
            break;
        }
        let data_len = chunk_len - 6;
        chunk_count += 1;
        if chunk_count > 10000 {
            break; // too many chunks, bail
        }

        match chunk_id {
            M_GEOMETRY => {
                let vert_count = read_u32(&mut reader)?;
                info.shared_vertex_count = vert_count;
                // Skip rest of geometry
                let remaining = data_len.saturating_sub(4);
                reader.seek(SeekFrom::Current(remaining as i64))?;
            }
            M_SUBMESH => {
                let start_pos = reader.stream_position()?;
                let material = read_cstring(&mut reader)?;
                let uses_shared = read_bool(&mut reader)?;
                let idx_count = read_u32(&mut reader)?;
                let idx_32bit = read_bool(&mut reader)?;

                // Skip index data
                let idx_size = if idx_32bit { 4u32 } else { 2u32 };
                reader.seek(SeekFrom::Current((idx_count * idx_size) as i64))?;

                let mut vert_count = 0u32;
                if !uses_shared {
                    // Check for embedded geometry
                    let sub_pos = reader.stream_position()?;
                    let bytes_used = (sub_pos - start_pos) as u32;
                    let sub_remaining = data_len.saturating_sub(bytes_used);
                    if sub_remaining >= 6 {
                        let sub_id = read_u16(&mut reader)?;
                        let _sub_len = read_u32(&mut reader)?;
                        if sub_id == M_GEOMETRY {
                            vert_count = read_u32(&mut reader)?;
                        }
                    }
                }

                let sub = SubMeshInfo {
                    material_name: material,
                    index_count: idx_count,
                    vertex_count: vert_count,
                    uses_shared_vertices: uses_shared,
                };
                info.total_indices += idx_count;
                info.total_vertices += vert_count;
                info.submeshes.push(sub);
                info.submesh_count += 1;

                // Seek to end of submesh chunk
                reader.seek(SeekFrom::Start(pos + chunk_len as u64))?;
            }
            M_MESH_SKELETON_LINK => {
                info.skeleton_link = read_cstring(&mut reader)?;
                let bytes_read = reader.stream_position()? - pos - 6;
                let skip = data_len as i64 - bytes_read as i64;
                if skip > 0 {
                    reader.seek(SeekFrom::Current(skip))?;
                }
            }
            M_MESH_BOUNDS => {
                let bounds = MeshBounds {
                    min_x: read_f32(&mut reader)?,
                    min_y: read_f32(&mut reader)?,
                    min_z: read_f32(&mut reader)?,
                    max_x: read_f32(&mut reader)?,
                    max_y: read_f32(&mut reader)?,
                    max_z: read_f32(&mut reader)?,
                    radius: read_f32(&mut reader)?,
                };
                info.bounds = Some(bounds);
            }
            M_MESH_LOD_LEVEL => {
                info.has_lod = true;
                reader.seek(SeekFrom::Start(pos + chunk_len as u64))?;
            }
            M_MESH_BONE_ASSIGNMENT => {
                info.has_bone_assignments = true;
                reader.seek(SeekFrom::Start(pos + chunk_len as u64))?;
            }
            _ => {
                // Skip unknown chunks
                reader.seek(SeekFrom::Start(pos + chunk_len as u64))?;
            }
        }
    }

    // Add shared vertices to total
    if info.shared_vertex_count > 0 {
        info.total_vertices += info.shared_vertex_count;
    }

    Ok(info)
}

fn scan_directory(dir: &Path) -> Vec<PathBuf> {
    let mut meshes = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                meshes.extend(scan_directory(&path));
            } else if path.extension().map_or(false, |e| e == "mesh") {
                meshes.push(path);
            }
        }
    }
    meshes
}

fn main() {
    let kenshi_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"M:\SteamLibrary\steamapps\common\Kenshi\data".to_string());

    let dir = Path::new(&kenshi_dir);
    println!("Scanning {} for .mesh files...", dir.display());

    let mesh_files = scan_directory(dir);
    println!("Found {} mesh files\n", mesh_files.len());

    let mut total_verts = 0u64;
    let mut total_indices = 0u64;
    let mut total_submeshes = 0u32;
    let mut skeletal_count = 0u32;
    let mut bounds_count = 0u32;
    let mut lod_count = 0u32;
    let mut errors = 0u32;
    let mut parsed = Vec::new();

    for (i, path) in mesh_files.iter().enumerate() {
        if i % 500 == 0 {
            eprint!("\r  Processing {}/{}...", i, mesh_files.len());
        }
        match parse_mesh(path) {
            Ok(info) => {
                total_verts += info.total_vertices as u64;
                total_indices += info.total_indices as u64;
                total_submeshes += info.submesh_count;
                if info.skeletally_animated {
                    skeletal_count += 1;
                }
                if info.bounds.is_some() {
                    bounds_count += 1;
                }
                if info.has_lod {
                    lod_count += 1;
                }
                parsed.push(info);
            }
            Err(e) => {
                errors += 1;
                if errors <= 5 {
                    eprintln!("  ERROR {}: {}", path.display(), e);
                }
            }
        }
    }

    // Sort by vertex count (largest first)
    parsed.sort_by(|a, b| b.total_vertices.cmp(&a.total_vertices));

    println!("═══════════════════════════════════════════════════");
    println!("  KENSHI MESH INDEX — Ogre MeshSerializer_v1.8");
    println!("═══════════════════════════════════════════════════");
    println!("  Total meshes parsed: {}", parsed.len());
    println!("  Parse errors: {}", errors);
    println!("  Total vertices: {} ({:.1}M)", total_verts, total_verts as f64 / 1_000_000.0);
    println!("  Total indices: {} ({:.1}M)", total_indices, total_indices as f64 / 1_000_000.0);
    println!("  Total submeshes: {}", total_submeshes);
    println!("  Skeletal meshes: {}", skeletal_count);
    println!("  Meshes with bounds: {}", bounds_count);
    println!("  Meshes with LOD: {}", lod_count);
    println!("═══════════════════════════════════════════════════\n");

    // Top 25 by vertex count
    println!("TOP 25 MESHES BY VERTEX COUNT:");
    println!("{:<40} {:>10} {:>10} {:>8} {:>7}", "Name", "Vertices", "Indices", "SubMsh", "Size MB");
    println!("{}", "-".repeat(80));
    for info in parsed.iter().take(25) {
        println!(
            "{:<40} {:>10} {:>10} {:>8} {:>7.1}",
            &info.file_name[..info.file_name.len().min(40)],
            info.total_vertices,
            info.total_indices,
            info.submesh_count,
            info.file_size as f64 / 1_048_576.0
        );
    }

    // Skeletal meshes (characters/creatures)
    println!("\nSKELETAL MESHES (animated characters/creatures):");
    println!("{:<40} {:>10} {:>10} {}", "Name", "Vertices", "Skeleton", "Materials");
    println!("{}", "-".repeat(80));
    for info in parsed.iter().filter(|i| i.skeletally_animated).take(30) {
        let mats: Vec<&str> = info.submeshes.iter().map(|s| s.material_name.as_str()).collect();
        println!(
            "{:<40} {:>10} {:>10} {}",
            &info.file_name[..info.file_name.len().min(40)],
            info.total_vertices,
            if info.skeleton_link.is_empty() { "(embedded)" } else { &info.skeleton_link },
            mats.join(", ").chars().take(30).collect::<String>()
        );
    }

    // Meshes with bounds (GIS data!)
    println!("\nLARGEST BOUNDING BOXES (world-scale structures):");
    let mut bounded: Vec<_> = parsed.iter().filter(|i| i.bounds.is_some()).collect();
    bounded.sort_by(|a, b| {
        let ra = a.bounds.as_ref().unwrap().radius;
        let rb = b.bounds.as_ref().unwrap().radius;
        rb.partial_cmp(&ra).unwrap_or(std::cmp::Ordering::Equal)
    });
    println!("{:<40} {:>10} {:>12}", "Name", "Radius", "Bounds (x,y,z)");
    println!("{}", "-".repeat(65));
    for info in bounded.iter().take(20) {
        let b = info.bounds.as_ref().unwrap();
        let dx = b.max_x - b.min_x;
        let dy = b.max_y - b.min_y;
        let dz = b.max_z - b.min_z;
        println!(
            "{:<40} {:>10.1} {:>4.0}×{:.0}×{:.0}",
            &info.file_name[..info.file_name.len().min(40)],
            b.radius,
            dx, dy, dz
        );
    }
}
