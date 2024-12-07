use glam::IVec3;

use crate::*;

pub struct Pipeline {
    pipeline: wgpu::RenderPipeline,
    skip: bool,
    //
    voxels_bind_group: BindGroupState,
}

const PIPELINE_NAME: &str = "Raycast Grid Plain";

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

        let voxels_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Voxels Buffer"),
            contents: &vec![0u8; CHUNK_VOLUME * 4],
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let voxels_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("voxels_bind_group_layout"),
            });
        let voxels_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &voxels_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: voxels_buffer.as_entire_binding(),
            }],
            label: Some("voxels_bind_group"),
        });
        let voxels_bind_group = BindGroupState {
            buffer: vec![voxels_buffer],
            bind_group: voxels_bind_group,
            bind_group_layout: voxels_bind_group_layout,
        };

        let shader = device.create_shader_module(wgpu::include_wgsl!("raycast_grid_plain.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline Layout")),
            bind_group_layouts: &[
                &global_bind_group.bind_group_layout,
                &diffuse_bind_group.bind_group_layout,
                &voxels_bind_group.bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline")),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
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

        Self {
            pipeline,
            skip: false,
            voxels_bind_group,
        }
    }

    fn extract(&mut self, sim_state: &mut SimulationState, queue: &wgpu::Queue) {
        // todo: it rewrites everything every frame
        let Some(chunk_data) = sim_state
            .universe
            .chunks
            .get(&IVec3::ZERO)
            .map(|c| c.get_ref())
        else {
            warn!("no chunk at 0,0,0");
            return;
        };

        queue.write_buffer(
            &self.voxels_bind_group.buffer[0],
            0,
            bytemuck::cast_slice(chunk_data.as_ref()),
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
        render_pass.set_bind_group(2, &self.voxels_bind_group.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn get_skip(&self) -> bool {
        self.skip
    }

    fn set_skip(&mut self, skip: bool) {
        self.skip = skip
    }
}
