use super::*;

// Real texture sampling/readback catches channel order and format-only changes.
#[test]
#[ignore = "requires a GPU adapter; run explicitly with --ignored"]
fn gpu_upload_preserves_rgb_rgba_bgra_colors_and_format_changes() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let target_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut renderer = WgpuGameRenderer::new(&device, target_format);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("color_test_target"),
        size: wgpu::Extent3d {
            width: 2,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("color_test_readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let expected = [255, 0, 0, 255, 0, 0, 255, 255];
    for (format, bytes) in [
        (PixelFormat::Bgra8888, &[0, 0, 255, 255, 255, 0, 0, 255][..]),
        (PixelFormat::Rgba8888, &expected[..]),
        (PixelFormat::Rgb24, &[255, 0, 0, 0, 0, 255][..]),
        (PixelFormat::Bgra8888, &[0, 0, 255, 255, 255, 0, 0, 255][..]),
    ] {
        renderer.upload_frame(&device, &queue, bytes, 2, 1, format);
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("color_test_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            renderer.draw(&queue, &mut pass, 2, 1, 0);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 2,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.map_async(wgpu::MapMode::Read, .., move |result| {
            tx.send(result).unwrap();
        });
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(10))
            .unwrap()
            .unwrap();
        assert_eq!(&readback.get_mapped_range(..)[..8], &expected, "{format:?}");
        readback.unmap();
    }
}
