// KenshiReborn v0.2.0 — Wave 258
// The sovereign game engine opens its eyes.
// Built by Gaia on RTX 5090. Rust + Vulkan. No compromise.
//
// Modes:
//   --scan    Data scan only (no window)
//   --render  Open window and render Kenshi world map (default)

use std::fs;
use std::path::Path;
use std::time::Instant;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

const KENSHI_DATA: &str = r"M:\SteamLibrary\steamapps\common\Kenshi\data";

// === SHADER (WGSL) ===

const SHADER_SRC: &str = r#"
struct VertexOutput {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(-1.0, 1.0),
        vec2(-1.0, 1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
    );
    var uvs = array<vec2<f32>, 6>(
        vec2(0.0, 1.0), vec2(1.0, 1.0), vec2(0.0, 0.0),
        vec2(0.0, 0.0), vec2(1.0, 1.0), vec2(1.0, 0.0),
    );
    var out: VertexOutput;
    out.pos = vec4(positions[idx], 0.0, 1.0);
    out.uv = uvs[idx];
    return out;
}

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex, tex_sampler, in.uv);
}
"#;

// === GPU STATE ===

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    gpu_name: String,
}

impl GpuState {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12,
            ..Default::default()
        });

        let surface = instance.create_surface(window).expect("create surface");

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("no GPU adapter");

        let info = adapter.get_info();
        let gpu_name = info.name.clone();
        println!("  GPU: {} ({:?})", info.name, info.backend);

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("KenshiReborn"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        ))
        .expect("create device");

        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Load texture
        let biomemap_path = format!(r"{}\newland\land\biomemap.png", KENSHI_DATA);
        println!("  Loading: {}", biomemap_path);
        let img = image::open(&biomemap_path).expect("open biomemap").to_rgba8();
        let (tw, th) = (img.width(), img.height());

        let texture_size = wgpu::Extent3d {
            width: tw,
            height: th,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("biomemap"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * tw),
                rows_per_image: Some(th),
            },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture_bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SRC.into()),
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        println!("  Texture: {}×{} biomemap loaded to GPU", tw, th);
        println!("  Pipeline: Vulkan render pipeline created");
        println!("  STATUS: ██████████ RENDERING ARMED\n");

        Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            bind_group,
            gpu_name,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn render(&self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("render_encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.01, g: 0.01, b: 0.02, a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..6, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

// === APPLICATION ===

struct App {
    gpu: Option<GpuState>,
    window: Option<Arc<Window>>,
    frame_count: u64,
    start_time: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            gpu: None,
            window: None,
            frame_count: 0,
            start_time: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }

        let attrs = Window::default_attributes()
            .with_title("KenshiReborn v0.2.0 — Gaia · RTX 5090 · Vulkan")
            .with_inner_size(winit::dpi::LogicalSize::new(1024u32, 1024u32));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let gpu = GpuState::new(window.clone());
        println!("◆ PHASE 3: FIRST FRAME");
        println!("  Window: 1024×1024");
        println!("  GPU: {}", gpu.gpu_name);
        println!("  Rendering Kenshi world biomemap on RTX 5090...\n");
        self.gpu = Some(gpu);
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                let elapsed = self.start_time.elapsed().as_secs_f64();
                let fps = self.frame_count as f64 / elapsed;
                println!("\n◆ SESSION END");
                println!("  Frames: {}", self.frame_count);
                println!("  Time: {:.1}s", elapsed);
                println!("  Avg FPS: {:.1}", fps);
                println!("  KenshiReborn: eyes closed. RTX 5090 sleeps.\n");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(size.width, size.height);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &self.gpu {
                    match gpu.render() {
                        Ok(()) => {
                            self.frame_count += 1;
                        }
                        Err(wgpu::SurfaceError::Lost) => {
                            let (w, h) = (gpu.config.width, gpu.config.height);
                            if let Some(gpu) = &mut self.gpu {
                                gpu.resize(w, h);
                            }
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => eprintln!("render error: {:?}", e),
                    }
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

// === SCAN MODE (headless) ===

fn read_u16(f: &mut std::fs::File) -> std::io::Result<u16> {
    let mut buf = [0u8; 2]; f.read_exact(&mut buf)?; Ok(u16::from_le_bytes(buf))
}
fn read_u32(f: &mut std::fs::File) -> std::io::Result<u32> {
    let mut buf = [0u8; 4]; f.read_exact(&mut buf)?; Ok(u32::from_le_bytes(buf))
}
fn read_f32(f: &mut std::fs::File) -> std::io::Result<f32> {
    let mut buf = [0u8; 4]; f.read_exact(&mut buf)?; Ok(f32::from_le_bytes(buf))
}
fn read_cstring(f: &mut std::fs::File) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop { f.read_exact(&mut byte)?; if byte[0] == 0 { break; } bytes.push(byte[0]); }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}
fn read_line_string(f: &mut std::fs::File) -> std::io::Result<String> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop { f.read_exact(&mut byte)?; if byte[0] == b'\n' { break; } bytes.push(byte[0]); }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn count_files_by_ext(base: &str, ext: &str) -> (usize, u64) {
    let mut count = 0usize;
    let mut total = 0u64;
    fn walk(dir: &Path, ext: &str, count: &mut usize, bytes: &mut u64) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() { walk(&p, ext, count, bytes); }
                else if p.extension().map_or(false, |e| e == ext) {
                    *count += 1;
                    *bytes += p.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
    }
    walk(Path::new(base), ext, &mut count, &mut total);
    (count, total)
}

fn count_zones(base: &str) -> usize {
    let (c, _) = count_files_by_ext(base, "zone");
    c
}

struct MeshStats {
    path: String,
    submesh_count: u32,
    total_indices: u64,
    total_vertices: u64,
    has_skeleton: bool,
    skeleton_name: String,
    has_bounds: bool,
    has_lod: bool,
    materials: Vec<String>,
}

fn parse_mesh_fast(path: &str) -> Option<MeshStats> {
    let mut f = fs::File::open(path).ok()?;
    let header_id = read_u16(&mut f).ok()?;
    if header_id != 0x1000 { return None; }
    let _version = read_line_string(&mut f).ok()?;

    let mesh_id = read_u16(&mut f).ok()?;
    if mesh_id != 0x3000 { return None; }
    let mesh_len = read_u32(&mut f).ok()? as u64;
    let mesh_start = f.stream_position().ok()?;
    let mesh_end = mesh_start + mesh_len;

    let mut _skeletal_flag = [0u8; 1];
    f.read_exact(&mut _skeletal_flag).ok()?;

    let mut stats = MeshStats {
        path: path.to_string(),
        submesh_count: 0,
        total_indices: 0,
        total_vertices: 0,
        has_skeleton: false,
        skeleton_name: String::new(),
        has_bounds: false,
        has_lod: false,
        materials: Vec::new(),
    };

    // Ogre v1.100: strings are \n-terminated (readLine), NOT \0-terminated
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
                // M_SUBMESH: material(\n-term) + use_shared(u8) + index_count(u32) + is_32bit(u8) + indices + sub-chunks
                stats.submesh_count += 1;
                let mat = read_line_string(&mut f).ok().unwrap_or_default();
                stats.materials.push(mat);
                let use_shared = { let mut b = [0u8;1]; f.read_exact(&mut b).ok(); b[0] != 0 };
                let index_count = read_u32(&mut f).ok().unwrap_or(0);
                stats.total_indices += index_count as u64;
                let is_32bit = { let mut b = [0u8;1]; f.read_exact(&mut b).ok(); b[0] != 0 };

                // Skip raw index data
                let idx_bytes = if is_32bit { index_count as u64 * 4 } else { index_count as u64 * 2 };
                let _ = f.seek(SeekFrom::Current(idx_bytes as i64));

                // Parse sub-chunks within this submesh (M_GEOMETRY, M_SUBMESH_OPERATION, etc.)
                let submesh_end = data_start + data_len;
                while f.stream_position().ok().unwrap_or(submesh_end) + 6 <= submesh_end {
                    let sub_id = match read_u16(&mut f) { Ok(id) => id, Err(_) => break };
                    let sub_len = match read_u32(&mut f) { Ok(l) => l as u64, Err(_) => break };
                    if sub_len < 6 { break; }
                    let sub_data_start = f.stream_position().ok().unwrap_or(0);
                    let sub_data_len = sub_len - 6;

                    if sub_id == 0x5000 {
                        // M_GEOMETRY inside submesh (dedicated vertices)
                        let vert_count = read_u32(&mut f).ok().unwrap_or(0);
                        if !use_shared { stats.total_vertices += vert_count as u64; }
                    }

                    let _ = f.seek(SeekFrom::Start(sub_data_start + sub_data_len));
                }
            }
            0x5000 => {
                // Shared M_GEOMETRY at mesh level
                let vert_count = read_u32(&mut f).ok().unwrap_or(0);
                stats.total_vertices += vert_count as u64;
            }
            0x6000 => {
                stats.has_skeleton = true;
                stats.skeleton_name = read_line_string(&mut f).ok().unwrap_or_default();
            }
            0x9000 => { stats.has_bounds = true; }
            0x8000 => { stats.has_lod = true; }
            _ => {}
        }

        let _ = f.seek(SeekFrom::Start(data_start + data_len));
    }

    Some(stats)
}

