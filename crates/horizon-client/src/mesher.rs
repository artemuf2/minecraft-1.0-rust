//! mesher.rs — Генерация кубов + заглушки для streaming.

use crate::assets::{Assets, RenderAssets};
use crate::blocks::BlockFacts;
use crate::chunk::Chunk;
use horizon_world::block_render::FaceVisibility;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub layer: u32,
    pub color: u32,
    pub light: u32,
    pub normal: [i8; 4],
    pub glint_uv: [f32; 2],
}

pub const CULL_BACK: [i8; 4] = [0, 0, 0, 1];

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TerrainVertex {
    pub pos: [f32; 3],
    pub uv: [u16; 2],
    pub layer: u16,
    pub light: u16,
    pub color: u32,
}

impl TerrainVertex {
    pub fn new(pos: [f32; 3], uv: [f32; 2], layer: u16, color: u32, light: u16) -> Self {
        Self {
            pos,
            uv: uv.map(|v| (v * 32768.0) as u16),
            layer,
            light,
            color,
        }
    }
}

#[derive(Default, Clone)]
pub struct SectionMesh {
    pub solid: Vec<TerrainVertex>,
    pub translucent: Vec<TerrainVertex>,
    pub block_entities: Vec<([i32; 3], u32)>,
}

impl SectionMesh {
    pub fn sort_solid(&mut self) {}
}

#[derive(Clone)]
pub struct MeshTables {
    pub visibility: FaceVisibility,
}

impl MeshTables {
    pub fn new(
        _: &Assets,
        _: &RenderAssets,
        _: &BlockFacts,
        visibility: FaceVisibility,
    ) -> crate::Result<Self> {
        Ok(Self { visibility })
    }
}

pub struct Mesher<'a> {
    pub nether_shading: bool,
    pub zoom: horizon_core::biome::BiomeZoom,
    pub blend_radius: i32,
    pub smooth_lighting: bool,
    pub cutout_leaves: bool,
    pub assets: &'a Assets,
    pub render: &'a RenderAssets,
    pub tables: &'a MeshTables,
    pub biomes: &'a crate::biome::Biomes,
    pub visibility: FaceVisibility,
}

impl<'a> Mesher<'a> {
    pub fn mesh(&mut self, _region: &[[Option<&Chunk>; 3]; 3], _sy: i32) -> SectionMesh {
        SectionMesh::default()
    }

    pub fn mesh_cube(pos: [f32; 3], layer: u32) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        let faces = [
            (
                [[0.0, 1.0, 0.0], [0.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, 0.0]],
                [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
                [0, 1, 0, 0],
            ),
            (
                [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                [0, -1, 0, 0],
            ),
            (
                [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
                [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
                [0, 0, -1, 0],
            ),
            (
                [[0.0, 0.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 1.0]],
                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                [0, 0, 1, 0],
            ),
            (
                [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 1.0], [0.0, 1.0, 0.0]],
                [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
                [-1, 0, 0, 0],
            ),
            (
                [[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [1.0, 1.0, 1.0], [1.0, 0.0, 1.0]],
                [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
                [1, 0, 0, 0],
            ),
        ];
        for (quad_pos, quad_uv, normal) in faces {
            for &i in &[0, 1, 2, 0, 2, 3] {
                let p = quad_pos[i];
                let uv = quad_uv[i];
                vertices.push(Vertex {
                    pos: [pos[0] + p[0], pos[1] + p[1], pos[2] + p[2]],
                    uv,
                    layer,
                    color: 0xFFFFFFFF,
                    light: 0xF0_00F0_00, // sky=15, block=15 (оба в старших/младших 4 битах)
                    normal,
                    glint_uv: [0.0, 0.0],
                });
            }
        }
        vertices
    }

    /// Несколько кубов: (позиция угла, layer). Рисуется за один вызов.
    pub fn mesh_blocks(blocks: &[([f32; 3], u32)]) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        for &(pos, layer) in blocks {
            vertices.extend(Self::mesh_cube(pos, layer));
        }
        vertices
    }
}

pub type Region = [[Option<std::sync::Arc<Chunk>>; 3]; 3];

pub fn borrow(region: &Region) -> [[Option<&Chunk>; 3]; 3] {
    std::array::from_fn(|z| std::array::from_fn(|x| region[z][x].as_deref()))
}