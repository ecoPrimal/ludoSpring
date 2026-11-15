// render3d.rs — 3D mesh renderer on WGPU/Vulkan
// Auto-rotating camera + diffuse lighting

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

use crate::mesh3d::MeshData;

const SHADER_3D: &str = r#"
struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    light_dir: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
};
struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = u.mvp * vec4(in.position, 1.0);
    out.world_normal = (u.model * vec4(in.normal, 0.0)).xyz;

    // Gate nodes glow (normal.x > 90 = gate marker flag)
    let is_gate = in.normal.x > 90.0;
    if is_gate {
        // Gate color encoded in normal: y = R*10, z = G*10
        out.color = vec3(in.normal.y / 10.0, in.normal.z / 10.0, 0.3);
    } else {
        // Kenshi desert palette — warm sandstone
        let n_abs = abs(normalize(in.normal));
        out.color = vec3(0.65 + n_abs.x * 0.2, 0.50 + n_abs.y * 0.15, 0.35 + n_abs.z * 0.1);
    }
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    // Gate nodes: emit light (no shading)
    let is_emissive = length(in.world_normal) < 0.01;
    if is_emissive {
        return vec4(in.color * 1.5, 1.0);
    }
    // Three-light rig: key + fill + rim
    let key_dir = normalize(u.light_dir.xyz);
    let fill_dir = normalize(vec3(-0.3, 0.8, -0.5));
    let key = max(dot(n, key_dir), 0.0) * 0.6;
    let fill = max(dot(n, fill_dir), 0.0) * 0.25;
    let ambient = 0.3;
    let rim = pow(1.0 - max(dot(n, normalize(vec3(0.0, 0.3, 1.0))), 0.0), 2.5) * 0.2;
    let base = in.color * (ambient + key + fill) + vec3(rim * 0.8, rim * 0.6, rim * 0.4);
    // Subtle geodesic glow on the background
    return vec4(base, 1.0);
}
"#;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex3D {
    position: [f32; 3],
    normal: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    mvp: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    light_dir: [f32; 4],
}

fn mat4_identity() -> [[f32; 4]; 4] {
    [[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]]
}

fn mat4_perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> [[f32; 4]; 4] {
    // WGPU clip space: Z maps to [0, 1] (not OpenGL's [-1, 1])
    let f = 1.0 / (fov_y / 2.0).tan();
    let nf = 1.0 / (near - far);
    [
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far * nf, -1.0],
        [0.0, 0.0, near * far * nf, 0.0],
    ]
}

fn mat4_look_at(eye: [f32; 3], center: [f32; 3], up: [f32; 3]) -> [[f32; 4]; 4] {
    let f = normalize3(sub3(center, eye));
    let s = normalize3(cross3(f, up));
    let u = cross3(s, f);
    [[s[0],u[0],-f[0],0.0],[s[1],u[1],-f[1],0.0],[s[2],u[2],-f[2],0.0],[-dot3(s,eye),-dot3(u,eye),dot3(f,eye),1.0]]
}

fn mat4_rotate_y(angle: f32) -> [[f32; 4]; 4] {
    let c = angle.cos(); let s = angle.sin();
    [[c,0.0,s,0.0],[0.0,1.0,0.0,0.0],[-s,0.0,c,0.0],[0.0,0.0,0.0,1.0]]
}

fn mat4_mul(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    // Column-major: m[col][row]. C = A * B means C_col_j = A * B_col_j
    let mut r = [[0.0f32; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            for k in 0..4 {
                r[col][row] += a[k][row] * b[col][k];
            }
        }
    }
    r
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn cross3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] { [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]] }
fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
fn normalize3(v: [f32; 3]) -> [f32; 3] { let l = dot3(v,v).sqrt(); if l < 1e-10 { v } else { [v[0]/l, v[1]/l, v[2]/l] } }

struct Gpu3D {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    index_count: u32,
    mesh_center: [f32; 3],
    mesh_radius: f32,
    start_time: Instant,
    gpu_name: String,
}

