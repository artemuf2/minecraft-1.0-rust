//! gfx.rs — OpenGL: мир (atlas) + 2D-прямоугольники + GUI-спрайты + текст.

use crate::mesher::Vertex;
use glow::HasContext;
use glutin::config::{ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, Version};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{SurfaceAttributesBuilder, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasWindowHandle;
use std::sync::Arc;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

pub const TRANSLUCENT_ITEM_OVERLAY: u32 = 0;
pub const TRANSLUCENT_GLINT_ITEM_OVERLAY: u32 = 0;
pub const ENERGY_SWIRL_OVERLAY: u32 = 0;
pub const TRANSLUCENT_PARTICLE_OVERLAY: u32 = 0;

#[derive(Clone, Debug, Default)]
pub struct AdapterInfo {
    pub name: String,
    pub device_type: String,
    pub backend: String,
}

pub struct Config {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy)]
pub struct Rect2D {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: [f32; 4],
}

#[derive(Clone, Copy)]
pub struct SpriteCall {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub uv: [f32; 4],
}

pub struct Renderer {
    pub config: Config,
    gl: glow::Context,
    _gl_surface: glutin::surface::Surface<WindowSurface>,
    _gl_context: glutin::context::PossiblyCurrentContext,
    _window: Arc<Window>,

    terrain_program: glow::Program,
    terrain_vao: glow::VertexArray,
    terrain_vbo: glow::Buffer,
    vp_loc: glow::UniformLocation,
    atlas_loc: glow::UniformLocation,
    lightmap_loc: glow::UniformLocation,
    atlas_tex: glow::Texture,
    lightmap_tex: glow::Texture,
    view: glam::Mat4,

    text_program: glow::Program,
    text_vao: glow::VertexArray,
    text_vbo: glow::Buffer,
    text_font_loc: glow::UniformLocation,
    text_screen_loc: glow::UniformLocation,
    text_origin_loc: glow::UniformLocation,
    font_tex: glow::Texture,
    font_pixel_w: u32,
    font_pixel_h: u32,

    rect_program: glow::Program,
    rect_vao: glow::VertexArray,
    rect_vbo: glow::Buffer,
    rect_screen_loc: glow::UniformLocation,

    sprite_program: glow::Program,
    sprite_vao: glow::VertexArray,
    sprite_vbo: glow::Buffer,
    sprite_screen_loc: glow::UniformLocation,
    sprite_tex_loc: glow::UniformLocation,
    gui_tex: glow::Texture,
}

const VS_TERRAIN: &str = r#"#version 330 core
layout(location=0) in vec3 pos;
layout(location=1) in vec2 uv;
layout(location=2) in uint layer;
layout(location=3) in uint color;
layout(location=4) in uint light;
uniform mat4 view_proj;
out vec2 v_uv; out float v_layer; out vec4 v_color; out vec2 v_light;
void main() {
    gl_Position = view_proj * vec4(pos, 1.0);
    v_uv = uv;
    v_layer = float(layer);
    v_color = vec4(float((color >> 16) & 0xFFu) / 255.0,
                   float((color >> 8)  & 0xFFu) / 255.0,
                   float( color        & 0xFFu) / 255.0,
                   float((color >> 24) & 0xFFu) / 255.0);
    float block = float( light        & 0xFu) / 15.0;
    float sky   = float((light >> 4)  & 0xFu) / 15.0;
    v_light = vec2(block, sky);
}"#;

const FS_TERRAIN: &str = r#"#version 330 core
in vec2 v_uv; in float v_layer; in vec4 v_color; in vec2 v_light;
uniform sampler2DArray atlas;
uniform sampler2D lightmap;
out vec4 frag;
void main() {
    vec4 tex = texture(atlas, vec3(v_uv, v_layer)) * v_color;
    vec3 lm = texture(lightmap, v_light).rgb;
    frag = vec4(tex.rgb * lm, tex.a);
}"#;

const VS_TEXT: &str = r#"#version 330 core
layout(location=0) in vec2 pos;
layout(location=1) in vec2 uv;
uniform vec2 screen_size;
uniform vec2 origin;
out vec2 v_uv;
void main() {
    vec2 p = origin + pos;
    vec2 ndc = vec2( p.x / screen_size.x * 2.0 - 1.0,
                     1.0 - p.y / screen_size.y * 2.0 );
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = uv;
}"#;

