use wgpu::VertexBufferLayout;

/// `cast_slice` による GPU 転送を前提としているため
/// `Pod + Zeroable + Copy` を要求する。
pub trait VertexLayout: bytemuck::Pod + bytemuck::Zeroable + Copy {
    fn layout() -> VertexBufferLayout<'static>;
}

/// 頂点構造体に `VertexLayout` を実装するマクロ。
///
/// # 使用例
/// ```rust
/// #[repr(C)]
/// #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
/// pub struct MyVertex {
///     pub position: [f32; 3],
///     pub uv:       [f32; 2],
/// }
///
/// impl_vertex_layout!(MyVertex,
///     0 => Float32x3, 0,
///     1 => Float32x2, 12,
/// );
/// ```
#[macro_export]
macro_rules! impl_vertex_layout {
    (
        $struct_name:ty,
        $( $location:expr => $format:ident , $offset:expr ),* $(,)?
    ) => {
        impl $crate::render::vertex::VertexLayout for $struct_name {
            fn layout() -> wgpu::VertexBufferLayout<'static> {
                wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<$struct_name>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        $(
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::$format,
                                offset: $offset as wgpu::BufferAddress,
                                shader_location: $location,
                            },
                        )*
                    ],
                }
            }
        }
    };
}