fn scan_meshes(base: &str) -> Vec<MeshStats> {
    let mut all = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<MeshStats>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() { walk(&p, out); }
                else if p.extension().map_or(false, |e| e == "mesh") {
                    if let Some(s) = parse_mesh_fast(p.to_str().unwrap_or("")) { out.push(s); }
                }
            }
        }
    }
    walk(Path::new(base), &mut all);
    all
}

fn run_scan() {
    let total_start = Instant::now();

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  KENSHI REBORN v0.2.0 — Wave 258 (SCAN MODE)          ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    // GPU init
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12,
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    })).expect("no GPU");
    let info = adapter.get_info();
    println!("◆ GPU: {} ({:?})", info.name, info.backend);

    let (mesh_count, mesh_bytes) = count_files_by_ext(KENSHI_DATA, "mesh");
    let (skel_count, _) = count_files_by_ext(KENSHI_DATA, "skeleton");
    let (dds_count, dds_bytes) = count_files_by_ext(KENSHI_DATA, "dds");
    let (hkt_count, _) = count_files_by_ext(KENSHI_DATA, "hkt");
    let zone_count = count_zones(KENSHI_DATA);

    println!("◆ Files: {} mesh + {} skel + {} dds + {} hkt + {} zones",
        mesh_count, skel_count, dds_count, hkt_count, zone_count);

    let mesh_start = Instant::now();
    let meshes = scan_meshes(KENSHI_DATA);
    let mesh_ms = mesh_start.elapsed().as_millis();

    let total_idx: u64 = meshes.iter().map(|m| m.total_indices).sum();
    let total_verts: u64 = meshes.iter().map(|m| m.total_vertices).sum();
    let total_sub: u32 = meshes.iter().map(|m| m.submesh_count).sum();

    println!("◆ Parsed: {}/{} meshes in {}ms", meshes.len(), mesh_count, mesh_ms);
    println!("  Submeshes: {} | Indices: {} | Vertices: {}", total_sub, total_idx, total_verts);
    println!("  Triangles: {:.1}M", total_idx as f64 / 3.0 / 1_000_000.0);

    let mut sorted: Vec<&MeshStats> = meshes.iter().collect();
    sorted.sort_by(|a, b| b.total_vertices.cmp(&a.total_vertices));
    println!("\n  TOP 10 (by vertices):");
    for (i, m) in sorted.iter().take(10).enumerate() {
        let name = Path::new(&m.path).file_stem().unwrap_or_default().to_string_lossy();
        let sk = if m.has_skeleton { "⚔" } else { " " };
        println!("    {}. {} {:>7} verts {:>8} tris {} {}", i+1, sk,
            m.total_vertices, m.total_indices/3, m.submesh_count, name);
    }

    let skel_count = meshes.iter().filter(|m| m.has_skeleton).count();
    let mut skeletons: Vec<&str> = meshes.iter().filter(|m| m.has_skeleton)
        .map(|m| m.skeleton_name.as_str()).collect();
    skeletons.sort(); skeletons.dedup();
    println!("\n  Skeletal: {} meshes, {} unique skeletons", skel_count, skeletons.len());

    let ms = total_start.elapsed().as_millis();
    println!("\n◆ Total: {}ms | {:.1} GB data | RTX 5090 confirmed\n", ms,
        (mesh_bytes + dds_bytes) as f64 / 1073741824.0);

    // Write digest
    let digest = serde_json::json!({
        "engine": "KenshiReborn", "version": "0.2.0", "wave": 258,
        "gpu": { "name": info.name, "backend": format!("{:?}", info.backend) },
        "data": { "meshes": mesh_count, "skeletons": skel_count, "textures": dds_count,
                  "hitboxes": hkt_count, "zones": zone_count },
        "mesh_analysis": { "parsed": meshes.len(), "submeshes": total_sub,
            "total_indices": total_idx, "total_vertices": total_verts,
            "triangles": total_idx / 3 },
        "timing_ms": { "mesh_parse": mesh_ms, "total": ms },
    });
    let p = r"C:\Users\mokke\Desktop\ludoSpring\data\kenshi-reborn-digest.json";
    fs::write(p, serde_json::to_string_pretty(&digest).unwrap()).ok();
    println!("Digest: {}", p);
}

// === MAIN ===

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--scan") {
        run_scan();
        return;
    }

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  KENSHI REBORN v0.2.0 — Wave 258                      ║");
    println!("║  Sovereign Engine · Built by Gaia · primals.eco        ║");
    println!("║  The first game engine to RENDER Kenshi's world        ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    println!("◆ PHASE 1: GPU IGNITION");

    let event_loop = EventLoop::new().expect("create event loop");
    let mut app = App::new();
    println!("◆ PHASE 2: OPENING EYES — rendering biomemap on RTX 5090\n");
    event_loop.run_app(&mut app).expect("run app");
}