const FS_TEXT: &str = r#"#version 330 core
in vec2 v_uv;
uniform sampler2D font;
out vec4 frag;
void main() {
    vec4 t = texture(font, v_uv);
    if (t.a < 0.5) discard;
    frag = vec4(t.rgb, 1.0);
}"#;

const VS_RECT: &str = r#"#version 330 core
layout(location=0) in vec2 pos;
layout(location=1) in vec4 color;
uniform vec2 screen_size;
out vec4 v_color;
void main() {
    vec2 ndc = vec2( pos.x / screen_size.x * 2.0 - 1.0,
                     1.0 - pos.y / screen_size.y * 2.0 );
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_color = color;
}"#;

const FS_RECT: &str = r#"#version 330 core
in vec4 v_color;
out vec4 frag;
void main() { frag = v_color; }"#;

const VS_SPRITE: &str = r#"#version 330 core
layout(location=0) in vec2 pos;
layout(location=1) in vec2 uv;
uniform vec2 screen_size;
out vec2 v_uv;
void main() {
    vec2 ndc = vec2( pos.x / screen_size.x * 2.0 - 1.0,
                     1.0 - pos.y / screen_size.y * 2.0 );
    gl_Position = vec4(ndc, 0.0, 1.0);
    v_uv = uv;
}"#;

const FS_SPRITE: &str = r#"#version 330 core
in vec2 v_uv;
uniform sampler2D tex;
out vec4 frag;
void main() { frag = texture(tex, v_uv); }"#;

fn compile(gl: &glow::Context, vs: &str, fs: &str) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program().map_err(|e| e.to_string())?;
        let mut shaders = Vec::new();
        for (kind, src) in [(glow::VERTEX_SHADER, vs), (glow::FRAGMENT_SHADER, fs)] {
            let shader = gl.create_shader(kind).map_err(|e| e.to_string())?;
            gl.shader_source(shader, src);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                return Err(format!("shader: {}", gl.get_shader_info_log(shader)));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(format!("link: {}", gl.get_program_info_log(program)));
        }
        for s in shaders {
            gl.detach_shader(program, s);
            gl.delete_shader(s);
        }
        Ok(program)
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex { pos: [f32; 2], uv: [f32; 2] }

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectVertex { pos: [f32; 2], color: [f32; 4] }

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SpriteVertex { pos: [f32; 2], uv: [f32; 2] }

