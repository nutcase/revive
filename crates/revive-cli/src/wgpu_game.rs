use revive_core::PixelFormat;

#[cfg(test)]
#[path = "wgpu_game_tests.rs"]
mod tests;

const GAME_SHADER: &str = r#"
struct VertexIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.pos = vec4<f32>(in.pos, 0.0, 1.0);
    out.uv = in.uv;
    return out;
}

@group(0) @binding(0) var game_texture: texture_2d<f32>;
@group(0) @binding(1) var game_sampler: sampler;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return textureSample(game_texture, game_sampler, in.uv);
}

@fragment
fn fs_rgb(in: VertexOut) -> @location(0) vec4<f32> {
    let size = textureDimensions(game_texture);
    let logical_size = vec2<u32>(size.x / 3u, size.y);
    let pixel = min(vec2<u32>(max(in.uv, vec2<f32>(0.0)) * vec2<f32>(logical_size)), logical_size - vec2<u32>(1u));
    let base = vec2<i32>(i32(pixel.x * 3u), i32(pixel.y));
    let c = vec3<f32>(textureLoad(game_texture, base, 0).r,
        textureLoad(game_texture, base + vec2<i32>(1, 0), 0).r,
        textureLoad(game_texture, base + vec2<i32>(2, 0), 0).r);
    // Match Rgba8UnormSrgb sampling; alpha is opaque for RGB24.
    let linear = select(pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), c / 12.92, c <= vec3<f32>(0.04045));
    return vec4<f32>(linear, 1.0);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    uv: [f32; 2],
}

const VERTEX_ATTRIBUTES: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2];

pub(crate) struct WgpuGameRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    texture: Option<wgpu::Texture>,
    bind_group: Option<wgpu::BindGroup>,
    texture_size: (usize, usize),
    texture_format: wgpu::TextureFormat,
    rgb_pipeline: wgpu::RenderPipeline,
}

impl WgpuGameRenderer {
    pub(crate) fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("game_frame_shader"),
            source: wgpu::ShaderSource::Wgsl(GAME_SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("game_frame_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("game_frame_pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let create_pipeline = |entry| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("game_frame_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &VERTEX_ATTRIBUTES,
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipeline = create_pipeline("fs_main");
        let rgb_pipeline = create_pipeline("fs_rgb");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("game_frame_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("game_frame_vertex_buffer"),
            size: (std::mem::size_of::<Vertex>() * 6) as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            vertex_buffer,
            texture: None,
            bind_group: None,
            texture_size: (0, 0),
            texture_format: wgpu::TextureFormat::Rgba8UnormSrgb,
            rgb_pipeline,
        }
    }

    pub(crate) fn upload_frame(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        data: &[u8],
        width: usize,
        height: usize,
        format: PixelFormat,
    ) {
        let texture_format = match format {
            PixelFormat::Bgra8888 => wgpu::TextureFormat::Bgra8UnormSrgb,
            PixelFormat::Rgb24 => wgpu::TextureFormat::R8Unorm,
            PixelFormat::Rgba8888 => wgpu::TextureFormat::Rgba8UnormSrgb,
        };
        if (width, height) != self.texture_size || texture_format != self.texture_format {
            self.create_texture(device, width, height, texture_format);
        }

        // Packed RGB bytes are uploaded directly. Unpack and sRGB decoding
        // happen in the fragment shader, without changing core/state layouts.
        let packed_rgb = format == PixelFormat::Rgb24;
        let upload_width = width * if packed_rgb { 3 } else { 1 };
        let bytes_per_row = width * if packed_rgb { 3 } else { 4 };

        if let Some(texture) = &self.texture {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row as u32),
                    rows_per_image: Some(height as u32),
                },
                wgpu::Extent3d {
                    width: upload_width as u32,
                    height: height as u32,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub(crate) fn draw(
        &self,
        queue: &wgpu::Queue,
        render_pass: &mut wgpu::RenderPass<'_>,
        target_width: u32,
        target_height: u32,
        panel_width: u32,
    ) {
        let Some(bind_group) = &self.bind_group else {
            return;
        };
        let game_width = target_width.saturating_sub(panel_width);
        if game_width == 0 || target_height == 0 || self.texture_size.0 == 0 {
            return;
        }

        let vertices = fitted_vertices(
            game_width,
            target_height,
            target_width,
            self.texture_size.0 as u32,
            self.texture_size.1 as u32,
        );
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        render_pass.set_pipeline(if self.texture_format == wgpu::TextureFormat::R8Unorm {
            &self.rgb_pipeline
        } else {
            &self.pipeline
        });
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..6, 0..1);
    }

    fn create_texture(
        &mut self,
        device: &wgpu::Device,
        width: usize,
        height: usize,
        format: wgpu::TextureFormat,
    ) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("game_frame_texture"),
            size: wgpu::Extent3d {
                width: (width
                    * if format == wgpu::TextureFormat::R8Unorm {
                        3
                    } else {
                        1
                    }) as u32,
                height: height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("game_frame_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        self.texture = Some(texture);
        self.bind_group = Some(bind_group);
        self.texture_size = (width, height);
        self.texture_format = format;
    }
}

fn fitted_vertices(
    game_width: u32,
    target_height: u32,
    target_width: u32,
    texture_width: u32,
    texture_height: u32,
) -> [Vertex; 6] {
    let scale = (game_width as f32 / texture_width as f32)
        .min(target_height as f32 / texture_height as f32);
    let draw_w = texture_width as f32 * scale;
    let draw_h = texture_height as f32 * scale;
    let left = (game_width as f32 - draw_w) * 0.5;
    let top = (target_height as f32 - draw_h) * 0.5;
    let right = left + draw_w;
    let bottom = top + draw_h;

    let x0 = pixel_x_to_ndc(left, target_width);
    let x1 = pixel_x_to_ndc(right, target_width);
    let y0 = pixel_y_to_ndc(top, target_height);
    let y1 = pixel_y_to_ndc(bottom, target_height);

    [
        Vertex {
            pos: [x0, y0],
            uv: [0.0, 0.0],
        },
        Vertex {
            pos: [x0, y1],
            uv: [0.0, 1.0],
        },
        Vertex {
            pos: [x1, y1],
            uv: [1.0, 1.0],
        },
        Vertex {
            pos: [x0, y0],
            uv: [0.0, 0.0],
        },
        Vertex {
            pos: [x1, y1],
            uv: [1.0, 1.0],
        },
        Vertex {
            pos: [x1, y0],
            uv: [1.0, 0.0],
        },
    ]
}

fn pixel_x_to_ndc(x: f32, target_width: u32) -> f32 {
    x / target_width as f32 * 2.0 - 1.0
}

fn pixel_y_to_ndc(y: f32, target_height: u32) -> f32 {
    1.0 - y / target_height as f32 * 2.0
}
