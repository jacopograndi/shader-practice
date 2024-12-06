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

fn sdf_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sdf_plane(p: vec3<f32>, n: vec3<f32>, h: f32) -> f32 {
    return dot(p, n) + h;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    // convert uv from (0.0..1.0, 0.0..1.0) to (0..viewport_size.x, 0..viewport_size.y)
    let px = in.uv * global.viewport_size.xy;

    // cast a ray from the camera origin that passes through the current pixel
    var clip_space = vec2(1.0, -1.0) * (in.uv * 2.0 - 1.0);
    let clip_far = global.world_from_clip * vec4(clip_space.x, clip_space.y, 1.0, 1.0);
    let clip_near = global.world_from_clip * vec4(clip_space.x, clip_space.y, 0.1, 1.0);
    let world_far = clip_far.xyz / clip_far.w;
    let world_near = clip_near.xyz / clip_near.w;
    let dir = normalize(world_far - world_near);

    // raycast
    var out: FragmentOutput;
    out.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    out.depth = 1.0;
    for (var i = 0u; i < 256u; i += 1u) {
        let travel_distance = f32(i) * 0.05;
        let sample = global.view_world_position.xyz + dir.xyz * travel_distance;
        var distance = 10000.0;
        let fog = 1.0 - f32(i) / 256.0;

        distance = min(distance, sdf_plane(sample, vec3<f32>(0.0, 1.0, 0.0), 0.0));
        if distance < 0.0 {
            out.color = vec4<f32>(1.0 - fog, 1.0, 1.0, 1.0);
            let clip = global.clip_from_world * vec4<f32>(sample, 1.0);
            let depth = clip.z / clip.w;
            out.depth = max(0.1, depth);
            break;
        }
        distance = min(distance, sdf_sphere(sample + vec3<f32>(0.0, 0.0, -1.0), 0.3));
        if distance < 0.0 {
            out.color = vec4<f32>(1.0, 1.0 - fog, 1.0 - fog, 1.0);
            let clip = global.clip_from_world * vec4<f32>(sample, 1.0);
            let depth = clip.z / clip.w;
            out.depth = max(0.1, depth);
            break;
        }
        distance = min(distance, sdf_sphere(sample + vec3<f32>(3.0, 0.0, 0.0), 1.0));
        distance = min(distance, sdf_sphere(sample + vec3<f32>(0.0, 0.0, 10.0), 1.0));
        distance = min(distance, sdf_sphere(sample + vec3<f32>(0.0, 3.0, -3.0), 1.0));
        distance = min(distance, sdf_sphere(sample + vec3<f32>(0.0, -3.0, 3.0), 1.0));
        distance = min(distance, sdf_sphere(sample + vec3<f32>(3.0, 0.0, -3.0), 1.0));
        distance = min(distance, sdf_sphere(sample + vec3<f32>(-3.0, 0.0, 3.0), 1.0));
        if distance < 0.0 {
            out.color = vec4<f32>(1.0, 1.0, 1.0 - fog, 1.0);
            let clip = global.clip_from_world * vec4<f32>(sample, 1.0);
            let depth = clip.z / clip.w;
            out.depth = max(0.1, depth);
            break;
        }
    }

    return out;
}
