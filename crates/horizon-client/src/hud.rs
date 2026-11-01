//! 2D overlay: one atlas holding GUI sprites, the bitmap font, item and block
//! textures, and a batch of textured quads in GUI pixels. CPU-only layout;
//! drawing goes through `draw_gl` (glow).
use crate::Result;
use horizon_assets::animation::AnimationMetadata;
use std::{collections::{HashMap, HashSet}, fs, path::{Path, PathBuf}};

// ---- Atlas (unchanged, CPU) ----------------------------------------------
#[derive(Clone, Copy, Debug)]
pub struct Rect { pub u0: f32, pub v0: f32, pub u1: f32, pub v1: f32, pub w: u32, pub h: u32 }
impl Rect {
    pub fn texels(&self, (width, height): (u32, u32)) -> (u32, u32) {
        (((self.u1 - self.u0) * width as f32).round() as u32,
         ((self.v1 - self.v0) * height as f32).round() as u32)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextureFlags { pub blur: bool, pub clamp: bool }
impl TextureFlags {
    pub fn from_mcmeta(json: &str) -> Option<Self> {
        let meta: serde_json::Value = serde_json::from_str(json).ok()?;
        let section = meta.get("texture")?;
        let flag = |key| section.get(key).and_then(serde_json::Value::as_bool) == Some(true);
        let flags = Self { blur: flag("blur"), clamp: flag("clamp") };
        (flags != Self::default()).then_some(flags)
    }
}
pub struct HudAtlas {
    pub pixels: Vec<u8>, pub width: u32, pub height: u32,
    pub sprites: HashMap<String, Rect>,
    pub translucent: HashSet<String>,
    pub flags: HashMap<String, TextureFlags>,
    pub sprite_animations: Vec<(Rect, crate::sprites::SpriteAnimation)>,
    pub font: crate::font::Font,
    pub glint: (Vec<u8>, u32, u32),
    pub glint_armor: (Vec<u8>, u32, u32),
    white: Rect,
}
pub const GLYPH_ROWS: u32 = 512;
pub const MAX_ATLAS_SIZE: u32 = 16384;
pub const MAP_SLOTS: usize = 256;
pub const MINIMAP_SPRITE: &str = "horizon/minimap";
pub const SKIN_SLOTS: usize = 128;
pub const WORLD_ICON_SLOTS: usize = 32;
pub const PACK_ICON_SLOTS: usize = 64;
pub const PACK_ICON_SIZE: u32 = 128;
pub const TRIM_SLOTS: usize = 32;

pub fn decode(path: &Path) -> Result<(u32, u32, Vec<u8>)> { decode_bytes(&fs::read(path)?) }
pub fn decode_bytes(data: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(data));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let mut bytes = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut bytes)?;
    let channels = info.color_type.samples();
    let mut rgba = Vec::with_capacity((info.width * info.height * 4) as usize);
    for p in bytes[..info.buffer_size()].chunks_exact(channels) {
        rgba.extend(match channels {
            4 => [p[0], p[1], p[2], p[3]],
            3 => [p[0], p[1], p[2], 255],
            2 => [p[0], p[0], p[0], p[1]],
            _ => [p[0], p[0], p[0], 255],
        });
    }
    Ok((info.width, info.height, rgba))
}
fn png_size(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut header = [0u8; 24];
    fs::File::open(path).ok()?.read_exact(&mut header).ok()?;
    let field = |at: usize| u32::from_be_bytes(header[at..at + 4].try_into().unwrap_or_default());
    (header[..8] == *b"\x89PNG\r\n\x1a\n" && header[12..16] == *b"IHDR")
        .then(|| (field(16), field(20)))
}
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return; };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() { walk(&path, out); }
        else if path.extension().is_some_and(|e| e == "png") { out.push(path); }
    }
}
type PackedImage = (String, u32, u32, Vec<u8>);
fn strip_frame(pixels: &[u8], stride: u32, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let side = w.max(h);
    let mut out = Vec::with_capacity((side * side * 4) as usize);
    for row in 0..side {
        let source_row = y + row * h / side;
        for column in 0..side {
            let at = ((source_row * stride + x + column * w / side) * 4) as usize;
            out.extend_from_slice(&pixels[at..at + 4]);
        }
    }
    out
}
fn wrap_border((name, w, h, data): &PackedImage) -> PackedImage {
    let (bw, bh) = (w + 2, h + 2);
    let mut out = Vec::with_capacity((bw * bh * 4) as usize);
    for y in 0..bh {
        let row = (y + h - 1) % h;
        for x in 0..bw {
            let at = ((row * w + (x + w - 1) % w) * 4) as usize;
            out.extend_from_slice(&data[at..at + 4]);
        }
    }
    (name.clone(), bw, bh, out)
}
fn atlas_sprites(root: &Path, packed: &[PackedImage]) -> Vec<PackedImage> {
    // unchanged: generated atlas sprites (paletted_permutations etc.)
    use crate::atlas_sources::{self, SpriteOrigin};
    let have: std::collections::HashSet<&str> = packed.iter().map(|i| i.0.as_str()).collect();
    let textures = atlas_sources::list_textures(root);
    let mut names: Vec<(String, Vec<(usize, SpriteOrigin)>)> = Vec::new();
    let mut slots = std::collections::HashMap::new();
    let mut order = 0;
    for (atlas, sources) in atlas_sources::atlases(root) {
        let item_models = atlas == "minecraft:blocks" || atlas == "minecraft:items";
        for (id, origin) in atlas_sources::list(&sources, &textures) {
            if !item_models && matches!(origin, SpriteOrigin::File(_)) { continue; }
            let name = id.strip_prefix("minecraft:").unwrap_or(&id).to_owned();
            if have.contains(name.as_str()) { continue; }
            if let SpriteOrigin::File(texture) = &origin
                && texture.strip_prefix("minecraft:") == Some(name.as_str()) { continue; }
            let slot = *slots.entry(name.clone()).or_insert_with(|| { names.push((name, Vec::new())); names.len() - 1 });
            names[slot].1.push((order, origin));
            order += 1;
        }
    }
    let load = |texture: &str, directory: &str| decode(&atlas_sources::texture_path(root, texture, directory)).ok();
    let first = |(name, origins): &(String, Vec<(usize, SpriteOrigin)>)| {
        origins.iter().find_map(|(order, origin)| {
            let image = match origin {
                SpriteOrigin::File(texture) => {
                    let path = texture.strip_prefix("minecraft:").unwrap_or(texture);
                    load(texture, "textures").map(|(w, h, p)| {
                        if h > w && (path.starts_with("block/") || path.starts_with("item/")) {
                            (w, w, p[..(w * w * 4) as usize].to_vec())
                        } else { (w, h, p) }
                    })
                }
                generated => atlas_sources::generate(generated, &load),
            };
            image.map(|(w, h, pixels)| (*order, (name.clone(), w, h, pixels)))
        })
    };
    let mut out: Vec<_> = crate::par::par_map(&names, first).into_iter().flatten().collect();
    out.sort_unstable_by_key(|(order, _)| *order);
    out.into_iter().map(|(_, image)| image).collect()
}
impl HudAtlas {
    pub fn glyph(&self, ch: char) -> Option<(Rect, f32)> { let g = self.font.glyph(ch); g.rect.map(|r| (r, g.advance)) }
    pub fn glyph_cell(&self, ch: char) -> (Option<Rect>, f32) { let g = self.font.glyph(ch); (g.rect, g.advance) }
    pub fn build(root: &Path) -> Result<Self> { Self::build_over(root, root) }
    pub fn build_over(root: &Path, defaults: &Path) -> Result<Self> {
        Self::pack(root, defaults, &["gui","item","block","font","environment","misc","painting","entity","particle","mob_effect","map"], true)
    }
    fn slots(images: &mut Vec<PackedImage>, trim: (u32, u32)) {
        for i in 0..MAP_SLOTS { images.push((format!("horizon/map{i}"), 128, 128, vec![0; 128*128*4])); }
        images.push((MINIMAP_SPRITE.into(), crate::minimap::SIZE as u32, crate::minimap::SIZE as u32, vec![0; crate::minimap::SIZE*crate::minimap::SIZE*4]));
        for i in 0..SKIN_SLOTS { images.push((format!("horizon/skin{i}"), 64, 64, vec![0; 64*64*4])); }
        for i in 0..TRIM_SLOTS { images.push((format!("horizon/trim{i}"), trim.0, trim.1, vec![0; (trim.0*trim.1*4) as usize])); }
        for i in 0..WORLD_ICON_SLOTS { images.push((format!("horizon/world-icon{i}"), 64, 64, vec![0; 64*64*4])); }
        let side = PACK_ICON_SIZE as usize;
        for i in 0..PACK_ICON_SLOTS { images.push((format!("horizon/pack-icon{i}"), PACK_ICON_SIZE, PACK_ICON_SIZE, vec![0; side*side*4])); }
        images.push(("horizon/ui-text".into(), 2048, 1024, vec![0; 2048*1024*4]));
    }
    pub fn fonts(root: &Path) -> Result<Self> {
        let mut atlas = Self::pack(root, root, &["font"], false)?;
        atlas.pixels = Vec::new();
        Ok(atlas)
    }
    fn layout(images: &[PackedImage], max: u32) -> Result<(u32, Vec<(u32, u32)>, u32)> {
        let mut width = 4096.min(max);
        loop {
            let mut placements = Vec::with_capacity(images.len());
            let (mut x, mut y, mut row) = (0u32, 0u32, 0u32);
            for (_, w, h, _) in images {
                if x + w + 1 > width { x = 0; y += row + 1; row = 0; }
                placements.push((x, y));
                x += w + 1;
                row = row.max(*h);
            }
            let glyph_region = y + row + 1;
            let wide_enough = images.iter().all(|i| i.1 < width);
            if wide_enough && glyph_region + GLYPH_ROWS <= max { return Ok((width, placements, glyph_region)); }
            if width >= max {
                let texels: u64 = images.iter().map(|i| u64::from(i.1) * u64::from(i.2)).sum();
                return Err(format!("the textures ({} images, {texels} texels) do not fit a {max}x{max} atlas", images.len()).into());
            }
            width = (width * 2).min(max);
        }
    }
    fn pack(root: &Path, defaults: &Path, folders: &[&str], full: bool) -> Result<Self> {
        let textures = root.join("assets/minecraft/textures");
        let default_textures = (defaults != root).then(|| defaults.join("assets/minecraft/textures"));
        let mut files = Vec::new();
        for folder in folders { walk(&textures.join(folder), &mut files); }
        files.retain(|p| { let s = p.to_string_lossy(); !s.contains("title/background") && !s.contains("realms") && !s.contains("presets") });
        type Decoded = (PackedImage, Option<TextureFlags>, Option<(u32, u32)>, bool);
        let decode_one = |path: &PathBuf| -> Option<Decoded> {
            let name = path.strip_prefix(&textures).ok()?.with_extension("").to_string_lossy().replace('\\', "/");
            let (mut w, mut h, pixels) = decode(path).ok()?;
            let mcmeta = fs::read_to_string(path.with_extension("png.mcmeta")).ok();
            let animation = mcmeta.as_deref().and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
                .and_then(|meta| meta.get("animation").and_then(|s| AnimationMetadata::from_json(&s.to_string()).ok()));
            let frames = name.starts_with("block/") || name.starts_with("item/");
            let pixels = if frames {
                let stride = w;
                let [fw, fh] = animation.as_ref().map_or([w.min(h) as i32; 2], |meta| meta.frame_size(w as i32, h as i32));
                match (u32::try_from(fw), u32::try_from(fh)) {
                    (Ok(fw), Ok(fh)) if fw > 0 && fh > 0 && w % fw == 0 && h % fh == 0 && (fw, fh) != (w, h) => {
                        let side = fw.max(fh);
                        (w, h) = (side, side);
                        strip_frame(&pixels, stride, 0, 0, fw, fh)
                    }
                    _ => pixels,
                }
            } else { pixels };
            let gui_size = default_textures.as_ref().and_then(|d| png_size(&d.join(format!("{name}.png"))))
                .map(|(dw, dh)| { let side = dw.min(dh); if frames { (side, side) } else { (dw, dh) } })
                .filter(|&size| size != (w, h));
            let flags = mcmeta.as_deref().and_then(TextureFlags::from_mcmeta);
            let animated = frames && animation.is_some();
            let (gw, gh) = gui_size.unwrap_or((w, h));
            (gw <= 1024 && (gh <= 512 || name.starts_with("font/"))).then_some(((name, w, h, pixels), flags, gui_size, animated))
        };
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get());
        let chunk = files.len().div_ceil(threads).max(1);
        let decoded: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = files.chunks(chunk).map(|part| scope.spawn(move || part.iter().filter_map(decode_one).collect::<Vec<_>>())).collect();
            handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        let mut flags = HashMap::new();
        let mut gui_sizes = HashMap::new();
        let mut animated: Vec<String> = Vec::new();
        let mut images: Vec<PackedImage> = Vec::with_capacity(decoded.len());
        for (image, image_flags, gui_size, is_animated) in decoded {
            if let Some(f) = image_flags { flags.insert(image.0.clone(), f); }
            if let Some(size) = gui_size { gui_sizes.insert(image.0.clone(), size); }
            if is_animated { animated.push(image.0.clone()); }
            images.push(image);
        }
        if full { images.extend(atlas_sprites(root, &images)); }
        if let Some(edition) = images.iter_mut().find(|i| i.0 == crate::title_edition::SPRITE)
            && let Some(pixels) = crate::title_edition::rust_edition(edition.1, edition.2, &edition.3) { edition.3 = pixels; }
        images.push(("horizon/white".into(), 4, 4, vec![255; 64]));
        if full { Self::slots(&mut images, crate::trims::slot_size(root)); }
        let mut bordered = std::collections::HashSet::new();
        for image in &mut images { if flags.get(&image.0).is_some_and(|f: &TextureFlags| f.blur) { *image = wrap_border(image); bordered.insert(image.0.clone()); } }
        images.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));
        let (width, placements, glyph_region) = Self::layout(&images, MAX_ATLAS_SIZE)?;
        let height = (glyph_region + GLYPH_ROWS).next_multiple_of(1024);
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let mut sprites = HashMap::new();
        let mut translucent = HashSet::new();
        for ((name, w, h, data), (px, py)) in images.iter().zip(placements) {
            if data.as_chunks::<4>().0.iter().any(|p| p[3] != 0 && p[3] != 255) { translucent.insert(name.clone()); }
            for row in 0..*h {
                let src = &data[(row * w * 4) as usize..((row + 1) * w * 4) as usize];
                let dst = (((py + row) * width + px) * 4) as usize;
                pixels[dst..dst + (w * 4) as usize].copy_from_slice(src);
            }
            let b = u32::from(bordered.contains(name));
            let (px, py, w, h) = (px + b, py + b, w - 2 * b, h - 2 * b);
            let (gw, gh) = gui_sizes.get(name).copied().unwrap_or((w, h));
            sprites.insert(name.clone(), Rect { u0: px as f32 / width as f32, v0: py as f32 / height as f32,
                u1: (px + w) as f32 / width as f32, v1: (py + h) as f32 / height as f32, w: gw, h: gh });
        }
        let mut loaded = crate::sprites::Sprites::default();
        let mut sprite_animations = Vec::new();
        animated.sort();
        for name in animated {
            let Some(rect) = sprites.get(&name).copied() else { continue; };
            let layer = loaded.insert(root, &format!("minecraft:{name}"))?;
            let Some(animation) = loaded.animations.iter().find(|a| a.layer == layer).cloned() else { continue; };
            let (tw, th) = rect.texels((width, height));
            let image = animation.resized_pixels(animation.frame(0), tw, th);
            let x = (rect.u0 * width as f32).round() as usize;
            let y = (rect.v0 * height as f32).round() as usize;
            let row_bytes = tw as usize * 4;
            for row in 0..th as usize {
                let dst = ((y + row) * width as usize + x) * 4;
                let src = row * row_bytes;
                pixels[dst..dst + row_bytes].copy_from_slice(&image[src..src + row_bytes]);
            }
            sprite_animations.push((rect, animation));
        }
        let white = sprites["horizon/white"];
        let glint = Self::glint_texture(root, defaults, "enchanted_glint_item");
        let glint_armor = Self::glint_texture(root, defaults, "enchanted_glint_armor");
        let font = crate::font::Font::load(root, &sprites, &pixels, (width, height), (glyph_region, height - glyph_region), white);
        Ok(Self { pixels, width, height, sprites, translucent, flags, sprite_animations, glint, glint_armor, font, white })
    }
    fn glint_texture(root: &Path, defaults: &Path, name: &str) -> (Vec<u8>, u32, u32) {
        let relative = format!("assets/minecraft/textures/misc/{name}.png");
        let mut path = root.join(&relative);
        if !path.exists() && defaults != root { path = defaults.join(&relative); }
        decode(&path).map_or((vec![0; 4], 1, 1), |(w, h, pixels)| (pixels, w, h))
    }
}

