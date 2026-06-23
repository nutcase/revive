use std::error::Error;
use std::io;

use egui_wgpu::{Renderer, ScreenDescriptor};
use revive_core::CoreInstance;
use sdl3::video::Window;

use crate::wgpu_game::WgpuGameRenderer;

const DEFAULT_SCALE: u32 = 3;

pub(crate) struct UiRenderData<'a> {
    pub(crate) textures_delta: &'a egui::TexturesDelta,
    pub(crate) primitives: &'a [egui::ClippedPrimitive],
    pub(crate) pixels_per_point: f32,
}

pub(crate) struct RenderState {
    #[allow(dead_code)]
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    game_renderer: WgpuGameRenderer,
    egui_renderer: Renderer,
    texture_size: (usize, usize),
    game_w: u32,
    game_h: u32,
    panel_width_px: u32,
}

impl RenderState {
    pub(crate) fn initial_window_size(frame_width: usize, frame_height: usize) -> (u32, u32) {
        (
            frame_width as u32 * DEFAULT_SCALE,
            frame_height as u32 * DEFAULT_SCALE,
        )
    }

    pub(crate) fn new(
        window: &Window,
        frame_width: usize,
        frame_height: usize,
        panel_width_px: u32,
    ) -> Result<Self, Box<dyn Error>> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = create_surface(&instance, window)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))?;
        let adapter_info = adapter.get_info();
        println!(
            "Renderer    : wgpu {:?} ({})",
            adapter_info.backend, adapter_info.name
        );
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("revive_wgpu_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                trace: wgpu::Trace::Off,
            }))?;
        let surface_config = surface_config_for(window, &surface, &adapter)?;
        surface.configure(&device, &surface_config);
        let game_renderer = WgpuGameRenderer::new(&device, surface_config.format);
        let egui_renderer = Renderer::new(
            &device,
            surface_config.format,
            egui_wgpu::RendererOptions::default(),
        );
        let (game_w, game_h) = Self::initial_window_size(frame_width, frame_height);

        Ok(Self {
            instance,
            surface,
            device,
            queue,
            surface_config,
            game_renderer,
            egui_renderer,
            texture_size: (frame_width, frame_height),
            game_w,
            game_h,
            panel_width_px,
        })
    }

    pub(crate) fn panel_width_px(&self) -> u32 {
        self.panel_width_px
    }

    pub(crate) fn set_panel_width_px(&mut self, panel_width_px: u32) {
        self.panel_width_px = panel_width_px;
    }

    pub(crate) fn resize_window_for_panel(&mut self, window: &mut Window, panel_visible: bool) {
        let _ = window.set_size(self.window_width(panel_visible), self.game_h);
        let _ = window.sync();
        self.configure_surface_for_window(window);
    }

    pub(crate) fn sync_surface_size(&mut self, window: &Window) {
        let (width, height) = window.size_in_pixels();
        if width != self.surface_config.width || height != self.surface_config.height {
            self.configure_surface_for_window(window);
        }
    }

    pub(crate) fn upload_core_frame(
        &mut self,
        core: &mut CoreInstance,
        window: &mut Window,
        panel_visible: bool,
    ) {
        let frame = core.frame();
        if (frame.width, frame.height) != self.texture_size {
            self.texture_size = (frame.width, frame.height);
            self.game_w = frame.width as u32 * DEFAULT_SCALE;
            self.game_h = frame.height as u32 * DEFAULT_SCALE;
            self.resize_window_for_panel(window, panel_visible);
        }
        self.game_renderer.upload_frame(
            &self.device,
            &self.queue,
            frame.data,
            frame.width,
            frame.height,
            frame.format,
        );
    }

    pub(crate) fn present_frame(
        &mut self,
        window: &Window,
        panel_visible: bool,
        ui: Option<UiRenderData<'_>>,
    ) -> Result<(), Box<dyn Error>> {
        self.sync_surface_size(window);
        if self.surface_config.width == 0 || self.surface_config.height == 0 {
            return Ok(());
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("revive_frame_encoder"),
            });
        let user_cmd_bufs = if let Some(ui) = ui.as_ref() {
            let screen_descriptor = self.screen_descriptor(ui.pixels_per_point);
            for (id, image_delta) in &ui.textures_delta.set {
                self.egui_renderer
                    .update_texture(&self.device, &self.queue, *id, image_delta);
            }
            self.egui_renderer.update_buffers(
                &self.device,
                &self.queue,
                &mut encoder,
                ui.primitives,
                &screen_descriptor,
            )
        } else {
            Vec::new()
        };

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.configure_surface_for_window(window);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(io::Error::other("failed to acquire wgpu surface texture").into());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("revive_frame_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut render_pass = render_pass.forget_lifetime();
            let panel_px = if panel_visible {
                window_units_to_pixels(window, self.panel_width_px).min(self.surface_config.width)
            } else {
                0
            };
            self.game_renderer.draw(
                &self.queue,
                &mut render_pass,
                self.surface_config.width,
                self.surface_config.height,
                panel_px,
            );

            if let Some(ui) = ui.as_ref() {
                let screen_descriptor = self.screen_descriptor(ui.pixels_per_point);
                self.egui_renderer
                    .render(&mut render_pass, ui.primitives, &screen_descriptor);
            }
        }

        self.queue
            .submit(user_cmd_bufs.into_iter().chain([encoder.finish()]));
        frame.present();

        if let Some(ui) = ui {
            for id in &ui.textures_delta.free {
                self.egui_renderer.free_texture(id);
            }
        }

        Ok(())
    }

    fn screen_descriptor(&self, pixels_per_point: f32) -> ScreenDescriptor {
        ScreenDescriptor {
            size_in_pixels: [self.surface_config.width, self.surface_config.height],
            pixels_per_point,
        }
    }

    fn configure_surface_for_window(&mut self, window: &Window) {
        let (width, height) = window.size_in_pixels();
        let width = width.max(1);
        let height = height.max(1);
        if width == self.surface_config.width && height == self.surface_config.height {
            return;
        }
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    fn window_width(&self, panel_visible: bool) -> u32 {
        if panel_visible {
            self.game_w + self.panel_width_px
        } else {
            self.game_w
        }
    }
}

