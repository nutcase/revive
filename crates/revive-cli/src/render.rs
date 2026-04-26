use egui_sdl2_gl::gl;
use sdl2::video::Window;

use crate::gl_game::GlGameRenderer;
use revive_core::CoreInstance;

const DEFAULT_SCALE: u32 = 3;

pub(crate) struct RenderState {
    game_renderer: Option<GlGameRenderer>,
    texture_size: (usize, usize),
    game_w: u32,
    game_h: u32,
    panel_width_px: u32,
}

impl RenderState {
    pub(crate) fn new(frame_width: usize, frame_height: usize, panel_width_px: u32) -> Self {
        Self {
            game_renderer: None,
            texture_size: (frame_width, frame_height),
            game_w: frame_width as u32 * DEFAULT_SCALE,
            game_h: frame_height as u32 * DEFAULT_SCALE,
            panel_width_px,
        }
    }

    pub(crate) fn initialize_gl(&mut self) {
        if self.game_renderer.is_none() {
            self.game_renderer = Some(GlGameRenderer::new());
        }
    }

    pub(crate) fn panel_width_px(&self) -> u32 {
        self.panel_width_px
    }

    pub(crate) fn set_panel_width_px(&mut self, panel_width_px: u32) {
        self.panel_width_px = panel_width_px;
    }

    pub(crate) fn initial_window_size(&self) -> (u32, u32) {
        (self.game_w, self.game_h)
    }

    pub(crate) fn resize_window_for_panel(&self, window: &mut Window, panel_visible: bool) {
        let _ = window.set_size(self.window_width(panel_visible), self.game_h);
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
        self.game_renderer
            .as_mut()
            .expect("OpenGL renderer must be initialized before frame upload")
            .upload_frame(frame.data, frame.width, frame.height, frame.format);
    }

    pub(crate) fn draw_game_view(&self, window: &Window, panel_visible: bool) {
        let (win_w, win_h) = window.size();
        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
        let panel_px = if panel_visible {
            self.panel_width_px
        } else {
            0
        };
        let game_vp_w = win_w.saturating_sub(panel_px);
        self.game_renderer
            .as_ref()
            .expect("OpenGL renderer must be initialized before drawing")
            .draw(0, 0, game_vp_w as i32, win_h as i32);
    }

    pub(crate) fn configure_ui_viewport(&self, window: &Window) -> (u32, u32) {
        let (win_w, win_h) = window.size();
        unsafe {
            gl::Viewport(0, 0, win_w as i32, win_h as i32);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::SCISSOR_TEST);
        }
        (win_w, win_h)
    }

    fn window_width(&self, panel_visible: bool) -> u32 {
        if panel_visible {
            self.game_w + self.panel_width_px
        } else {
            self.game_w
        }
    }
}
