use std::{
    collections::HashMap,
    f32::consts::PI,
    time::{Duration, Instant},
};

use glam::{
    f32::{Vec2, Vec3, Vec4},
    EulerRot, Mat4, Quat,
};
use log::{error, info, warn};
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowBuilder},
};

mod attachments;
mod voxels;

mod analytical_sdf_cube;
mod analytical_sdf_sphere;
mod debug_depth;
mod debug_empty;
mod debug_ui;
mod rasterize_instanced;
mod rasterize_simple;
mod raycast_grid_plain;
mod raycast_sdf;

use attachments::*;
use voxels::*;

pub trait PipelineState {
    fn new(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
        bind_groups: &mut HashMap<String, BindGroupState>,
    ) -> Self
    where
        Self: Sized;

    fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_groups: &HashMap<String, BindGroupState>,
        attachments: &HashMap<String, Attachment>,
        clear_depth: bool,
    );

    fn extract(&mut self, _sim_state: &mut SimulationState, queue: &wgpu::Queue) {}

    fn get_skip(&self) -> bool;
    fn set_skip(&mut self, skip: bool);

    fn get_name(&self) -> String;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalUniform {
    viewport_size: Vec4,
    view_world_position: Vec4,
    world_from_clip: Mat4,
    clip_from_world: Mat4,
    view_from_clip: Mat4,
    clip_from_view: Mat4,
    view_from_world: Mat4,
    world_from_view: Mat4,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct UiUniform {
    pipelines_skip: [[u32; 4]; 256],
    pipelines_num: u32,
}

pub struct BindGroupState {
    pub buffer: Vec<wgpu::Buffer>,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

struct RenderState<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: &'a Window,
    //
    bind_groups: HashMap<String, BindGroupState>,
    attachments: HashMap<String, Attachment>,
    pipelines: Vec<Box<dyn PipelineState>>,
    //
    uniform_global: GlobalUniform,
    uniform_ui: UiUniform,
}

impl<'a> RenderState<'a> {
    async fn new(window: &'a Window) -> RenderState<'a> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                },
                None,
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            desired_maximum_frame_latency: 2,
            view_formats: vec![],
        };

        let mut bind_groups = HashMap::new();

        let uniform_global = GlobalUniform {
            viewport_size: Vec4::ZERO,
            view_world_position: Vec4::ZERO,
            world_from_clip: Mat4::ZERO,
            clip_from_world: Mat4::ZERO,
            view_from_clip: Mat4::ZERO,
            clip_from_view: Mat4::ZERO,
            view_from_world: Mat4::ZERO,
            world_from_view: Mat4::ZERO,
        };
        let global_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Buffer"),
            contents: bytemuck::cast_slice(&[uniform_global]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let global_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("global_bind_group_layout"),
            });
        let global_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &global_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: global_uniform_buffer.as_entire_binding(),
            }],
            label: Some("global_bind_group"),
        });
        let global_bind_group = BindGroupState {
            buffer: vec![global_uniform_buffer],
            bind_group: global_uniform_bind_group,
            bind_group_layout: global_uniform_bind_group_layout,
        };
        bind_groups.insert("global".to_string(), global_bind_group);

        let uniform_ui = UiUniform {
            pipelines_num: 1,
            pipelines_skip: [[0; 4]; 256],
        };
        let ui_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Ui Buffer"),
            contents: bytemuck::cast_slice(&[uniform_ui]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let ui_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("ui_bind_group_layout"),
            });
        let ui_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &ui_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ui_uniform_buffer.as_entire_binding(),
            }],
            label: Some("ui_bind_group"),
        });
        let ui_bind_group = BindGroupState {
            buffer: vec![ui_uniform_buffer],
            bind_group: ui_uniform_bind_group,
            bind_group_layout: ui_uniform_bind_group_layout,
        };
        bind_groups.insert("ui".to_string(), ui_bind_group);

        let diffuse_bytes = include_bytes!(".././assets/blocks.png");
        let diffuse_image = image::load_from_memory(diffuse_bytes).unwrap();
        let diffuse_rgba = diffuse_image.to_rgba8();
        use image::GenericImageView;
        let dimensions = diffuse_image.dimensions();
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };
        let diffuse_texture = device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("diffuse_texture"),
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &diffuse_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &diffuse_rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * dimensions.0),
                rows_per_image: Some(dimensions.1),
            },
            texture_size,
        );
        let diffuse_texture_view =
            diffuse_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
                        // This should match the filterable field of the
                        // corresponding Texture entry above.
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });
        let diffuse_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });
        let diffuse_bind_group = BindGroupState {
            buffer: vec![],
            bind_group: diffuse_bind_group,
            bind_group_layout: texture_bind_group_layout,
        };
        bind_groups.insert("diffuse".to_string(), diffuse_bind_group);

        let mut attachments = HashMap::new();
        attachments.insert(
            "depth".to_string(),
            Attachment::Depth(DepthAttachment::create_depth_texture(
                &device,
                &config,
                &mut bind_groups,
            )),
        );

        let mut pipelines: Vec<Box<dyn PipelineState>> = Vec::new();

        // Shorthand to construct the pipelines vec
        struct Params<'a> {
            pipelines: &'a mut Vec<Box<dyn PipelineState>>,
            device: &'a wgpu::Device,
            config: &'a wgpu::SurfaceConfiguration,
            bind_groups: &'a mut HashMap<String, BindGroupState>,
        }
        let mut p = Params {
            pipelines: &mut pipelines,
            device: &device,
            config: &config,
            bind_groups: &mut bind_groups,
        };
        fn push_pipeline<'a, T: PipelineState + 'static>(p: &'a mut Params) {
            p.pipelines
                .push(Box::new(T::new(p.device, p.config, p.bind_groups)))
        }

        // ┌─┐                                  ┌─┐ //
        // │ ├──────────────────────────────────┤ │ //
        // │ │                                  │ │ //
        // │ │                                  │ │ //
        // │ ├──────────────────────────────────┤ │~~~
        // │ ├──────────────────────────────────┤ │~~~
        // └─┘                                  └─┘ //
        //push_pipeline::<raycast_sdf::Pipeline>(&mut p);
        //push_pipeline::<analytical_sdf_sphere::Pipeline>(&mut p);
        //push_pipeline::<analytical_sdf_cube::Pipeline>(&mut p);
        //push_pipeline::<rasterize_simple::Pipeline>(&mut p);
        push_pipeline::<raycast_grid_plain::Pipeline>(&mut p);
        push_pipeline::<rasterize_instanced::Pipeline>(&mut p);
        push_pipeline::<debug_depth::Pipeline>(&mut p);
        push_pipeline::<debug_ui::Pipeline>(&mut p);

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
            uniform_global,
            uniform_ui,
            attachments,
            bind_groups,
            pipelines,
        }
    }

    fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.uniform_global.viewport_size =
                Vec4::new(new_size.width as f32, new_size.height as f32, 0.0, 0.0);
            self.uniform_global.clip_from_view = Mat4::perspective_rh(
                PI * 0.5,
                self.uniform_global.viewport_size.x / self.uniform_global.viewport_size.y,
                0.1,
                1000.0,
            );
            self.uniform_global.view_from_clip = self.uniform_global.clip_from_view.inverse();

            self.attachments.insert(
                "depth".to_string(),
                Attachment::Depth(DepthAttachment::create_depth_texture(
                    &self.device,
                    &self.config,
                    &mut self.bind_groups,
                )),
            );
        }
    }

    pub fn extract(&mut self, sim_state: &mut SimulationState) {
        self.uniform_global.view_world_position = sim_state.camera_position.extend(0.0);
        self.uniform_global.world_from_view =
            Mat4::from_rotation_translation(sim_state.camera_rotation, sim_state.camera_position);
        self.uniform_global.view_from_world = self.uniform_global.world_from_view.inverse();

        // the view-projection matrix
        self.uniform_global.clip_from_world =
            self.uniform_global.clip_from_view * self.uniform_global.view_from_world;
        self.uniform_global.world_from_clip = self.uniform_global.clip_from_world.inverse();

        let Some(global_buffer) = self
            .bind_groups
            .get("global")
            .map(|b| b.buffer.get(0))
            .flatten()
        else {
            return;
        };
        self.queue.write_buffer(
            global_buffer,
            0,
            bytemuck::cast_slice(&[self.uniform_global]),
        );

        self.uniform_ui.pipelines_num = self.pipelines.len() as u32;
        for i in 0..self.pipelines.len() {
            self.uniform_ui.pipelines_skip[i] = if self.pipelines[i].get_skip() {
                [1, 0, 0, 0]
            } else {
                [0, 0, 0, 0]
            };
        }

        let Some(ui_buffer) = self
            .bind_groups
            .get("ui")
            .map(|b| b.buffer.get(0))
            .flatten()
        else {
            return;
        };
        self.queue
            .write_buffer(ui_buffer, 0, bytemuck::cast_slice(&[self.uniform_ui]));

        for pipeline in self.pipelines.iter_mut() {
            pipeline.extract(sim_state, &self.queue);
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.attachments
            .insert("color".into(), Attachment::Color(ColorAttachment { view }));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let mut clear_depth = true;
        for pipeline in self.pipelines.iter() {
            if !pipeline.get_skip() {
                pipeline.render(
                    &mut encoder,
                    &self.bind_groups,
                    &self.attachments,
                    clear_depth,
                );
                clear_depth = false;
            }
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct KeyState {
    just_pressed: bool,
    just_released: bool,
    pressed: bool,
}

#[derive(Clone, Debug, Default)]
struct InputState {
    map: HashMap<KeyCode, KeyState>,
    mouse_pos: Vec2,
    mouse_moved: Vec2,
}

#[allow(dead_code)]
impl InputState {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            mouse_pos: Vec2::ZERO,
            mouse_moved: Vec2::ZERO,
        }
    }

    fn device_event(&mut self, event: &DeviceEvent) {
        match event {
            DeviceEvent::MouseMotion { delta: (x, y) } => {
                self.mouse_moved += Vec2::new(*x as f32, *y as f32)
            }
            _ => {}
        }
    }

    fn window_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(keycode),
                        repeat: false,
                        state,
                        ..
                    },
                ..
            } => {
                let keystate = self
                    .map
                    .entry(*keycode)
                    .or_insert_with(|| KeyState::default());
                keystate.just_pressed = state == &ElementState::Pressed;
                keystate.just_released = state == &ElementState::Released;
                keystate.pressed = keystate.just_pressed;
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = Vec2::new(position.x as f32, position.y as f32)
            }
            _ => {}
        }
    }

    fn update(&mut self) {
        for (_, state) in self.map.iter_mut() {
            if state.pressed && state.just_released {
                state.pressed = false;
            }
            state.just_pressed = false;
            state.just_released = false;
        }
        self.mouse_moved = Vec2::ZERO;
    }

    fn is_pressed(&self, keycode: &KeyCode) -> bool {
        self.map.get(keycode).is_some_and(|s| s.pressed)
    }
    fn is_just_pressed(&self, keycode: &KeyCode) -> bool {
        self.map.get(keycode).is_some_and(|s| s.just_pressed)
    }
    fn is_just_released(&self, keycode: &KeyCode) -> bool {
        self.map.get(keycode).is_some_and(|s| s.just_released)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SimulationState {
    pub camera_position: Vec3,
    pub camera_rotation: Quat,
    pub universe: Universe,
}

impl SimulationState {
    fn new() -> Self {
        Self {
            camera_position: Vec3::ZERO,
            camera_rotation: Quat::from_rotation_z(PI * 0.5) * Quat::from_rotation_x(PI),
            universe: simple_universe(),
        }
    }

    fn update(&mut self, time_delta: Duration, input_state: &mut InputState) {
        let dt = time_delta.as_secs_f32();

        let speed = 3.0;
        let mouse_sensitivity = Vec2::new(1.0, 1.0) * 0.1;

        let (mut yaw, mut pitch, _) = self.camera_rotation.to_euler(EulerRot::YXZ);
        pitch -= (mouse_sensitivity.y * input_state.mouse_moved.y).to_radians();
        yaw -= (mouse_sensitivity.x * input_state.mouse_moved.x).to_radians();
        pitch = pitch.clamp(-1.54, 1.54);
        let yaw_rot = Quat::from_axis_angle(Vec3::Y, yaw);
        let pitch_rot = Quat::from_axis_angle(Vec3::X, pitch);
        self.camera_rotation = yaw_rot * pitch_rot;

        let mut acceleration = Vec3::ZERO;
        if input_state.is_pressed(&KeyCode::KeyW) {
            acceleration -= Vec3::Z;
        }
        if input_state.is_pressed(&KeyCode::KeyS) {
            acceleration += Vec3::Z;
        }
        if input_state.is_pressed(&KeyCode::KeyA) {
            acceleration -= Vec3::X;
        }
        if input_state.is_pressed(&KeyCode::KeyD) {
            acceleration += Vec3::X;
        }
        if input_state.is_pressed(&KeyCode::KeyQ) {
            acceleration -= Vec3::Y;
        }
        if input_state.is_pressed(&KeyCode::KeyE) {
            acceleration += Vec3::Y;
        }
        let boost = if input_state.is_pressed(&KeyCode::ShiftLeft) {
            3.0
        } else {
            1.0
        };
        self.camera_position += self.camera_rotation * acceleration * speed * boost * dt;
    }
}

pub async fn run() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    let mut render_state = RenderState::new(&window).await;
    let mut surface_configured = false;

    // Run this even if no event has happened
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut time_simulation = Duration::ZERO;
    let mut time_frame = Instant::now();
    let mut time_accumulator = Duration::ZERO;
    let time_delta = Duration::from_millis(20);

    let mut sim_state = SimulationState::new();
    let mut input_state = InputState::new();
    let mut rendered = false;

    event_loop
        .run(move |event, control_flow| {
            // update the simulation
            let duration_frame = Instant::now() - time_frame;
            time_frame = Instant::now();
            time_accumulator += duration_frame;
            while time_accumulator >= time_delta {
                sim_state.update(time_delta, &mut input_state);

                // debug change render pass
                let mut indices = vec![];
                if input_state.is_just_pressed(&KeyCode::Digit1) {
                    indices.push(0);
                }
                if input_state.is_just_pressed(&KeyCode::Digit2) {
                    indices.push(1);
                }
                if input_state.is_just_pressed(&KeyCode::Digit3) {
                    indices.push(2);
                }
                if input_state.is_just_pressed(&KeyCode::Digit4) {
                    indices.push(3);
                }
                if input_state.is_just_pressed(&KeyCode::Digit5) {
                    indices.push(4);
                }
                if input_state.is_just_pressed(&KeyCode::Digit6) {
                    indices.push(5);
                }
                if input_state.is_just_pressed(&KeyCode::Digit7) {
                    indices.push(6);
                }
                if input_state.is_just_pressed(&KeyCode::Digit8) {
                    indices.push(7);
                }
                if input_state.is_just_pressed(&KeyCode::Digit9) {
                    indices.push(8);
                }
                if input_state.is_just_pressed(&KeyCode::Digit0) {
                    indices.push(9);
                }
                let skips: Vec<bool> = render_state
                    .pipelines
                    .iter()
                    .map(|p| p.get_skip())
                    .collect();
                for i in indices {
                    if i < render_state.pipelines.len() {
                        render_state.pipelines[i].set_skip(!skips[i]);
                    }
                }

                input_state.update();
                time_accumulator -= time_delta;
                time_simulation += time_delta;
            }

            if rendered {
                rendered = false;
                if duration_frame < Duration::from_millis(10) {
                    info!(target: "timing", "rendered in {}ms", duration_frame.as_nanos() as f64 / 1000000.0);
                }
                else if duration_frame < Duration::from_millis(100) {
                    warn!(target: "timing", "rendered in {}ms", duration_frame.as_nanos() as f64 / 1000000.0);
                }
                else {
                    error!(target: "timing", "rendered in {}ms", duration_frame.as_nanos() as f64 / 1000000.0);
                }
            }

            // render
            match event {
                Event::DeviceEvent { event, .. } => {
                    input_state.device_event(&event);
                }
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == render_state.window().id() => {
                    input_state.window_event(event);

                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::Escape),
                                    ..
                                },
                            ..
                        } => control_flow.exit(),
                        WindowEvent::Resized(physical_size) => {
                            log::info!("physical_size: {physical_size:?}");
                            surface_configured = true;
                            render_state.resize(*physical_size);

                            render_state
                                .window()
                                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                                .unwrap();
                        }
                        WindowEvent::RedrawRequested => {
                            render_state.window().request_redraw();

                            if !surface_configured {
                                return;
                            }

                            render_state.extract(&mut sim_state);
                            match render_state.render() {
                                Ok(_) => {
                                    rendered = true;
                                }
                                // Reconfigure the surface if it's lost or outdated
                                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                                    render_state.resize(render_state.size)
                                }
                                // The system is out of memory, we should probably quit
                                Err(wgpu::SurfaceError::OutOfMemory) => {
                                    log::error!("OutOfMemory");
                                    control_flow.exit();
                                }
                                // This happens when the a frame takes too long to present
                                Err(wgpu::SurfaceError::Timeout) => {
                                    log::warn!("Surface timeout")
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        })
        .unwrap();
}

fn main() {
    pollster::block_on(run());
}
