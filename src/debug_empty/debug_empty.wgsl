struct GlobalUniform {
    viewport_size: vec4<f32>,
    view_world_position: vec4<f32>,
    world_from_clip: mat4x4<f32>,
    clip_from_world: mat4x4<f32>,
    view_from_clip: mat4x4<f32>,
    clip_from_view: mat4x4<f32>,
    view_from_world: mat4x4<f32>,
    world_from_view: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> global: GlobalUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32(vertex_index >> 1u), f32(vertex_index & 1u)) * 2.0;
    let clip_position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return VertexOutput(clip_position, uv);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}
