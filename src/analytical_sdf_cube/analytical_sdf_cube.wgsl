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

struct CubeSDF {
    min: vec4<f32>,
    max: vec4<f32>,
    color: vec4<f32>,
};

// returns the intersections between a ray and a cuboid (axis aligned, not rotated)
fn analytical_cube_ray_intersection(
    ray_origin: vec3<f32>,
    ray_direction: vec3<f32>,
    min: vec3<f32>,
    max: vec3<f32>,
) -> vec2<f32> {
    let v1 = (min.x - ray_origin.x) / ray_direction.x;
    let v2 = (max.x - ray_origin.x) / ray_direction.x;
    let v3 = (min.y - ray_origin.y) / ray_direction.y;
    let v4 = (max.y - ray_origin.y) / ray_direction.y;
    let v5 = (min.z - ray_origin.z) / ray_direction.z;
    let v6 = (max.z - ray_origin.z) / ray_direction.z;
    let v7 = max(max(min(v1, v2), min(v3, v4)), min(v5, v6));
    let v8 = min(min(max(v1, v2), max(v3, v4)), max(v5, v6));
    if v8 < 0.0 || v7 > v8 {
        return vec2(0.0);
    }

    return vec2(v7, v8);
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var cube_sdf: CubeSDF;
    cube_sdf.min = vec4<f32>(1.0, 1.0, 1.0, 0.0);
    cube_sdf.max = vec4<f32>(2.0, 2.0, 2.0, 0.0);
    cube_sdf.color = vec4<f32>(0.4, 0.0, 1.0, 0.0);

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
    let int = analytical_cube_ray_intersection(
        ray_origin,
        ray_direction,
        cube_sdf.min.xyz,
        cube_sdf.max.xyz,
    );
    if int.x <= 0.0 {
        // no intersection
        return FragmentOutput(vec4<f32>(0.0), 1.0);
    } else {
        // int.x is the intersection distance
        let intersection_point = ray_origin + ray_direction * int.x;
        let clip = global.clip_from_world * vec4<f32>(intersection_point, 1.0);
        let depth = max(0.1, clip.z / clip.w);
        return FragmentOutput(cube_sdf.color, depth);
    }

    return FragmentOutput(vec4<f32>(0.0), 1.0);
}
