struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_y: f32,
    @location(1) normal: vec3<f32>,
};

struct FragOutput {
    @location(0) color: vec4<f32>,
};

struct HeightMap {
    data: array<vec4<f32>, 73>,
}

struct ChunkData {
    rel_pos: vec4<f32>, // アライメントを揃えるため vec4
    lod_level: u32,
    _padding0: u32, // アライメントのためのパディング
    _padding1: u32,
    _padding2: u32,
    height_map: HeightMap,
}

@group(0) @binding(0) var<storage, read> all_chunks: array<ChunkData>;
@group(0) @binding(1) var<uniform> vp_matrix: mat4x4<f32>;

fn get_h(chunk_id: u32, i: u32) -> f32 {
    return all_chunks[chunk_id].height_map.data[i >> 2u][i & 3u];
}

@vertex
fn vs_main(
    @builtin(vertex_index) index: u32,
    @builtin(instance_index) chunk_id: u32// インスタンスID
) -> VertexOutput {
    let x = index % 17u;
    let z = index / 17u;
    let h = get_h(chunk_id, index);

    // 現在処理しているチャンクのデータを配列から引っ張る
    let chunk = all_chunks[chunk_id];
    let scale = f32(1u << chunk.lod_level);

    // 境界を1頂点クランプした添字で隣接高さ取得
    let xl = select(x - 1u, x, x == 0u);
    let xr = select(x + 1u, x, x == 16u);
    let zu = select(z - 1u, z, z == 0u);
    let zd = select(z + 1u, z, z == 16u);

    let hL = get_h(chunk_id, z * 17u + xl);
    let hR = get_h(chunk_id, z * 17u + xr);
    let hU = get_h(chunk_id, zu * 17u + x);
    let hD = get_h(chunk_id, zd * 17u + x);

    // 解析的に法線を算出
    let inv_scale = 1.0 / (2.0 * scale);
    let normal = normalize(vec3<f32>(
        (hL - hR) * inv_scale,
        1.0,
        (hU - hD) * inv_scale,
    ));

    var out: VertexOutput;
    out.position = vp_matrix * vec4<f32>(
        chunk.rel_pos.x + f32(x) * scale,
        chunk.rel_pos.y + h,
        chunk.rel_pos.z + f32(z) * scale,
        1.0,
    );
    out.world_y = chunk.rel_pos.y + h;
    out.normal = normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragOutput {
    let h = in.world_y;

    var base = vec3<f32>(0.30, 0.68, 0.20);

    let light = normalize(vec3<f32>(1.0, 2.0, 0.8));
    let diff = max(dot(in.normal, light), 0.0);
    let lit = base * (0.75 * diff);

    var out: FragOutput;
    out.color = vec4<f32>(lit, 1.0);
    return out;
}