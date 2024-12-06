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

@group(0) @binding(0)
var depth_texture: texture_2d<f32>;
@group(0) @binding(1)
var depth_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let near = 0.1;
    let far = 1000.0;
    let depth = textureSample(depth_texture, depth_sampler, in.uv).x;
    let r = (2.0 * near) / (far + near - depth * (far - near));
    return vec4<f32>(vec3<f32>(r), 1.0);
}
