struct VertexInput {
    @location(0) index: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

struct FragOutput {
    @location(0) color: vec4<f32>,
};

@group(0) @binding(0)
var<storage> height_map: array<f32>;

@group(0) @binding(1)
var<uniform> chunk_pos: vec2<u32>;

@group(0) @binding(2)
var<uniform> vp_matrix: mat4x4<f32>;

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    let x = v.index & 0x0Fu;
    let z = v.index >> 4u;

    let h = height_map[v.index];

    let world_x = f32(chunk_pos.x * 16u + x);
    let world_z = f32(chunk_pos.y * 16u + z);

    let world_pos = vec3<f32>(world_x, h, world_z);

    var out: VertexOutput;
    out.position = vp_matrix * vec4<f32>(world_pos, 1.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragOutput {
    var out: FragOutput;
    out.color = vec4<f32>(0.3, 0.7, 0.2, 1.0); // 仮の地面色
    return out;
}