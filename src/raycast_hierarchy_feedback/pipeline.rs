use std::sync::{Arc, RwLock};

use glam::IVec3;
use wgpu::ComputePassDescriptor;

use crate::*;

const FEEDBACK_BUFFER_SIZE: usize = 16;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Feedback {
    requested: [Vec4; FEEDBACK_BUFFER_SIZE],
}
impl Feedback {
    fn empty() -> Self {
        Self {
            requested: [Vec4::ZERO; FEEDBACK_BUFFER_SIZE],
        }
    }
}

#[derive(Debug, Clone)]
enum FeedbackReadStatus {
    Idle,
    WaitForRead,
    Mapped,
}

pub struct Pipeline {
    pipeline_stream: wgpu::ComputePipeline,
    pipeline_raycast: wgpu::RenderPipeline,
    skip: bool,
    //
    feedback_cpu_buffer: wgpu::Buffer,
    feedback_gpu_bind_group: BindGroupState,
    feedback_read_available: Arc<RwLock<FeedbackReadStatus>>,
    voxels_bind_group: BindGroupState,
    //
    loaded_chunks: HashMap<IVec3, ChunkVersion>,
}

const PIPELINE_NAME: &str = "Raycast Hierarchy Feedback";

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

        let feedback = Feedback::empty();
        let feedback_gpu_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Feedback GPU Buffer"),
            contents: bytemuck::cast_slice(&[feedback]),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let feedback_cpu_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Feedback CPU Buffer"),
            contents: bytemuck::cast_slice(&[feedback]),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        });
        let feedback_gpu_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("feedback_gpu_bind_group_layout"),
            });
        let feedback_gpu_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &feedback_gpu_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: feedback_gpu_buffer.as_entire_binding(),
            }],
            label: Some("feedback_gpu_bind_group"),
        });
        let feedback_gpu_bind_group = BindGroupState {
            buffer: vec![feedback_gpu_buffer],
            bind_group: feedback_gpu_bind_group,
            bind_group_layout: feedback_gpu_bind_group_layout,
        };

        let chunks_grid_side = 8;
        let chunks_grid_volume = chunks_grid_side * chunks_grid_side * chunks_grid_side;
        let chunks_grid_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Grid Buffer"),
            contents: &vec![0u8; chunks_grid_volume],
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let voxels_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Voxels Buffer"),
            contents: &vec![0u8; CHUNK_VOLUME * 4 * chunks_grid_volume],
            usage: wgpu::BufferUsages::STORAGE,
        });
        let stream_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Stream Buffer"),
            contents: &vec![0u8; CHUNK_VOLUME * 4 * FEEDBACK_BUFFER_SIZE],
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let voxels_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("voxels_bind_group_layout"),
            });
        let voxels_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &voxels_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: chunks_grid_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: voxels_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: stream_buffer.as_entire_binding(),
                },
            ],
            label: Some("voxels_bind_group"),
        });
        let voxels_bind_group = BindGroupState {
            buffer: vec![chunks_grid_buffer, voxels_buffer, stream_buffer],
            bind_group: voxels_bind_group,
            bind_group_layout: voxels_bind_group_layout,
        };

        let render_shader =
            device.create_shader_module(wgpu::include_wgsl!("raycast_hierarchy_feedback.wgsl"));
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline Layout")),
                bind_group_layouts: &[
                    &global_bind_group.bind_group_layout,
                    &diffuse_bind_group.bind_group_layout,
                    &voxels_bind_group.bind_group_layout,
                    &feedback_gpu_bind_group.bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Render Pipeline")),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: "vs_main",
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
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

        let stream_shader = device.create_shader_module(wgpu::include_wgsl!("stream_chunks.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Stream Pipeline Layout")),
            bind_group_layouts: &[&voxels_bind_group.bind_group_layout],
            push_constant_ranges: &[],
        });
        let stream_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(&(PIPELINE_NAME.to_string() + " Stream Pipeline")),
            layout: Some(&pipeline_layout),
            module: &stream_shader,
            entry_point: "copy",
            compilation_options: Default::default(),
        });

        Self {
            pipeline_raycast: render_pipeline,
            pipeline_stream: stream_pipeline,
            skip: false,
            feedback_cpu_buffer,
            feedback_gpu_bind_group,
            feedback_read_available: Arc::new(RwLock::new(FeedbackReadStatus::Idle)),
            voxels_bind_group,
            loaded_chunks: HashMap::new(),
        }
    }

    fn extract(&mut self, sim_state: &mut SimulationState, queue: &wgpu::Queue) {
        let chunk_pos = IVec3::ZERO;
        let mut reload = false;

        let Some(chunk) = sim_state.universe.chunks.get(&chunk_pos) else {
            warn!("no chunk at 0,0,0");
            return;
        };

        if let Some(loaded_version) = self.loaded_chunks.get_mut(&chunk_pos) {
            if chunk.version != *loaded_version {
                *loaded_version = chunk.version.clone();
                reload = true;
            }
        } else {
            self.loaded_chunks
                .insert(chunk_pos.clone(), chunk.version.clone());
            reload = true;
        }

        if reload {
            /*
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
            */
        }

        let status = self.feedback_read_available.read().unwrap().clone();
        match status {
            FeedbackReadStatus::Idle => {
                *self.feedback_read_available.write().unwrap() = FeedbackReadStatus::WaitForRead;
                let arc = self.feedback_read_available.clone();
                let slice = self.feedback_cpu_buffer.slice(..);
                slice.map_async(wgpu::MapMode::Read, move |result| match result {
                    Ok(()) => {
                        *arc.write().unwrap() = FeedbackReadStatus::Mapped;
                    }
                    Err(e) => {
                        println!("error: {:?}", e);
                        panic!("feedback mapping error");
                    }
                });
            }
            FeedbackReadStatus::WaitForRead => {}
            FeedbackReadStatus::Mapped => {
                // read the mapped feedback buffer to get the request queue
                let slice = self.feedback_cpu_buffer.slice(..).get_mapped_range();
                let feed: &Feedback = bytemuck::from_bytes(slice.get(..).unwrap());
                drop(slice);
                self.feedback_cpu_buffer.unmap();
                *self.feedback_read_available.write().unwrap() = FeedbackReadStatus::Idle;

                // write to the streaming buffer the requested chunks

                // reset the gpu feedback request queue
                queue.write_buffer(
                    &self.feedback_gpu_bind_group.buffer[0],
                    0,
                    bytemuck::cast_slice(&[Feedback::empty()]),
                );
            }
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
        let Some(diffuse_bind_group) = bind_groups.get("diffuse") else {
            return;
        };

        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor::default());

            let dispatch_size = 32 / 4; // chunk size / 4
            compute_pass.set_pipeline(&self.pipeline_stream);
            compute_pass.set_bind_group(0, &self.voxels_bind_group.bind_group, &[]);
            compute_pass.dispatch_workgroups(dispatch_size, dispatch_size, dispatch_size);
        }

        {
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

            render_pass.set_pipeline(&self.pipeline_raycast);
            render_pass.set_bind_group(0, &global_bind_group.bind_group, &[]);
            render_pass.set_bind_group(1, &diffuse_bind_group.bind_group, &[]);
            render_pass.set_bind_group(2, &self.voxels_bind_group.bind_group, &[]);
            render_pass.set_bind_group(3, &self.feedback_gpu_bind_group.bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }

        if matches!(
            *self.feedback_read_available.read().unwrap(),
            FeedbackReadStatus::Idle
        ) {
            encoder.copy_buffer_to_buffer(
                &self.feedback_gpu_bind_group.buffer[0],
                0,
                &self.feedback_cpu_buffer,
                0,
                std::mem::size_of::<Feedback>() as u64,
            );
        }
    }

    fn get_skip(&self) -> bool {
        self.skip
    }

    fn set_skip(&mut self, skip: bool) {
        self.skip = skip
    }
}