pub fn build_renderer(
    event_loop: &ActiveEventLoop,
    window: Arc<Window>,
    block_pixels: &[u8],
    block_size: u32,
    block_layers: u32,
    font_pixels: &[u8],
    font_w: u32,
    font_h: u32,
    gui_pixels: &[u8],
    gui_w: u32,
    gui_h: u32,
) -> Result<Renderer, String> {
    let raw = window.window_handle().ok().map(|h| h.as_raw());
    let template = ConfigTemplateBuilder::new().with_alpha_size(8);

    let (_, gl_config) = DisplayBuilder::new()
        .build(event_loop, template, |cfgs| {
            cfgs.reduce(|a, b| if b.num_samples() > a.num_samples() { b } else { a })
                .unwrap()
        })
        .map_err(|e| format!("glutin display: {e}"))?;

    let gl_display = gl_config.display();
    let ctx_attrs = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(raw);
    let not_current = unsafe {
        gl_display.create_context(&gl_config, &ctx_attrs).map_err(|e| e.to_string())?
    };
    let size = window.inner_size();
    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw.ok_or("no raw window handle")?,
        std::num::NonZeroU32::new(size.width.max(1)).unwrap(),
        std::num::NonZeroU32::new(size.height.max(1)).unwrap(),
    );
    let gl_surface = unsafe {
        gl_display.create_window_surface(&gl_config, &attrs).map_err(|e| e.to_string())?
    };
    let gl_context = not_current.make_current(&gl_surface).map_err(|e| e.to_string())?;
    let gl = unsafe {
        glow::Context::from_loader_function(|s| {
            gl_display.get_proc_address(&std::ffi::CString::new(s).unwrap())
        })
    };

    unsafe {
        gl.viewport(0, 0, size.width as i32, size.height as i32);
        gl.enable(glow::DEPTH_TEST);
        gl.disable(glow::CULL_FACE);
        gl.front_face(glow::CCW);

        // ---------- Мир ----------
        let program = compile(&gl, VS_TERRAIN, FS_TERRAIN)?;
        let vao = gl.create_vertex_array().map_err(|e| e.to_string())?;
        let vbo = gl.create_buffer().map_err(|e| e.to_string())?;
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let stride = std::mem::size_of::<Vertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_i32(2, 1, glow::UNSIGNED_INT, stride, 20);
        gl.enable_vertex_attrib_array(3);
        gl.vertex_attrib_pointer_i32(3, 1, glow::UNSIGNED_INT, stride, 24);
        gl.enable_vertex_attrib_array(4);
        gl.vertex_attrib_pointer_i32(4, 1, glow::UNSIGNED_INT, stride, 28);

        let atlas_tex = gl.create_texture().map_err(|e| e.to_string())?;
        gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(atlas_tex));
        gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D_ARRAY, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_image_3d(
            glow::TEXTURE_2D_ARRAY, 0, glow::RGBA8 as i32,
            block_size as i32, block_size as i32, block_layers as i32,
            0, glow::RGBA, glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(Some(block_pixels)),
        );

        let lightmap_tex = gl.create_texture().map_err(|e| e.to_string())?;
        gl.bind_texture(glow::TEXTURE_2D, Some(lightmap_tex));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
        let white = [255u8; 16 * 16 * 4];
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32, 16, 16, 0,
            glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(&white)));

        let vp_loc = gl.get_uniform_location(program, "view_proj").unwrap();
        let atlas_loc = gl.get_uniform_location(program, "atlas").unwrap();
        let lightmap_loc = gl.get_uniform_location(program, "lightmap").unwrap();

        // ---------- Текст ----------
        let text_program = compile(&gl, VS_TEXT, FS_TEXT)?;
        let text_vao = gl.create_vertex_array().map_err(|e| e.to_string())?;
        let text_vbo = gl.create_buffer().map_err(|e| e.to_string())?;
        gl.bind_vertex_array(Some(text_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(text_vbo));
        let ts = std::mem::size_of::<TextVertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, ts, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, ts, 8);

        let font_tex = gl.create_texture().map_err(|e| e.to_string())?;
        gl.bind_texture(glow::TEXTURE_2D, Some(font_tex));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32,
            font_w as i32, font_h as i32, 0,
            glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(font_pixels)));

        let text_font_loc = gl.get_uniform_location(text_program, "font").unwrap();
        let text_screen_loc = gl.get_uniform_location(text_program, "screen_size").unwrap();
        let text_origin_loc = gl.get_uniform_location(text_program, "origin").unwrap();

        // ---------- 2D-цвет ----------
        let rect_program = compile(&gl, VS_RECT, FS_RECT)?;
        let rect_vao = gl.create_vertex_array().map_err(|e| e.to_string())?;
        let rect_vbo = gl.create_buffer().map_err(|e| e.to_string())?;
        gl.bind_vertex_array(Some(rect_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(rect_vbo));
        let rs = std::mem::size_of::<RectVertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, rs, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 4, glow::FLOAT, false, rs, 8);
        let rect_screen_loc = gl.get_uniform_location(rect_program, "screen_size").unwrap();

        // ---------- 2D-спрайт (GUI) ----------
        let sprite_program = compile(&gl, VS_SPRITE, FS_SPRITE)?;
        let sprite_vao = gl.create_vertex_array().map_err(|e| e.to_string())?;
        let sprite_vbo = gl.create_buffer().map_err(|e| e.to_string())?;
        gl.bind_vertex_array(Some(sprite_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(sprite_vbo));
        let ss = std::mem::size_of::<SpriteVertex>() as i32;
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, ss, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, ss, 8);

        let gui_tex = gl.create_texture().map_err(|e| e.to_string())?;
        gl.bind_texture(glow::TEXTURE_2D, Some(gui_tex));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA8 as i32,
            gui_w as i32, gui_h as i32, 0,
            glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(gui_pixels)));

        let sprite_screen_loc = gl.get_uniform_location(sprite_program, "screen_size").unwrap();
        let sprite_tex_loc = gl.get_uniform_location(sprite_program, "tex").unwrap();

        Ok(Renderer {
            config: Config { width: size.width.max(1), height: size.height.max(1) },
            gl, _gl_surface: gl_surface, _gl_context: gl_context, _window: window,
            terrain_program: program, terrain_vao: vao, terrain_vbo: vbo,
            vp_loc, atlas_loc, lightmap_loc,
            atlas_tex, lightmap_tex,
            view: glam::Mat4::IDENTITY,
            text_program, text_vao, text_vbo,
            text_font_loc, text_screen_loc, text_origin_loc,
            font_tex, font_pixel_w: font_w, font_pixel_h: font_h,
            rect_program, rect_vao, rect_vbo, rect_screen_loc,
            sprite_program, sprite_vao, sprite_vbo,
            sprite_screen_loc, sprite_tex_loc, gui_tex,
        })
    }
}

