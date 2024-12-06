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

@group(1) @binding(0)
var diffuse_texture: texture_2d<f32>;
@group(1) @binding(1)
var diffuse_sampler: sampler;

struct InstanceInput {
    @location(5) pos: vec3<f32>,
    @location(6) id: u32,
};

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) id: u32,
}

@vertex
fn vs_main(model: VertexInput, instance: InstanceInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = global.clip_from_world * vec4<f32>(model.position + instance.pos.xyz, 1.0);
    out.id = instance.id;
    out.uv = model.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let offset = vec2<f32>(
        f32(in.id % 16),
        f32(u32(in.id / 16)),
    );
    return textureSample(
        diffuse_texture,
        diffuse_sampler,
        (offset + in.uv) / 16.0
    );
}