// ---- CPU vertices & batch (unchanged, optimized) -------------------------
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HudVertex {
    pub pos: [f32; 2],
    pub uv: [f32; 2],
    pub color: u32,
    pub depth: f32,
    pub glint_uv: [f32; 2],
}
pub static GUI_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub struct HudBatch {
    pub vertices: Vec<HudVertex>,
    pub scale: f32,
    pub width: f32,
    pub height: f32,
    pub patches: Vec<crate::font::Patch>,
    pub blends: Vec<(std::ops::Range<u32>, Blend)>,
    pub end_portal: EndPortalFill,
    pub glint_alpha: f32,
    pub blur_at: u32,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Blend { Alpha, Invert, Additive, Depth([u32; 4]), Vignette, EndPortal }
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EndPortalFill { pub sprites: [[f32; 4]; 2], pub game_time: f32 }
pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> u32 { u32::from(a) << 24 | u32::from(b) << 16 | u32::from(g) << 8 | u32::from(r) }
impl HudBatch {
    pub fn new(window: [u32; 2]) -> Self {
        let limit = GUI_SCALE.load(std::sync::atomic::Ordering::Relaxed);
        let mut scale = 1;
        while (limit == 0 || scale < limit) && window[0] / (scale + 1) >= 320 && window[1] / (scale + 1) >= 240 { scale += 1; }
        Self { vertices: Vec::with_capacity(1024), scale: scale as f32,
            width: (window[0] as f32 / scale as f32).ceil(), height: (window[1] as f32 / scale as f32).ceil(),
            patches: Vec::new(), blends: Vec::new(), end_portal: EndPortalFill::default(), glint_alpha: 0.75, blur_at: 0 }
    }
    pub fn invert_since(&mut self, start: usize) { self.blend_since(start, Blend::Invert); }
    pub fn blend_since(&mut self, start: usize, blend: Blend) {
        let range = start as u32..self.vertices.len() as u32;
        if range.is_empty() || blend == Blend::Alpha { return; }
        match self.blends.last_mut() {
            Some((last, b)) if last.end == range.start && *b == blend => last.end = range.end,
            _ => self.blends.push((range, blend)),
        }
    }
    pub fn text_highlight(&mut self, atlas: &HudAtlas, (x, y, w, h): (f32, f32, f32, f32), invert_text: bool) {
        if invert_text { let s = self.vertices.len(); self.fill(atlas, x, y, w, h, rgba(255,255,255,255)); self.invert_since(s); }
        let s = self.vertices.len(); self.fill(atlas, x, y, w, h, rgba(0,0,255,255)); self.blend_since(s, Blend::Additive);
    }
    pub fn runs(&self) -> Vec<(std::ops::Range<u32>, Blend)> {
        let mut out = Vec::new(); let mut at = 0;
        for (r, blend) in &self.blends { if r.start > at { out.push((at..r.start, Blend::Alpha)); } out.push((r.clone(), *blend)); at = r.end; }
        let end = self.vertices.len() as u32; if end > at { out.push((at..end, Blend::Alpha)); }
        out
    }
    pub fn runs_in(&self, vertices: std::ops::Range<u32>) -> Vec<(std::ops::Range<u32>, Blend)> {
        let mut runs = self.runs();
        for (range, _) in &mut runs { *range = range.start.max(vertices.start)..range.end.min(vertices.end); }
        runs.retain(|(r, _)| !r.is_empty()); runs
    }
    pub fn centered_origin(&self, width: f32, height: f32) -> (f32, f32) {
        (((self.width - width) / 2.0).trunc(), ((self.height - height) / 2.0).trunc())
    }
    pub fn quad(&mut self, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], color: u32) {
        for i in [0, 1, 2, 0, 2, 3] { self.vertices.push(HudVertex { pos: p[i], uv: uv[i], color, depth: 0.0, glint_uv: [0.0, 0.0] }); }
    }
    pub fn quad_glint(&mut self, p: [[f32; 2]; 4], uv: [[f32; 2]; 4], glint_uv: [[f32; 2]; 4], color: u32) {
        for i in [0, 1, 2, 0, 2, 3] { self.vertices.push(HudVertex { pos: p[i], uv: uv[i], color, depth: 0.0, glint_uv: glint_uv[i] }); }
    }
    pub fn quad_3d(&mut self, p: [[f32; 3]; 4], uv: [[f32; 2]; 4], color: u32) {
        for i in [0, 1, 2, 0, 2, 3] { self.vertices.push(HudVertex { pos: [p[i][0], p[i][1]], uv: uv[i], color, depth: p[i][2], glint_uv: [0.0, 0.0] }); }
    }
    pub fn depth_since(&mut self, start: usize, clip: [f32; 4]) {
        let vertices = &mut self.vertices[start..]; if vertices.is_empty() { return; }
        let min = vertices.iter().map(|v| v.depth).fold(f32::INFINITY, f32::min);
        let max = vertices.iter().map(|v| v.depth).fold(f32::NEG_INFINITY, f32::max);
        for v in vertices { v.depth = (max - v.depth) / (max - min).max(f32::EPSILON); }
        let [x0, y0, x1, y1] = clip.map(|v| (v * self.scale).max(0.0) as u32);
        self.blends.push((start as u32..self.vertices.len() as u32, Blend::Depth([x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0)])));
    }
    pub fn rect_uv(&mut self, x: f32, y: f32, w: f32, h: f32, r: Rect, color: u32) {
        self.quad([[x, y], [x + w, y], [x + w, y + h], [x, y + h]],
            [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]], color);
    }
    pub fn sprite(&mut self, atlas: &HudAtlas, name: &str, x: f32, y: f32) {
        if let Some(r) = atlas.sprites.get(name).copied() { self.rect_uv(x, y, r.w as f32, r.h as f32, r, u32::MAX); }
    }
    pub fn sprite_scaled(&mut self, atlas: &HudAtlas, name: &str, x: f32, y: f32, w: f32, h: f32, color: u32) {
        if let Some(r) = atlas.sprites.get(name).copied() { self.rect_uv(x, y, w, h, r, color); }
    }
    pub fn sprite_part(&mut self, atlas: &HudAtlas, name: &str, x: f32, y: f32, fraction: f32) {
        if let Some(mut r) = atlas.sprites.get(name).copied() {
            let w = r.w as f32 * fraction; r.u1 = r.u0 + (r.u1 - r.u0) * fraction;
            self.rect_uv(x, y, w, r.h as f32, r, u32::MAX);
        }
    }
    pub fn scale_since(&mut self, start: usize, cx: f32, cy: f32, sx: f32, sy: f32) {
        for v in &mut self.vertices[start..] { v.pos[0] = cx + (v.pos[0] - cx) * sx; v.pos[1] = cy + (v.pos[1] - cy) * sy; }
    }
    pub fn clip_since(&mut self, start: usize, x0: f32, y0: f32, x1: f32, y1: f32) {
        let quads: Vec<[HudVertex; 6]> = self.vertices[start..].as_chunks::<6>().0.to_vec();
        self.vertices.truncate(start);
        for q in quads {
            let corners = [q[0], q[1], q[2], q[5]];
            let (mut x_lo, mut x_hi) = (f32::INFINITY, f32::NEG_INFINITY);
            let (mut y_lo, mut y_hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for v in &corners { x_lo = x_lo.min(v.pos[0]); x_hi = x_hi.max(v.pos[0]); y_lo = y_lo.min(v.pos[1]); y_hi = y_hi.max(v.pos[1]); }
            let find = |x: f32, y: f32| corners.iter().find(|v| v.pos == [x, y]).copied();
            let rect = (find(x_lo, y_lo), find(x_hi, y_lo), find(x_hi, y_hi), find(x_lo, y_hi));
            let (Some(tl), Some(tr), Some(br), Some(bl)) = rect else {
                let cx = corners.iter().map(|v| v.pos[0]).sum::<f32>() / 4.0;
                let cy = corners.iter().map(|v| v.pos[1]).sum::<f32>() / 4.0;
                if cx >= x0 && cx < x1 && cy >= y0 && cy < y1 { self.vertices.extend_from_slice(&q); }
                continue;
            };
            let (cx0, cy0) = (x_lo.max(x0), y_lo.max(y0));
            let (cx1, cy1) = (x_hi.min(x1), y_hi.min(y1));
            if cx0 >= cx1 || cy0 >= cy1 { continue; }
            let t = |p: f32, lo: f32, hi: f32| if hi == lo { 0.0 } else { (p - lo) / (hi - lo) };
            let uv = |x: f32, y: f32| {
                let (s, r) = (t(x, x_lo, x_hi), t(y, y_lo, y_hi));
                std::array::from_fn(|i| {
                    let top = tl.uv[i] + (tr.uv[i] - tl.uv[i]) * s;
                    let bottom = bl.uv[i] + (br.uv[i] - bl.uv[i]) * s;
                    top + (bottom - top) * r
                })
            };
            self.quad([[cx0, cy0], [cx1, cy0], [cx1, cy1], [cx0, cy1]],
                [uv(cx0, cy0), uv(cx1, cy0), uv(cx1, cy1), uv(cx0, cy1)], tl.color);
        }
    }
    pub fn sprite_region(&mut self, atlas: &HudAtlas, name: &str, [x, y, w, h]: [f32; 4], tw: f32, th: f32) {
        if let Some(mut r) = atlas.sprites.get(name).copied() {
            r.u1 = r.u0 + (r.u1 - r.u0) * w / tw; r.v1 = r.v0 + (r.v1 - r.v0) * h / th;
            self.rect_uv(x, y, w, h, r, u32::MAX);
        }
    }
    pub fn fill(&mut self, atlas: &HudAtlas, x: f32, y: f32, w: f32, h: f32, color: u32) {
        let mut r = atlas.white; let (cu, cv) = ((r.u0 + r.u1) / 2.0, (r.v0 + r.v1) / 2.0);
        r.u0 = cu; r.u1 = cu; r.v0 = cv; r.v1 = cv;
        self.rect_uv(x, y, w, h, r, color);
    }
    pub fn fill_quad(&mut self, atlas: &HudAtlas, corners: [[f32; 2]; 4], color: u32) {
        let w = atlas.white; let uv = [(w.u0 + w.u1) / 2.0, (w.v0 + w.v1) / 2.0];
        self.quad(corners, [uv; 4], color);
    }
    pub fn fill_gradient(&mut self, atlas: &HudAtlas, (x, y, w, h): (f32, f32, f32, f32), top: u32, bottom: u32) {
        let r = atlas.white; let uv = [(r.u0 + r.u1) / 2.0, (r.v0 + r.v1) / 2.0];
        let p = [[x, y], [x + w, y], [x + w, y + h], [x, y + h]]; let c = [top, top, bottom, bottom];
        for i in [0, 1, 2, 0, 2, 3] { self.vertices.push(HudVertex { pos: p[i], uv, color: c[i], depth: 0.0, glint_uv: [0.0, 0.0] }); }
    }
    pub fn text_width(atlas: &HudAtlas, text: &str) -> f32 { atlas.font.width(text) }
    pub fn text(&mut self, atlas: &HudAtlas, text: &str, x: f32, y: f32, color: u32, shadow: bool) -> f32 {
        let t = if text.contains('§') { crate::text::Text::legacy(text) } else { crate::text::Text::literal(text) };
        self.text_styled(atlas, &t, x, y, color, shadow)
    }
    pub fn text_styled(&mut self, atlas: &HudAtlas, text: &crate::text::Text, x: f32, y: f32, color: u32, shadow: bool) -> f32 {
        let text = text.resolved(); let text = crate::bidi::visual(&text);
        let font = &atlas.font; let alpha = (color >> 24) as u8;
        let mut glyphs = Vec::new(); let mut cursor = x;
        for (i, (ch, style)) in text.chars().enumerate() {
            let mut g = font.glyph_styled(ch, style);
            if style.obfuscated == Some(true) && ch != ' ' { g = font.random_glyph(style, g.advance); }
            let c = style.color.map_or(color, |rgb| rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, alpha));
            let bold = style.is_bold();
            let advance = g.advance + if bold { g.offset } else { 0.0 };
            glyphs.push((cursor, g, c, bold, style.is_italic(), style.underlined == Some(true), style.strikethrough == Some(true), i == 0, advance));
            cursor += advance;
        }
        self.patches.extend(font.take_patches());
        let white = font.white();
        let passes: &[bool] = if shadow { &[true, false] } else { &[false] };
        for &is_shadow in passes {
            for &(gx, g, c, bold, italic, underline, strike, first, advance) in &glyphs {
                let (dx, col) = if is_shadow {
                    let b = c.to_le_bytes();
                    (g.offset, rgba((f32::from(b[0]) * 0.25) as u8, (f32::from(b[1]) * 0.25) as u8, (f32::from(b[2]) * 0.25) as u8, b[3]))
                } else { (0.0, c) };
                let (px, py) = (gx + dx, y + dx);
                let shear = |v: f32| if italic { 1.0 - 0.25 * v } else { 0.0 };
                let thick = if bold { 0.1 } else { 0.0 };
                let draw = |batch: &mut Self, ox: f32| match g.rect {
                    Some(r) => {
                        let (x0, x1, y0, y1) = (px + ox + g.left, px + ox + g.right, py + g.top, py + g.bottom);
                        batch.quad([
                            [x0 + shear(g.top) - thick, y0 - thick],
                            [x1 + shear(g.top) + thick, y0 - thick],
                            [x1 + shear(g.bottom) + thick, y1 + thick],
                            [x0 + shear(g.bottom) - thick, y1 + thick]],
                            [[r.u0, r.v0], [r.u1, r.v0], [r.u1, r.v1], [r.u0, r.v1]], col);
                    }
                    None if g.right > 0.0 => {
                        let (bx, by) = (px + ox, py);
                        for (fx, fy, fw, fh) in [(0.0,0.0,5.0,1.0),(0.0,7.0,5.0,1.0),(0.0,1.0,1.0,6.0),(4.0,1.0,1.0,6.0)] {
                            batch.rect_uv(bx + fx, by + fy, fw, fh, white, col);
                        }
                    }
                    None => {}
                };
                draw(self, 0.0);
                if bold { draw(self, g.offset); }
                let x0 = if first { px - 1.0 } else { px };
                if strike { self.rect_uv(x0, py + 3.5, px + advance - x0, 1.0, white, col); }
                if underline { self.rect_uv(x0, py + 8.0, px + advance - x0, 1.0, white, col); }
            }
        }
        cursor - x
    }
}