impl Renderer {
    pub fn set_camera(&mut self, pos: [f32; 3], yaw_deg: f32, pitch_deg: f32) {
        let yaw = yaw_deg.to_radians();
        let pitch = pitch_deg.to_radians();
        let forward = glam::Vec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            yaw.cos() * pitch.cos(),
        );
        let eye = glam::Vec3::from(pos);
        let target = eye + forward;
        self.view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
    }

    pub fn render_frame(
        &mut self,
        cube: &[Vertex],
        rects: &[Rect2D],
        sprites: &[SpriteCall],
        text: &[(&str, f32, f32, f32)],
    ) -> Result<(), String> {
        unsafe {
            self.gl.clear_color(0.5, 0.7, 1.0, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            if !cube.is_empty()    { self.draw_world(cube)?; }
            if !rects.is_empty()   { self.draw_rects(rects)?; }
            if !sprites.is_empty() { self.draw_sprites(sprites)?; }
            if !text.is_empty()    { self.draw_text(text)?; }
            self._gl_surface.swap_buffers(&self._gl_context).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn draw_world(&mut self, cube: &[Vertex]) -> Result<(), String> {
        let gl = &self.gl;
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.disable(glow::CULL_FACE);
            let aspect = self.config.width as f32 / self.config.height.max(1) as f32;
            let proj = glam::Mat4::perspective_rh(70f32.to_radians(), aspect, 300.0, 0.1);
            let view_proj = proj * self.view;
            gl.use_program(Some(self.terrain_program));
            gl.uniform_matrix_4_f32_slice(Some(&self.vp_loc), false, &view_proj.to_cols_array());
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D_ARRAY, Some(self.atlas_tex));
            gl.uniform_1_i32(Some(&self.atlas_loc), 0);
            gl.active_texture(glow::TEXTURE1);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.lightmap_tex));
            gl.uniform_1_i32(Some(&self.lightmap_loc), 1);
            gl.bind_vertex_array(Some(self.terrain_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.terrain_vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(cube), glow::STREAM_DRAW);
            gl.draw_arrays(glow::TRIANGLES, 0, cube.len() as i32);
            gl.bind_vertex_array(None);
        }
        Ok(())
    }

    fn draw_rects(&mut self, rects: &[Rect2D]) -> Result<(), String> {
        let gl = &self.gl;
        unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(self.rect_program));
            gl.uniform_2_f32(Some(&self.rect_screen_loc),
                self.config.width as f32, self.config.height as f32);
            gl.bind_vertex_array(Some(self.rect_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.rect_vbo));
            for r in rects {
                let x0 = r.x; let y0 = r.y;
                let x1 = r.x + r.w; let y1 = r.y + r.h;
                let c = r.color;
                let v = [
                    RectVertex { pos: [x0, y0], color: c },
                    RectVertex { pos: [x1, y0], color: c },
                    RectVertex { pos: [x1, y1], color: c },
                    RectVertex { pos: [x0, y0], color: c },
                    RectVertex { pos: [x1, y1], color: c },
                    RectVertex { pos: [x0, y1], color: c },
                ];
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&v), glow::STREAM_DRAW);
                gl.draw_arrays(glow::TRIANGLES, 0, 6);
            }
            gl.bind_vertex_array(None);
            gl.disable(glow::BLEND);
            gl.enable(glow::DEPTH_TEST);
        }
        Ok(())
    }

    fn draw_sprites(&mut self, sprites: &[SpriteCall]) -> Result<(), String> {
        let gl = &self.gl;
        unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(self.sprite_program));
            gl.uniform_2_f32(Some(&self.sprite_screen_loc),
                self.config.width as f32, self.config.height as f32);
            gl.uniform_1_i32(Some(&self.sprite_tex_loc), 0);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.gui_tex));
            gl.bind_vertex_array(Some(self.sprite_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.sprite_vbo));
            for s in sprites {
                let x0 = s.x; let y0 = s.y;
                let x1 = s.x + s.w; let y1 = s.y + s.h;
                let [u0, v0, u1, v1] = s.uv;
                let v = [
                    SpriteVertex { pos: [x0, y0], uv: [u0, v0] },
                    SpriteVertex { pos: [x1, y0], uv: [u1, v0] },
                    SpriteVertex { pos: [x1, y1], uv: [u1, v1] },
                    SpriteVertex { pos: [x0, y0], uv: [u0, v0] },
                    SpriteVertex { pos: [x1, y1], uv: [u1, v1] },
                    SpriteVertex { pos: [x0, y1], uv: [u0, v1] },
                ];
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&v), glow::STREAM_DRAW);
                gl.draw_arrays(glow::TRIANGLES, 0, 6);
            }
            gl.bind_vertex_array(None);
            gl.disable(glow::BLEND);
            gl.enable(glow::DEPTH_TEST);
        }
        Ok(())
    }

    fn draw_text(&mut self, text: &[(&str, f32, f32, f32)]) -> Result<(), String> {
        let gl = &self.gl;
        unsafe {
            gl.disable(glow::DEPTH_TEST);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(self.text_program));
            gl.uniform_2_f32(Some(&self.text_screen_loc),
                self.config.width as f32, self.config.height as f32);
            gl.uniform_1_i32(Some(&self.text_font_loc), 0);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(self.font_tex));
            gl.bind_vertex_array(Some(self.text_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.text_vbo));
            for (s, x, y, scale) in text {
                let verts = build_text_vertices(s, *scale, self.font_pixel_w / 16, self.font_pixel_h / 16);
                if verts.is_empty() { continue; }
                gl.uniform_2_f32(Some(&self.text_origin_loc), *x, *y);
                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&verts), glow::STREAM_DRAW);
                gl.draw_arrays(glow::TRIANGLES, 0, verts.len() as i32);
            }
            gl.bind_vertex_array(None);
            gl.disable(glow::BLEND);
            gl.enable(glow::DEPTH_TEST);
        }
        Ok(())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width;
        self.config.height = height;
        unsafe { self.gl.viewport(0, 0, width as i32, height as i32); }
    }
}

