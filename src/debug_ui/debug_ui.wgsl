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

// for stride reason, wrap u32 to set correct array stride at 16, otherwise it's 4
struct wrapped_u32 {
  @size(16) elem: u32
}
struct UiUniform {
    pipelines_skip: array<wrapped_u32, 256>,
    pipelines_num: u32,
};
@group(1) @binding(0)
var<uniform> ui: UiUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let uv = vec2<f32>(f32(vertex_index >> 1u), f32(vertex_index & 1u)) * 2.0;
    let clip_position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    return VertexOutput(clip_position, uv);
}

fn in_box(pos: vec2<f32>, size: vec2<f32>, t: vec2<f32>) -> bool {
    return pos.x > t.x && pos.x < t.x + size.x && //
        pos.y > t.y && pos.y < t.y + size.y;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // convert uv from (0.0..1.0, 0.0..1.0) to (0..viewport_size.x, 0..viewport_size.y)
    let px = in.uv * global.viewport_size.xy;

    for (var i = 0u; i < ui.pipelines_num; i++) {
        if in_box(vec2(100.0 + f32(i) * 25, 200.0), vec2(20.0, 20.0), px) {
            if ui.pipelines_skip[i].elem == 1 {
                return vec4<f32>(1.0, 0.0, 0.0, 1.0);
            } else {
                return vec4<f32>(0.0, 0.5, 0.0, 1.0);
            }
        }
    }

    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
