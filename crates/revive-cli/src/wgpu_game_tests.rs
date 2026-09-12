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
            width: 257,
            height: 2,
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
        size: 1280 * 2,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let expected: Vec<u8> = (0..514)
        .flat_map(|i| [i as u8, (i * 37) as u8, (i * 13 + 10) as u8, 255])
        .collect();
    let rgb: Vec<_> = expected
        .chunks_exact(4)
        .flat_map(|p| p[..3].iter().copied())
        .collect();
    let bgra: Vec<_> = expected
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0], p[3]])
        .collect();
    for (format, bytes) in [
        (PixelFormat::Bgra8888, &bgra[..]),
        (PixelFormat::Rgba8888, &expected[..]),
        (PixelFormat::Rgb24, &rgb[..]),
        (PixelFormat::Bgra8888, &bgra[..]),
    ] {
        renderer.upload_frame(&device, &queue, bytes, 257, 2, format);
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
            renderer.draw(&queue, &mut pass, 257, 2, 0);
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
                    bytes_per_row: Some(1280),
                    rows_per_image: Some(2),
                },
            },
            wgpu::Extent3d {
                width: 257,
                height: 2,
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
        {
            let mapped = readback.get_mapped_range(..);
            for y in 0..2 {
                let actual = &mapped[y * 1280..y * 1280 + 257 * 4];
                let expected = &expected[y * 257 * 4..(y + 1) * 257 * 4];
                for (i, (&a, &b)) in actual.iter().zip(expected).enumerate() {
                    assert!(
                        a.abs_diff(b) <= 1,
                        "{format:?} row={y} byte={i}: {a} != {b}"
                    );
                }
            }
        }
        readback.unmap();
    }
}

#[test]
#[ignore = "manual GPU release benchmark"]
fn benchmark_rgb_upload_and_render() {
    use std::{
        hint::black_box,
        time::{Duration, Instant},
    };
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .unwrap();
    eprintln!("GPU: {:?}", adapter.get_info());
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).unwrap();
    let rgb: Vec<u8> = (0..320 * 240 * 3).map(|i| (i * 37) as u8).collect();
    let mut rgba = vec![0u8; 320 * 240 * 4];
    for (width, height) in [(320, 240), (1280, 960)] {
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("rgb_benchmark"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        for packed in [false, true] {
            let mut renderer = WgpuGameRenderer::new(&device, wgpu::TextureFormat::Rgba8UnormSrgb);
            let mut upload_time = Duration::ZERO;
            let mut total_time = Duration::ZERO;
            for frame in 0..120 {
                let start = Instant::now();
                if packed {
                    renderer.upload_frame(
                        &device,
                        &queue,
                        black_box(&rgb),
                        320,
                        240,
                        PixelFormat::Rgb24,
                    );
                } else {
                    for (src, dst) in black_box(&rgb)
                        .chunks_exact(3)
                        .zip(rgba.chunks_exact_mut(4))
                    {
                        dst[..3].copy_from_slice(src);
                        dst[3] = 255;
                    }
                    renderer.upload_frame(&device, &queue, &rgba, 320, 240, PixelFormat::Rgba8888);
                }
                let uploaded = start.elapsed();
                let mut encoder = device.create_command_encoder(&Default::default());
                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
                    renderer.draw(&queue, &mut pass, width, height, 0);
                }
                queue.submit([encoder.finish()]);
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: Some(Duration::from_secs(10)),
                    })
                    .unwrap();
                if frame >= 20 {
                    upload_time += uploaded;
                    total_time += start.elapsed();
                }
            }
            eprintln!("RGB target={width}x{height} packed={packed}: upload={:?}/frame synchronized_total={:?}/frame", upload_time/100, total_time/100);
        }
    }
}
