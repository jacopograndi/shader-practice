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

@group(2) @binding(0)
var<storage, read> chunk: array<u32>;

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

fn in_chunk_bounds(v: vec3f, offset: vec3f, size: vec3f) -> bool {
    let x = v.x >= offset.x && v.x < offset.x + size.x;
    let y = v.y >= offset.y && v.y < offset.y + size.y;
    let z = v.z >= offset.z && v.z < offset.z + size.z;
    return x && y && z;
}

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    // cast a ray from the camera origin that passes through the current pixel
    var clip_space = vec2(1.0, -1.0) * (in.uv * 2.0 - 1.0);
    let clip_far = global.world_from_clip * vec4(clip_space.x, clip_space.y, 1.0, 1.0);
    let clip_near = global.world_from_clip * vec4(clip_space.x, clip_space.y, 0.1, 1.0);
    let world_far = clip_far.xyz / clip_far.w;
    let world_near = clip_near.xyz / clip_near.w;
    let dir = normalize(world_far - world_near);
    let dir_long = world_far - world_near;

    // cube chunk intersection
    let chunk_pos = vec3<f32>(0.0, 0.0, 0.0);
    var ray_origin = global.view_world_position.xyz;
    let ray_direction = dir.xyz;
    if !in_chunk_bounds(ray_origin, chunk_pos, vec3<f32>(32.0)) {
        let epsilon = vec3<f32>(0.00001);
        let int = analytical_cube_ray_intersection(
            ray_origin,
            ray_direction,
            chunk_pos,
            chunk_pos + vec3<f32>(32.0)
        );
        if int.x <= 0.0 {
            return FragmentOutput(vec4<f32>(0.0), 1.0);
        } else {
            ray_origin += ray_direction * int.x;
        }
    }

    // raycast inside the chunk using DDA (digital differential analyzer)
    var side = -1.0;
    var voxel_id = 0u;
    var map = floor(ray_origin);
    let delta_dist = 1.0 / abs(ray_direction);
    let s = step(vec3<f32>(0.0), ray_direction);
    let step_dir = 2.0 * s - 1.0;
    var side_dist = (s - step_dir * fract(ray_origin)) * delta_dist;
    var i = 0u;
    for (; i < 50u; i++) {
        let conds = step(side_dist.xxyy, side_dist.yzzx);
        var cases = vec3<f32>(0.0);
        cases.x = conds.x * conds.y;
        cases.y = (1. - cases.x) * conds.z * conds.w;
        cases.z = (1. - cases.x) * (1. - cases.y);
        side_dist += max((2.0 * cases - 1.0) * delta_dist, vec3<f32>(0.0));
        map += cases * step_dir;
        if !in_chunk_bounds(map, chunk_pos, vec3<f32>(32.0)) {
            break;
        }

        let idx = u32(map.x) * (32u * 32u) + u32(map.y) * 32u + u32(map.z);
        voxel_id = chunk[idx];
        if voxel_id > 0 {
            side = cases.y + 2. * cases.z;
            break;
        }
    }

    if side == -1 {
        // no hit
        return FragmentOutput(vec4<f32>(0.0), 1.0);
    }

    // calculate the normal for ambient occlusion
    var normal = -vec3<f32>(f32(side == 0), f32(side == 1), f32(side == 2)) * step_dir;

    // find intersection point by intersecting with the face's plane
    var n = vec3<f32>(f32(side == 0), f32(side == 1), f32(side == 2));
    let p = map + 0.5 - step_dir * 0.5;
    let t = (dot(n, p - ray_origin)) / dot(n, ray_direction);
    let hit = ray_origin + ray_direction * t;
    let uvw = hit - map;
    var uv = vec2<f32>(0.0);
    if side == 0 {
        uv = uvw.yz;
    } else if side == 1 {
        uv = uvw.zx;
    } else if side == 2 {
        uv = uvw.xy;
    }

    // apply texture
    let offset = vec2<f32>(
        f32(voxel_id % 16),
        f32(u32(voxel_id / 16)),
    );
    let color = textureSample(
        diffuse_texture,
        diffuse_sampler,
        (offset + uv) / 16.0
    );

    let clip = global.clip_from_world * vec4<f32>(hit, 1.0);
    let depth = max(0.1, clip.z / clip.w);
    return FragmentOutput(color, depth);
}