fn build_text_vertices(text: &str, scale: f32, cell_w: u32, cell_h: u32) -> Vec<TextVertex> {
    let atlas_cols = 16.0;
    let atlas_rows = 16.0;
    let gw = cell_w as f32 * scale;
    let gh = cell_h as f32 * scale;
    let mut out = Vec::new();
    let mut pen_x = 0.0f32;
    for ch in text.chars() {
        let code = ch as u32;
        if code >= 256 { pen_x += gw; continue; }
        let col = (code % 16) as f32;
        let row = (code / 16) as f32;
        let u0 = col / atlas_cols; let v0 = row / atlas_rows;
        let u1 = (col + 1.0) / atlas_cols; let v1 = (row + 1.0) / atlas_rows;
        let x0 = pen_x; let y0 = 0.0;
        let x1 = pen_x + gw; let y1 = gh;
        out.extend_from_slice(&[
            TextVertex { pos: [x0, y0], uv: [u0, v0] },
            TextVertex { pos: [x1, y0], uv: [u1, v0] },
            TextVertex { pos: [x1, y1], uv: [u1, v1] },
            TextVertex { pos: [x0, y0], uv: [u0, v0] },
            TextVertex { pos: [x1, y1], uv: [u1, v1] },
            TextVertex { pos: [x0, y1], uv: [u0, v1] },
        ]);
        pen_x += gw;
    }
    out
}