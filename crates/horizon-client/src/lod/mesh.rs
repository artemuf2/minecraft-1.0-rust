//! lod/mesh.rs — Заглушка для LOD.

use super::{Column, Span, TileKey};
use crate::mesher::{SectionMesh, Vertex};

pub(super) struct EdgeColumn {
    pub start: i32,
    pub end: i32,
    pub column: Column,
}

#[derive(Default)]
pub(super) struct Mesher;

impl Mesher {
    pub(super) fn mesh(
        &mut self,
        _key: TileKey,
        _columns: &[Column],
        _covered: &crate::fast_hash::FxHashSet<(i32, i32)>,
        _covered_bounds: Option<[i32; 4]>,
        _context: &crate::streaming::WorkerContext,
        _neighbors: &[Vec<EdgeColumn>; 4],
        _finer: &[TileKey],
    ) -> SectionMesh {
        SectionMesh::default()
    }
}