use crate::*;

const PIPELINE_NAME: &str = "Rasterize Simple";

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}
impl Vertex {
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
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
    Vertex {
        position: [0.0, 0.0, 0.0],
        color: [0.0, 0.0, 0.0],
    },
    Vertex {
        position: [1.0, 0.0, 0.0],
        color: [1.0, 0.0, 0.0],
    },
    Vertex {
        position: [1.0, 1.0, 0.0],
        color: [1.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.0, 1.0, 0.0],
        color: [0.0, 1.0, 0.0],
    },
    Vertex {
        position: [0.0, 0.0, 1.0],
        color: [0.0, 0.0, 1.0],
    },
    Vertex {
        position: [1.0, 0.0, 1.0],
        color: [1.0, 0.0, 1.0],
    },
    Vertex {
        position: [1.0, 1.0, 1.0],
        color: [1.0, 1.0, 1.0],
    },
    Vertex {
        position: [0.0, 1.0, 1.0],
        color: [0.0, 1.0, 1.0],
    },
];

// with annotated faces
const INDICES: &[u16] = &[
    0, 3, 1, // -z
    3, 2, 1, // -z
    3, 6, 2, // +y
    3, 7, 6, // +y
    1, 2, 6, // +x
    1, 6, 5, // +x
    7, 4, 6, // +z
    6, 4, 5, // +z
    7, 3, 4, // -x
    4, 3, 0, // -x
    5, 0, 1, // -y
    4, 0, 5, // -y
];

pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    skip: bool,
    //
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
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

        let shader = device.create_shader_module(wgpu::include_wgsl!("rasterize_simple.wgsl"));
        let render_pipeline_rasterize_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline Layout")),
                bind_group_layouts: &[&global_bind_group.bind_group_layout],
                push_constant_ranges: &[],
            });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline")),
            layout: Some(&render_pipeline_rasterize_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
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

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipeline,
            skip: false,
            vertex_buffer,
            index_buffer,
        }
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
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
    }

    fn get_skip(&self) -> bool {
        self.skip
    }

    fn set_skip(&mut self, skip: bool) {
        self.skip = skip
    }
}
