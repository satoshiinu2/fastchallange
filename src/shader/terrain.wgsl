struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};
struct FragOutput {
    @location(0) color: vec4<f32>,
};
struct HeightMap {
    data: array<vec4<f32>, 64>,
}

@group(0) @binding(0)
var<uniform> height_map: HeightMap;
@group(0) @binding(1)
var<uniform> rel_pos: vec3<f32>;
@group(0) @binding(2)
var<uniform> vp_matrix: mat4x4<f32>;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let x = index % 16u;
    let z = index / 16u;
    let h = height_map.data[index / 4u][index % 4u];
    let world_x = rel_pos.x + f32(x);
    let world_y = rel_pos.y + h;
    let world_z = rel_pos.z + f32(z);
    var out: VertexOutput;
    out.position = vp_matrix * vec4<f32>(world_x, world_y, world_z, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragOutput {
    var out: FragOutput;
    out.color = vec4<f32>(0.3, 0.7, 0.2, 1.0);
    return out;
}