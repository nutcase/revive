use egui_sdl2_gl::gl;
use egui_sdl2_gl::gl::types::*;
use revive_core::PixelFormat;
use std::ffi::CString;
use std::ptr;

const VS_SRC: &str = r#"#version 150
in vec2 a_pos;
in vec2 a_uv;
out vec2 v_uv;
void main() {
    gl_Position = vec4(a_pos, 0.0, 1.0);
    v_uv = a_uv;
}
"#;

const FS_SRC: &str = r#"#version 150
in vec2 v_uv;
out vec4 o_color;
uniform sampler2D u_tex;
void main() {
    o_color = texture(u_tex, v_uv);
}
"#;

#[rustfmt::skip]
const QUAD: [f32; 16] = [
    -1.0, -1.0,  0.0, 1.0,
     1.0, -1.0,  1.0, 1.0,
     1.0,  1.0,  1.0, 0.0,
    -1.0,  1.0,  0.0, 0.0,
];

pub struct GlGameRenderer {
    program: GLuint,
    vao: GLuint,
    vbo: GLuint,
    texture: GLuint,
    tex_w: usize,
    tex_h: usize,
    tex_format: Option<PixelFormat>,
}

impl GlGameRenderer {
    pub fn new() -> Self {
        unsafe {
            let vs = compile_shader(gl::VERTEX_SHADER, VS_SRC);
            let fs = compile_shader(gl::FRAGMENT_SHADER, FS_SRC);
            let program = gl::CreateProgram();
            gl::AttachShader(program, vs);
            gl::AttachShader(program, fs);
            gl::LinkProgram(program);
            gl::DeleteShader(vs);
            gl::DeleteShader(fs);

            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            let mut vbo = 0;
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (QUAD.len() * std::mem::size_of::<f32>()) as GLsizeiptr,
                QUAD.as_ptr() as *const _,
                gl::STATIC_DRAW,
            );

            let stride = 4 * std::mem::size_of::<f32>() as GLsizei;
            let a_pos = gl::GetAttribLocation(program, c_str("a_pos").as_ptr());
            gl::EnableVertexAttribArray(a_pos as GLuint);
            gl::VertexAttribPointer(
                a_pos as GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                ptr::null(),
            );

            let a_uv = gl::GetAttribLocation(program, c_str("a_uv").as_ptr());
            gl::EnableVertexAttribArray(a_uv as GLuint);
            gl::VertexAttribPointer(
                a_uv as GLuint,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (2 * std::mem::size_of::<f32>()) as *const _,
            );

            gl::BindVertexArray(0);

            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as GLint,
            );
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as GLint,
            );
            gl::BindTexture(gl::TEXTURE_2D, 0);

            Self {
                program,
                vao,
                vbo,
                texture,
                tex_w: 0,
                tex_h: 0,
                tex_format: None,
            }
        }
    }

    pub fn upload_frame(&mut self, frame: &[u8], w: usize, h: usize, format: PixelFormat) {
        let (internal_format, external_format) = gl_texture_formats(format);

        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            if w != self.tex_w || h != self.tex_h || self.tex_format != Some(format) {
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    internal_format,
                    w as GLsizei,
                    h as GLsizei,
                    0,
                    external_format,
                    gl::UNSIGNED_BYTE,
                    frame.as_ptr() as *const _,
                );
                self.tex_w = w;
                self.tex_h = h;
                self.tex_format = Some(format);
            } else {
                gl::TexSubImage2D(
                    gl::TEXTURE_2D,
                    0,
                    0,
                    0,
                    w as GLsizei,
                    h as GLsizei,
                    external_format,
                    gl::UNSIGNED_BYTE,
                    frame.as_ptr() as *const _,
                );
            }
            gl::BindTexture(gl::TEXTURE_2D, 0);
        }
    }

    pub fn draw(&self, vp_x: i32, vp_y: i32, vp_w: i32, vp_h: i32) {
        if vp_w <= 0 || vp_h <= 0 || self.tex_w == 0 || self.tex_h == 0 {
            return;
        }

        let src_aspect = self.tex_w as f64 / self.tex_h as f64;
        let dst_aspect = vp_w as f64 / vp_h as f64;
        let (fit_w, fit_h) = if dst_aspect > src_aspect {
            let h = vp_h;
            let w = (vp_h as f64 * src_aspect).round() as i32;
            (w, h)
        } else {
            let w = vp_w;
            let h = (vp_w as f64 / src_aspect).round() as i32;
            (w, h)
        };
        let fit_x = vp_x + (vp_w - fit_w) / 2;
        let fit_y = vp_y + (vp_h - fit_h) / 2;

        unsafe {
            gl::Viewport(fit_x, fit_y, fit_w, fit_h);
            gl::Disable(gl::BLEND);
            gl::Disable(gl::SCISSOR_TEST);
            gl::UseProgram(self.program);
            gl::ActiveTexture(gl::TEXTURE0);
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::BindVertexArray(self.vao);
            gl::DrawArrays(gl::TRIANGLE_FAN, 0, 4);
            gl::BindVertexArray(0);
            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::UseProgram(0);
        }
    }
}

fn gl_texture_formats(format: PixelFormat) -> (GLint, GLenum) {
    match format {
        PixelFormat::Rgb24 => (gl::RGB8 as GLint, gl::RGB),
        PixelFormat::Rgba8888 => (gl::RGBA8 as GLint, gl::RGBA),
        PixelFormat::Bgra8888 => (gl::RGBA8 as GLint, gl::BGRA),
    }
}

impl Drop for GlGameRenderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, &self.texture);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
            gl::DeleteProgram(self.program);
        }
    }
}

fn c_str(value: &str) -> CString {
    CString::new(value).unwrap()
}

unsafe fn compile_shader(kind: GLenum, source: &str) -> GLuint {
    unsafe {
        let shader = gl::CreateShader(kind);
        let c_source = c_str(source);
        gl::ShaderSource(shader, 1, &c_source.as_ptr(), ptr::null());
        gl::CompileShader(shader);
        let mut success = 0;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);
        if success == 0 {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = vec![0u8; len as usize];
            gl::GetShaderInfoLog(shader, len, ptr::null_mut(), buf.as_mut_ptr() as *mut _);
            eprintln!("Shader compile error: {}", String::from_utf8_lossy(&buf));
        }
        shader
    }
}
