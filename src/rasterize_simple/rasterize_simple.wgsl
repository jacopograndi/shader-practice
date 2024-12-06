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

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    // translation defined here instead of a buffer
    let translation = vec3<f32>(1.0, 1.0, 1.0);

    var out: VertexOutput;
    out.color = vec4<f32>(in.color, 1.0);
    out.clip_position = global.clip_from_world * vec4<f32>(in.position + translation, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