impl Gpu3D {
    fn new(window: Arc<Window>, mesh: &MeshData) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::DX12,
            ..Default::default()
        });
        let surface = instance.create_surface(window).expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })).expect("adapter");

        let info = adapter.get_info();
        let gpu_name = info.name.clone();
        println!("  GPU: {} ({:?})", info.name, info.backend);

        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("KenshiReborn3D"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )).expect("device");

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

        // Compute mesh bounding box for camera
        let mut min = [f32::MAX; 3];
        let mut max = [f32::MIN; 3];
        for p in &mesh.positions {
            for i in 0..3 { min[i] = min[i].min(p[i]); max[i] = max[i].max(p[i]); }
        }
        let center = [(min[0]+max[0])/2.0, (min[1]+max[1])/2.0, (min[2]+max[2])/2.0];
        let radius = sub3(max, min).iter().map(|d| d*d).sum::<f32>().sqrt() / 2.0;
        println!("  Bounds: [{:.1},{:.1},{:.1}] → [{:.1},{:.1},{:.1}], radius={:.1}",
            min[0], min[1], min[2], max[0], max[1], max[2], radius);

        // Vertex buffer — mesh + gate node octahedrons orbiting as snow globe
        let mut vertices: Vec<Vertex3D> = mesh.positions.iter().enumerate().map(|(i, p)| {
            let n = if i < mesh.normals.len() { mesh.normals[i] } else { [0.0, 1.0, 0.0] };
            Vertex3D { position: *p, normal: n }
        }).collect();
        let mut indices = mesh.indices.clone();
        let mesh_verts = vertices.len() as u32;
        let mesh_indices = indices.len();

        // Gate nodes — octahedrons orbiting at 2× radius
        // Each gate: 6 verts (±X, ±Y, ±Z) + 8 triangles
        // Normal flag: x>90 signals gate to shader, y/z encode RGB color
        struct GateNode { angle: f32, elev: f32, r: f32, g: f32 }
        let gates = vec![
            GateNode { angle: 0.0,    elev: 0.2,  r: 0.0, g: 8.0 },  // northGate — green
            GateNode { angle: 1.57,   elev: 0.0,  r: 8.0, g: 4.0 },  // eastGate — orange
            GateNode { angle: 3.14,   elev: -0.1, r: 4.0, g: 8.0 },  // ironGate — lime
            GateNode { angle: 4.71,   elev: 0.1,  r: 2.0, g: 6.0 },  // sporeGate — teal
            GateNode { angle: 0.8,    elev: 0.5,  r: 8.0, g: 0.0 },  // grapheneGate — red
            GateNode { angle: 2.4,    elev: -0.4, r: 3.0, g: 3.0 },  // graftGate — dim
        ];
        let orb_r = radius * 2.2;
        let node_size = radius * 0.08;
        for g in &gates {
            let cx = center[0] + orb_r * g.angle.cos() * g.elev.cos();
            let cy = center[1] + orb_r * g.elev.sin();
            let cz = center[2] + orb_r * g.angle.sin() * g.elev.cos();
            let s = node_size;
            let base = vertices.len() as u32;
            // Gate color encoded in normal: x=99 (flag), y=R*10, z=G*10
            let gn = [99.0, g.r, g.g];
            vertices.push(Vertex3D { position: [cx+s, cy, cz], normal: gn });
            vertices.push(Vertex3D { position: [cx-s, cy, cz], normal: gn });
            vertices.push(Vertex3D { position: [cx, cy+s, cz], normal: gn });
            vertices.push(Vertex3D { position: [cx, cy-s, cz], normal: gn });
            vertices.push(Vertex3D { position: [cx, cy, cz+s], normal: gn });
            vertices.push(Vertex3D { position: [cx, cy, cz-s], normal: gn });
            // 8 faces of octahedron
            for &[a,b,c] in &[[0,2,4],[2,1,4],[1,3,4],[3,0,4],[2,0,5],[1,2,5],[3,1,5],[0,3,5u32]] {
                indices.push(base+a); indices.push(base+b); indices.push(base+c);
            }
        }
        println!("  Snow globe: {} gate nodes ({} extra verts, {} extra tris)",
            gates.len(), vertices.len() - mesh_verts as usize, (indices.len() - mesh_indices) / 3);

        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let uniforms = Uniforms { mvp: mat4_identity(), model: mat4_identity(), light_dir: [0.5, 1.0, 0.3, 0.0] };
        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("uniforms"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniform_buf.as_entire_binding() }],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader3d"), source: wgpu::ShaderSource::Wgsl(SHADER_3D.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl"), bind_group_layouts: &[&bind_group_layout], push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rp3d"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex3D>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format, blend: Some(wgpu::BlendState::REPLACE), write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None, // Show both sides
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let depth_view = Self::create_depth(&device, config.width, config.height);

        println!("  Uploaded: {} verts + {} indices to GPU", vertices.len(), mesh.indices.len());

        Self {
            surface, device, queue, config, pipeline,
            vertex_buf, index_buf, uniform_buf, bind_group, depth_view,
            index_count: indices.len() as u32,
            mesh_center: center, mesh_radius: radius,
            start_time: Instant::now(), gpu_name,
        }
    }

    fn create_depth(device: &wgpu::Device, w: u32, h: u32) -> wgpu::TextureView {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d { width: w.max(1), height: h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        }).create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = Self::create_depth(&self.device, w, h);
        }
    }

    fn render(&self) -> Result<(), wgpu::SurfaceError> {
        let t = self.start_time.elapsed().as_secs_f32();
        let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
        let dist = self.mesh_radius * 2.5;

        // Auto-orbit camera
        let angle = t * 0.5;
        let eye = [
            self.mesh_center[0] + dist * angle.cos(),
            self.mesh_center[1] + self.mesh_radius * 0.5,
            self.mesh_center[2] + dist * angle.sin(),
        ];

        let model = mat4_identity();
        let view = mat4_look_at(eye, self.mesh_center, [0.0, 1.0, 0.0]);
        let proj = mat4_perspective(std::f32::consts::FRAC_PI_4, aspect, 0.1, dist * 10.0);
        let mvp = mat4_mul(proj, mat4_mul(view, model));

        let uniforms = Uniforms {
            mvp,
            model,
            light_dir: [0.5, 1.0, 0.3, 0.0],
        };
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::cast_slice(&[uniforms]));

        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("enc") });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rp"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.02, g: 0.02, b: 0.05, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            pass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..self.index_count, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}

