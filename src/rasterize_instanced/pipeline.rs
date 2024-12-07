use crate::*;

// instancing a cube a lot of times
// this approach doesn't support transparency
// as it would mean reordering every cube every frame to draw back to front
const PIPELINE_NAME: &str = "Rasterize Instanced";

const NUM_INSTANCES_PER_ROW: u32 = 32;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    pos: Vec3,
    id: u32,
}

impl Instance {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Instance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
}
impl Vertex {
    const fn new(position: [f32; 3], uv: [f32; 2]) -> Self {
        Self { position, uv }
    }
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

// flat shaded cube
const VERTICES: &[Vertex] = &[
    // -z [0, 3, 1]
    Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0]),
    Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0]),
    Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0]),
    // -z [3, 2, 1]
    Vertex::new([0.0, 1.0, 0.0], [0.0, 1.0]),
    Vertex::new([1.0, 1.0, 0.0], [1.0, 1.0]),
    Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0]),
    // +y [3, 6, 2]
    Vertex::new([0.0, 1.0, 0.0], [0.0, 0.0]),
    Vertex::new([1.0, 1.0, 1.0], [1.0, 1.0]),
    Vertex::new([1.0, 1.0, 0.0], [1.0, 0.0]),
    // +y [3, 7, 6]
    Vertex::new([0.0, 1.0, 0.0], [0.0, 0.0]),
    Vertex::new([0.0, 1.0, 1.0], [1.0, 0.0]),
    Vertex::new([1.0, 1.0, 1.0], [1.0, 1.0]),
    // +x [1, 2, 6]
    Vertex::new([1.0, 0.0, 0.0], [0.0, 0.0]),
    Vertex::new([1.0, 1.0, 0.0], [0.0, 1.0]),
    Vertex::new([1.0, 1.0, 1.0], [1.0, 1.0]),
    // +x [1, 6, 5]
    Vertex::new([1.0, 0.0, 0.0], [0.0, 0.0]),
    Vertex::new([1.0, 1.0, 1.0], [1.0, 1.0]),
    Vertex::new([1.0, 0.0, 1.0], [1.0, 0.0]),
    // +z [7, 4, 6]
    Vertex::new([0.0, 1.0, 1.0], [1.0, 1.0]),
    Vertex::new([0.0, 0.0, 1.0], [1.0, 0.0]),
    Vertex::new([1.0, 1.0, 1.0], [0.0, 1.0]),
    // +z [6, 4, 5]
    Vertex::new([1.0, 1.0, 1.0], [0.0, 1.0]),
    Vertex::new([0.0, 0.0, 1.0], [1.0, 0.0]),
    Vertex::new([1.0, 0.0, 1.0], [0.0, 0.0]),
    // -x [7, 3, 4]
    Vertex::new([0.0, 1.0, 1.0], [0.0, 1.0]),
    Vertex::new([0.0, 1.0, 0.0], [1.0, 1.0]),
    Vertex::new([0.0, 0.0, 1.0], [0.0, 0.0]),
    // -x [4, 3, 0]
    Vertex::new([0.0, 0.0, 1.0], [0.0, 0.0]),
    Vertex::new([0.0, 1.0, 0.0], [1.0, 1.0]),
    Vertex::new([0.0, 0.0, 0.0], [1.0, 0.0]),
    // -y [5, 0, 1]
    Vertex::new([1.0, 0.0, 1.0], [1.0, 1.0]),
    Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0]),
    Vertex::new([1.0, 0.0, 0.0], [1.0, 0.0]),
    // -y [4, 0, 5]
    Vertex::new([0.0, 0.0, 1.0], [0.0, 1.0]),
    Vertex::new([0.0, 0.0, 0.0], [0.0, 0.0]),
    Vertex::new([1.0, 0.0, 1.0], [1.0, 1.0]),
];

pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    skip: bool,
    //
    vertex_buffer: wgpu::Buffer,
    instances: Vec<Instance>,
    instance_buffer: wgpu::Buffer,
}

impl PipelineState for Pipeline {
    fn get_name(&self) -> String {
        PIPELINE_NAME.to_string()
    }

    fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        bind_groups: &mut HashMap<String, BindGroupState>,
    ) -> Self {
        let Some(global_bind_group) = bind_groups.get("global") else {
            panic!("global bind group missing");
        };
        let Some(diffuse_bind_group) = bind_groups.get("diffuse") else {
            panic!("diffuse bind group missing");
        };

        let shader = device.create_shader_module(wgpu::include_wgsl!("rasterize_instanced.wgsl"));
        let render_pipeline_rasterize_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline Layout")),
                bind_group_layouts: &[
                    &global_bind_group.bind_group_layout,
                    &diffuse_bind_group.bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline")),
            layout: Some(&render_pipeline_rasterize_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc(), Instance::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0x0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: bytemuck::cast_slice(VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let instances = (0..NUM_INSTANCES_PER_ROW)
            .flat_map(|z| {
                (0..NUM_INSTANCES_PER_ROW).flat_map(move |x| {
                    (0..NUM_INSTANCES_PER_ROW).map(move |y| Instance {
                        pos: Vec3::new(x as f32, y as f32, z as f32),
                        id: ((x + y * 16 + z * 256) % 256) as u32,
                    })
                })
            })
            .collect::<Vec<_>>();

        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        Self {
            pipeline,
            skip: false,
            vertex_buffer,
            instances,
            instance_buffer,
        }
    }

    fn extract(&mut self, sim_state: &mut SimulationState, queue: &wgpu::Queue) {
        // todo: it rewrites everything every frame
        self.instances.clear();
        for (world_xyz, chunk) in sim_state.universe.chunks.iter() {
            let r = chunk.get_ref();
            for chunk_xyz in Chunk::iter() {
                let i = Chunk::xyz2idx(chunk_xyz);
                let id = r[i].id as u32;
                if id == 0 {
                    continue;
                }
                let pos = (world_xyz + chunk_xyz).as_vec3();
                self.instances.push(Instance { pos, id });
            }
        }

        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&self.instances),
        );
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_groups: &HashMap<String, BindGroupState>,
        attachments: &HashMap<String, Attachment>,
        clear_depth: bool,
    ) {
        let Some(Attachment::Color(color_attachment)) = attachments.get("color") else {
            return;
        };
        let Some(Attachment::Depth(depth_attachment)) = attachments.get("depth") else {
            return;
        };
        let Some(global_bind_group) = bind_groups.get("global") else {
            return;
        };
        let Some(diffuse_bind_group) = bind_groups.get("diffuse") else {
            return;
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Render Pass")),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_attachment.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_attachment.view,
                depth_ops: Some(wgpu::Operations {
                    load: if clear_depth {
                        wgpu::LoadOp::Clear(1.0)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &global_bind_group.bind_group, &[]);
        render_pass.set_bind_group(1, &diffuse_bind_group.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        render_pass.draw(0..VERTICES.len() as u32, 0..self.instances.len() as _);
    }

    fn get_skip(&self) -> bool {
        self.skip
    }

    fn set_skip(&mut self, skip: bool) {
        self.skip = skip
    }
}
