//! assets.rs — загрузка реестра, текстур блоков и атласа GUI.

use horizon_core::position::BlockPos;
use horizon_world::registry::{BlockRegistry, BlockStateId};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

type StdResult<T> = std::result::Result<T, String>;

#[derive(Clone, Default)]
pub struct DummyModels;
impl DummyModels {
    pub fn collect(&self, _state: BlockStateId, _pos: BlockPos) -> StdResult<Vec<()>> {
        Err("no models".to_string())
    }
    pub fn models_at(&self, _state: BlockStateId, _pos: BlockPos, _out: &mut Vec<()>) -> StdResult<()> {
        Err("no models".to_string())
    }
}

#[derive(Clone, Copy)]
pub struct StateFacts;
impl horizon_world::heightmap::HeightmapState for StateFacts {
    fn is_air(self) -> bool { false }
    fn is_air_block(self) -> bool { false }
    fn has_fluid(self) -> bool { false }
    fn blocks_motion_in_heightmap(self) -> bool { false }
    fn blocks_motion_in_heightmap_no_leaves(self) -> bool { false }
}

#[derive(Clone)]
pub struct Properties;
impl horizon_world::worldgen::terrain::TerrainBlockStates for Properties {
    type State = StateFacts;
    fn heightmap_state(&self, _: BlockStateId) -> StateFacts { StateFacts }
}

pub struct GuiAtlas {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// (u0, v0, u1, v1) для каждой спрайт-области.
    pub rects: HashMap<&'static str, [f32; 4]>,
}

pub struct Assets {
    pub registry: &'static BlockRegistry,
    pub properties: Properties,
    pub material_states: [BlockStateId; 6],
    pub blocks_fluid_flow: BTreeSet<String>,

    pub block_pixels: Vec<u8>,
    pub block_size: u32,
    pub block_layers: u32,
    pub block_names: HashMap<String, u32>,

    pub font_pixels: Vec<u8>,
    pub font_w: u32,
    pub font_h: u32,

    pub gui: GuiAtlas,
}

pub struct RenderAssets {
    pub sprite_size: u32,
    pub sprite_minimum_size: u32,
    pub pixels: Vec<u8>,
    pub texture_layers: u32,
    pub sprite_layers: BTreeMap<String, u32>,
    pub sprite_mips: Vec<()>,
    pub translucent_layers: Vec<bool>,
    pub animations: Vec<()>,
    pub model_catalog: DummyModels,
    pub model_library: StdResult<std::sync::Arc<()>>,
    pub model_placements: Vec<(BlockPos, BlockStateId)>,
    pub model_gallery: Vec<()>,
    pub gui_pixels: Vec<u8>,
    pub gui_width: u32,
    pub gui_height: u32,
}

impl Assets {
    pub fn load(root: &Path) -> StdResult<Self> {
        println!("[Assets] Загрузка из {}", root.display());

        let registry = load_registry(root)?;
        let (block_pixels, block_layers, block_names) = load_blocks(&root.join("textures"))?;
        let (font_pixels, font_w, font_h) = load_font(&root.join("font"))?;
        let gui = load_gui_atlas(&root.join("gui"))?;

        Ok(Self {
            registry,
            properties: Properties,
            material_states: [BlockStateId(0); 6],
            blocks_fluid_flow: BTreeSet::new(),
            block_pixels,
            block_size: 16,
            block_layers,
            block_names,
            font_pixels,
            font_w,
            font_h,
            gui,
        })
    }

    pub fn block_layer(&self, name: &str) -> Option<u32> {
        self.block_names.get(name).copied()
    }

    pub fn reload(&self, root: &Path) -> StdResult<RenderAssets> {
        let (gui_pixels, gui_width, gui_height) = (
            self.gui.pixels.clone(),
            self.gui.width,
            self.gui.height,
        );
        Ok(RenderAssets {
            sprite_size: self.block_size,
            sprite_minimum_size: self.block_size,
            pixels: self.block_pixels.clone(),
            texture_layers: self.block_layers,
            sprite_layers: BTreeMap::new(),
            sprite_mips: Vec::new(),
            translucent_layers: vec![false; self.block_layers as usize],
            animations: Vec::new(),
            model_catalog: DummyModels,
            model_library: Err("stub".to_string()),
            model_placements: Vec::new(),
            model_gallery: Vec::new(),
            gui_pixels,
            gui_width,
            gui_height,
        })
    }
}