pub struct App3D {
    gpu: Option<Gpu3D>,
    window: Option<Arc<Window>>,
    mesh: MeshData,
    frame_count: u64,
    start: Instant,
}

impl App3D {
    pub fn new(mesh: MeshData) -> Self {
        Self { gpu: None, window: None, mesh, frame_count: 0, start: Instant::now() }
    }
}

impl ApplicationHandler for App3D {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() { return; }
        let title = format!("KenshiReborn v0.6.0 — {} — Snow Globe · 6 Gates · RTX 5090", self.mesh.name);
        let attrs = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(winit::dpi::LogicalSize::new(1280u32, 720u32));
        let window = Arc::new(event_loop.create_window(attrs).expect("window"));
        let gpu = Gpu3D::new(window.clone(), &self.mesh);
        println!("  Pipeline: 3D render with depth + lighting");
        println!("  Camera: auto-orbit around mesh center");
        println!("  STATUS: ██████████ 3D RENDERING LIVE\n");
        self.gpu = Some(gpu);
        self.window = Some(window);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                let fps = self.frame_count as f64 / self.start.elapsed().as_secs_f64();
                println!("\n◆ CLOSED — {} frames, {:.1} FPS", self.frame_count, fps);
                event_loop.exit();
            }
            WindowEvent::Resized(s) => { if let Some(g) = &mut self.gpu { g.resize(s.width, s.height); } }
            WindowEvent::RedrawRequested => {
                if let Some(g) = &self.gpu {
                    match g.render() {
                        Ok(()) => self.frame_count += 1,
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            let (w,h) = (g.config.width, g.config.height);
                            if let Some(g) = &mut self.gpu { g.resize(w,h); }
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => eprintln!("render: {:?}", e),
                    }
                }
                if let Some(w) = &self.window { w.request_redraw(); }
            }
            _ => {}
        }
    }
}

pub fn run_mesh_render(mesh_path: &str) {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  KENSHI REBORN v0.4.0 — WOOD AGE                      ║");
    println!("║  3D Mesh Renderer · Vulkan · RTX 5090                  ║");
    println!("║  The first Kenshi mesh rendered by a sovereign engine   ║");
    println!("╚══════════════════════════════════════════════════════════╝\n");

    println!("◆ LOADING MESH: {}", mesh_path);
    let mesh = load_mesh(mesh_path).expect("Failed to load mesh");
    println!("  ✓ {} vertices, {} triangles\n", mesh.positions.len(), mesh.indices.len() / 3);

    println!("◆ INITIALIZING 3D RENDERER");
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App3D::new(mesh);
    event_loop.run_app(&mut app).expect("run");
}

use crate::mesh3d::load_mesh;
use wgpu::util::DeviceExt;
