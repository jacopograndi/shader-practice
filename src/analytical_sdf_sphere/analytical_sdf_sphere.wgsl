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

struct SphereSDF {
    translation: vec4<f32>,
    color: vec4<f32>,
    radius: f32,
    padding: array<f32, 3>,
};

fn analytical_sphere_ray_intersection(
    ray_origin: vec3<f32>,
    ray_direction: vec3<f32>,
    center: vec3<f32>,
    radius: f32,
) -> vec2<f32> {
    let diff = ray_origin - center;
    let b = dot(diff, ray_direction);
    let c = dot(diff, diff) - radius * radius;
    let h = b * b - c;
    if h < 0.0 { return vec2(-1.0); }
    let root = sqrt(h);
    return vec2<f32>(-b - root, -b + root);
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var sphere_sdf: SphereSDF;
    sphere_sdf.translation = vec4<f32>(1.0, 0.25, 1.0, 0.0);
    sphere_sdf.color = vec4<f32>(0.0, 0.5, 0.0, 1.0);
    sphere_sdf.radius = 1.0;

    // convert uv from (0.0..1.0, 0.0..1.0) to (0..viewport_size.x, 0..viewport_size.y)
    let px = in.uv * global.viewport_size.xy;

    // cast a ray from the camera origin that passes through the current pixel
    var clip_space = vec2(1.0, -1.0) * (in.uv * 2.0 - 1.0);
    let clip_far = global.world_from_clip * vec4(clip_space.x, clip_space.y, 1.0, 1.0);
    let clip_near = global.world_from_clip * vec4(clip_space.x, clip_space.y, 0.1, 1.0);
    let world_far = clip_far.xyz / clip_far.w;
    let world_near = clip_near.xyz / clip_near.w;
    let dir = normalize(world_far - world_near);

    let ray_origin = global.view_world_position.xyz;
    let ray_direction = dir.xyz;
    let int = analytical_sphere_ray_intersection(
        ray_origin,
        ray_direction,
        sphere_sdf.translation.xyz,
        sphere_sdf.radius
    );
    if int.y < 0.0 {
        // no intersection
        return FragmentOutput(vec4<f32>(0.0), 1.0);
    } else if int.x < 0.0 {
        // inside the sphere
        return FragmentOutput(sphere_sdf.color, 0.0);
    } else {
        // int.x is the intersection distance
        let intersection_point = ray_origin + ray_direction * int.x;
        let clip = global.clip_from_world * vec4<f32>(intersection_point, 1.0);
        let depth = max(0.1, clip.z / clip.w);
        return FragmentOutput(sphere_sdf.color, depth);
    }

    return FragmentOutput(vec4<f32>(0.0), 1.0);
}