fn load_registry(root: &Path) -> StdResult<&'static BlockRegistry> {
    let facts_path = root.join("facts.json");
    let file = std::fs::File::open(&facts_path)
        .map_err(|e| format!("не могу открыть {}: {e}", facts_path.display()))?;
    let registry = BlockRegistry::from_facts_json(file)
        .map_err(|e| format!("не могу разобрать {}: {e}", facts_path.display()))?;
    println!("[Assets] Реестр загружен из {}", facts_path.display());
    Ok(Box::leak(Box::new(registry)))
}

fn load_blocks(dir: &Path) -> StdResult<(Vec<u8>, u32, HashMap<String, u32>)> {
    let mut pixels = Vec::new();
    let mut names = HashMap::new();
    let mut count = 0;

    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("Не могу прочитать {}: {}", dir.display(), e))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("png"))
        .collect();
    files.sort();

    for path in files {
        let img = image::open(&path)
            .map_err(|e| format!("{}: {}", path.display(), e))?
            .to_rgba8();
        if img.width() != 16 || img.height() != 16 {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("<unknown>")
            .to_string();
        names.insert(name, count);
        pixels.extend_from_slice(&img.into_raw());
        count += 1;
    }
    println!("[Assets] Загружено текстур блоков: {}", count);
    Ok((pixels, count, names))
}

fn load_font(dir: &Path) -> StdResult<(Vec<u8>, u32, u32)> {
    let path = dir.join("default.png");
    let img = image::open(&path)
        .map_err(|e| format!("{}: {}", path.display(), e))?
        .to_rgba8();
    let (w, h) = (img.width(), img.height());
    println!("[Assets] Шрифт: {}x{}", w, h);
    Ok((img.into_raw(), w, h))
}

/// Склеивает отдельные PNG-файлы кнопок/хотбара в один атлас.
/// Возвращает пиксели + координаты UV для каждого элемента.
fn load_gui_atlas(dir: &Path) -> StdResult<GuiAtlas> {
    // Порядок — важен: каждый элемент кладём в свой слот.
    let items: &[(&'static str, &str)] = &[
        ("button_normal",   "button_normal.png"),
        ("button_hover",    "button_hover.png"),
        ("button_disabled", "button_disabled.png"),
        ("hotbar",          "hotbar.png"),
        ("hotbar_selection","hotbar_selection.png"),
        ("bg",              "gui.png"),
    ];

    // Определяем общий размер: суммарная ширина, максимальная высота.
    let mut loaded: Vec<(&'static str, image::RgbaImage)> = Vec::new();
    for (key, file) in items {
        let path = dir.join(file);
        if !path.exists() {
            println!("[Assets] Пропуск {}: нет файла", file);
            continue;
        }
        match image::open(&path) {
            Ok(img) => loaded.push((key, img.to_rgba8())),
            Err(e) => println!("[Assets] {}: {e}", file),
        }
    }
    if loaded.is_empty() {
        // Пустой атлас 16×16 (белый) — чтобы шейдер не падал.
        return Ok(GuiAtlas {
            pixels: vec![255u8; 16 * 16 * 4],
            width: 16,
            height: 16,
            rects: HashMap::new(),
        });
    }
    let total_w: u32 = loaded.iter().map(|(_, i)| i.width()).sum();
    let max_h: u32 = loaded.iter().map(|(_, i)| i.height()).max().unwrap_or(16);
    let mut pixels = vec![0u8; (total_w * max_h * 4) as usize];
    let mut rects = HashMap::new();
    let mut x = 0u32;
    for (key, img) in &loaded {
        let (w, h) = (img.width(), img.height());
        let src = img.as_raw();
        for row in 0..h {
            let dst = ((row * total_w + x) * 4) as usize;
            let src_i = (row * w * 4) as usize;
            pixels[dst..dst + (w * 4) as usize]
                .copy_from_slice(&src[src_i..src_i + (w * 4) as usize]);
        }
        let u0 = x as f32 / total_w as f32;
        let v0 = 0.0;
        let u1 = (x + w) as f32 / total_w as f32;
        let v1 = h as f32 / max_h as f32;
        rects.insert(*key, [u0, v0, u1, v1]);
        x += w;
    }
    println!("[Assets] GUI-атлас: {}x{} ({} элементов)", total_w, max_h, rects.len());
    Ok(GuiAtlas { pixels, width: total_w, height: max_h, rects })
}