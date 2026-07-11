struct VpUniform {
    vp: mat4x4<f32>,
};

struct ModelUniform {
    model: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> vp_uniform: VpUniform;

@group(1) @binding(0) var<uniform> model_uniform: ModelUniform;

@group(2) @binding(0) var tex: texture_2d<f32>;
@group(2) @binding(1) var samp: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = model_uniform.model * vec4<f32>(in.position, 1.0);
    out.clip_position = vp_uniform.vp * world_pos;
    out.normal = (model_uniform.model * vec4<f32>(in.normal, 0.0)).xyz;
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let base_color = textureSample(tex, samp, in.uv);

    // 簡易ライティング(固定方向ライト + アンビエント)
    let light_dir = normalize(vec3<f32>(0.4, 1.0, 0.3));
    let n = normalize(in.normal);
    let diffuse = max(dot(n, light_dir), 0.0);
    let ambient = 0.3;
    let lighting = ambient + diffuse * 0.7;

    let color = base_color.rgb * lighting;
    return vec4<f32>(color, base_color.a);
}