fn surface_config_for(
    window: &Window,
    surface: &wgpu::Surface<'_>,
    adapter: &wgpu::Adapter,
) -> Result<wgpu::SurfaceConfiguration, Box<dyn Error>> {
    let capabilities = surface.get_capabilities(adapter);
    let format = capabilities
        .formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .or_else(|| capabilities.formats.first().copied())
        .ok_or_else(|| io::Error::other("wgpu surface exposes no texture formats"))?;
    let present_mode = if capabilities
        .present_modes
        .contains(&wgpu::PresentMode::Immediate)
    {
        wgpu::PresentMode::Immediate
    } else {
        wgpu::PresentMode::Fifo
    };
    let alpha_mode = capabilities
        .alpha_modes
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
        .or_else(|| capabilities.alpha_modes.first().copied())
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    let (width, height) = window.size_in_pixels();

    Ok(wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: width.max(1),
        height: height.max(1),
        present_mode,
        alpha_mode,
        view_formats: vec![format],
        desired_maximum_frame_latency: 0,
    })
}

fn create_surface(
    instance: &wgpu::Instance,
    window: &Window,
) -> Result<wgpu::Surface<'static>, Box<dyn Error>> {
    // The SDL window is owned by the event loop and is dropped after RenderState. The unsafe
    // surface target lets the renderer resize the window while keeping the raw handles valid.
    let target = unsafe { wgpu::SurfaceTargetUnsafe::from_display_and_window(window, window)? };
    let surface = unsafe { instance.create_surface_unsafe(target)? };
    Ok(surface)
}

fn window_units_to_pixels(window: &Window, width: u32) -> u32 {
    let (window_width, _) = window.size();
    let (pixel_width, _) = window.size_in_pixels();
    if window_width == 0 {
        return width;
    }
    ((width as f32) * (pixel_width as f32 / window_width as f32)).round() as u32
}