// ---- OpenGL drawing (the only wgpu-free path) ----------------------------
const VS_HUD: &str = r#"#version 330 core
layout(location=0) in vec2 pos;
layout(location=1) in vec2 uv;
layout(location=2) in vec4 color;
uniform vec2 screen;
out vec2 v_uv; out vec4 v_color;
void main() {
    vec2 p = pos / screen * 2.0 - 1.0;
    gl_Position = vec4(p.x, -p.y, 0.0, 1.0);
    v_uv = uv; v_color = color;
}"#;
const FS_HUD: &str = r#"#version 330 core
in vec2 v_uv; in vec4 v_color;
uniform sampler2D atlas;
out vec4 frag;
void main() { frag = texture(atlas, v_uv) * v_color; }"#;

/// Draw `batch` with OpenGL. `gl` is a glow context, `atlas_tex` is a
/// `GL_TEXTURE_2D` holding `atlas.pixels`, `size` is the framebuffer size.
/// Reuses the caller's VAO/VBO/program (see `hud_program`, `hud_vao`, `hud_vbo`).
pub fn draw_gl(
    gl: &glow::Context,
    hud_program: glow::Program,
    hud_vao: glow::VertexArray,
    hud_vbo: glow::Buffer,
    atlas_tex: glow::Texture,
    screen_loc: &glow::UniformLocation,
    atlas_loc: &glow::UniformLocation,
    batch: &HudBatch,
    size: [u32; 2],
) {
    use glow::HasContext;
    if batch.vertices.is_empty() { return; }
    unsafe {
        gl.use_program(Some(hud_program));
        gl.uniform_2_f32(Some(screen_loc), size[0] as f32 / batch.scale, size[1] as f32 / batch.scale);
        gl.active_texture(glow::TEXTURE0);
        gl.bind_texture(glow::TEXTURE_2D, Some(atlas_tex));
        gl.uniform_1_i32(Some(atlas_loc), 0);
        gl.bind_vertex_array(Some(hud_vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(hud_vbo));
        let bytes: &[u8] = bytemuck::cast_slice(&batch.vertices);
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
        gl.enable(glow::BLEND);
        for (range, blend) in batch.runs() {
            match blend {
                Blend::Alpha => gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA),
                Blend::Invert => gl.blend_func(glow::ONE_MINUS_DST_COLOR, glow::ONE_MINUS_SRC_COLOR),
                Blend::Additive => gl.blend_func(glow::ONE, glow::ONE),
                Blend::Vignette => gl.blend_func(glow::ZERO, glow::ONE_MINUS_SRC_COLOR),
                Blend::Depth(clip) => {
                    gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
                    gl.enable(glow::SCISSOR_TEST);
                    gl.scissor(clip[0] as i32, clip[1] as i32, clip[2] as i32, clip[3] as i32);
                }
                Blend::EndPortal => gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA),
            }
            gl.draw_arrays(glow::TRIANGLES, range.start as i32, range.len() as i32);
            if matches!(blend, Blend::Depth(_)) { gl.disable(glow::SCISSOR_TEST); }
        }
        gl.disable(glow::BLEND);
        gl.bind_vertex_array(None);
    }
}