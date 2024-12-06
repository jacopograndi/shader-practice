use crate::*;

pub struct Pipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub skip: bool,
}

const PIPELINE_NAME: &str = "Debug Depth";

impl PipelineState for Pipeline {
    fn get_name(&self) -> String {
        PIPELINE_NAME.to_string()
    }

    fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        bind_groups: &mut HashMap<String, BindGroupState>,
    ) -> Self {
        let Some(depth_bind_group) = bind_groups.get("depth") else {
            panic!("global bind group missing");
        };

        let shader = device.create_shader_module(wgpu::include_wgsl!("debug_depth.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline Layout")),
            bind_group_layouts: &[&depth_bind_group.bind_group_layout],
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0x0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });
        Self {
            pipeline,
            skip: true,
        }
    }

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_groups: &HashMap<String, BindGroupState>,
        attachments: &HashMap<String, Attachment>,
        _clear_depth: bool,
    ) {
        let Some(Attachment::Color(color_attachment)) = attachments.get("color") else {
            return;
        };
        let Some(depth_bind_group) = bind_groups.get("depth") else {
            return;
        };

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Render Pass")),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_attachment.view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &depth_bind_group.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    fn get_skip(&self) -> bool {
        self.skip
    }

    fn set_skip(&mut self, skip: bool) {
        self.skip = skip
    }
}
