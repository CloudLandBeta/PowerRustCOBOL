// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! Triangle meshes: the vertex data a scene node draws.

use std::sync::OnceLock;

use super::math::{Aabb, Vec3};
use crate::error::{GraphicsError, Result};

/// The geometry of one mesh. Immutable once built; the scene shares it by `Arc`.
pub struct MeshData {
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Unit normals, one per vertex.
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates (origin top-left, as in glTF), one per vertex.
    pub uvs: Vec<[f32; 2]>,
    /// Triangles as indices, three per triangle, counter-clockwise from the front.
    pub indices: Vec<u32>,
    /// The four joints each vertex follows (empty for a mesh that is not skinned).
    pub joints: Vec<[u16; 4]>,
    /// The weights of those joints (empty for a mesh that is not skinned).
    pub weights: Vec<[f32; 4]>,
    /// The box around the positions.
    pub bounds: Aabb,
    collider: OnceLock<Option<parry3d::shape::TriMesh>>,
}

impl std::fmt::Debug for MeshData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MeshData({} vertices, {} triangles{})", self.positions.len(), self.triangle_count(), if self.is_skinned() { ", skinned" } else { "" })
    }
}

impl MeshData {
    /// Check and assemble a mesh. Missing normals are computed (smooth, area weighted),
    /// missing texture coordinates are zero. `joints` and `weights` are both given or both empty.
    pub fn new(
        positions: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        uvs: Vec<[f32; 2]>,
        indices: Vec<u32>,
        joints: Vec<[u16; 4]>,
        weights: Vec<[f32; 4]>,
    ) -> Result<MeshData> {
        let n = positions.len();
        if n == 0 || indices.is_empty() {
            return Err(GraphicsError::invalid("a mesh needs at least one vertex and one triangle"));
        }
        if indices.len() % 3 != 0 {
            return Err(GraphicsError::invalid("a mesh's index count must be a multiple of three"));
        }
        if indices.iter().any(|&i| i as usize >= n) {
            return Err(GraphicsError::invalid("a mesh triangle refers to a vertex that does not exist"));
        }
        if positions.iter().any(|p| p.iter().any(|v| !v.is_finite())) {
            return Err(GraphicsError::invalid("a mesh vertex position is not a finite number"));
        }
        if !normals.is_empty() && normals.len() != n {
            return Err(GraphicsError::invalid("a mesh needs one normal per vertex"));
        }
        if !uvs.is_empty() && uvs.len() != n {
            return Err(GraphicsError::invalid("a mesh needs one texture coordinate pair per vertex"));
        }
        if joints.len() != weights.len() || (!joints.is_empty() && joints.len() != n) {
            return Err(GraphicsError::invalid("a skinned mesh needs joints and weights for every vertex"));
        }
        let normals = if normals.is_empty() { compute_normals(&positions, &indices) } else { sanitize_normals(normals) };
        let uvs = if uvs.is_empty() { vec![[0.0, 0.0]; n] } else { uvs.into_iter().map(|t| if t[0].is_finite() && t[1].is_finite() { t } else { [0.0, 0.0] }).collect() };
        let weights = weights.into_iter().map(normalize_weights).collect();
        let bounds = Aabb::from_points(positions.iter());
        Ok(MeshData { positions, normals, uvs, indices, joints, weights, bounds, collider: OnceLock::new() })
    }

    /// How many triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Whether the vertices follow joints.
    pub fn is_skinned(&self) -> bool {
        !self.joints.is_empty()
    }

    /// The memory the data holds.
    pub fn memory_bytes(&self) -> u64 {
        (self.positions.len() * 12 + self.normals.len() * 12 + self.uvs.len() * 8 + self.indices.len() * 4 + self.joints.len() * 8 + self.weights.len() * 16) as u64
    }

    /// The collision shape picking uses, built the first time it is needed.
    pub(crate) fn trimesh(&self) -> Option<&parry3d::shape::TriMesh> {
        self.collider
            .get_or_init(|| {
                let verts: Vec<Vec3> = self.positions.iter().map(|p| Vec3::from_array(*p)).collect();
                let tris: Vec<[u32; 3]> = self.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]]).collect();
                parry3d::shape::TriMesh::new(verts, tris).ok()
            })
            .as_ref()
    }
}

fn normalize_weights(w: [f32; 4]) -> [f32; 4] {
    let w = w.map(|v| if v.is_finite() && v > 0.0 { v } else { 0.0 });
    let sum: f32 = w.iter().sum();
    if sum > 0.0 {
        w.map(|v| v / sum)
    } else {
        [1.0, 0.0, 0.0, 0.0]
    }
}

fn sanitize_normals(normals: Vec<[f32; 3]>) -> Vec<[f32; 3]> {
    normals
        .into_iter()
        .map(|n| {
            let v = Vec3::from_array(n);
            if v.is_finite() && v.length_squared() > 1.0e-12 {
                v.normalize().to_array()
            } else {
                [0.0, 1.0, 0.0]
            }
        })
        .collect()
}

/// Smooth normals: every vertex gets the sum of its triangles' (area-weighted) normals.
pub fn compute_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut acc = vec![Vec3::ZERO; positions.len()];
    for t in indices.chunks_exact(3) {
        let (a, b, c) = (Vec3::from_array(positions[t[0] as usize]), Vec3::from_array(positions[t[1] as usize]), Vec3::from_array(positions[t[2] as usize]));
        let n = (b - a).cross(c - a);
        for &i in t {
            acc[i as usize] += n;
        }
    }
    acc.into_iter().map(|n| n.try_normalize().unwrap_or(Vec3::Y).to_array()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tri() -> (Vec<[f32; 3]>, Vec<u32>) {
        (vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]], vec![0, 1, 2])
    }

    #[test]
    fn a_triangle_gets_its_normal_and_bounds() {
        let (p, i) = tri();
        let m = MeshData::new(p, vec![], vec![], i, vec![], vec![]).unwrap();
        assert_eq!(m.normals[0], [0.0, 0.0, 1.0]);
        assert_eq!(m.bounds.max, Vec3::new(1.0, 1.0, 0.0));
        assert_eq!(m.triangle_count(), 1);
        assert!(!m.is_skinned());
        assert!(m.trimesh().is_some());
    }

    #[test]
    fn bad_meshes_are_refused() {
        let (p, i) = tri();
        assert!(MeshData::new(vec![], vec![], vec![], vec![], vec![], vec![]).is_err());
        assert!(MeshData::new(p.clone(), vec![], vec![], vec![0, 1], vec![], vec![]).is_err(), "not a multiple of 3");
        assert!(MeshData::new(p.clone(), vec![], vec![], vec![0, 1, 9], vec![], vec![]).is_err(), "index out of range");
        assert!(MeshData::new(vec![[f32::NAN, 0.0, 0.0]; 3], vec![], vec![], i.clone(), vec![], vec![]).is_err(), "NaN position");
        assert!(MeshData::new(p.clone(), vec![[0.0, 0.0, 1.0]], vec![], i.clone(), vec![], vec![]).is_err(), "one normal for three vertices");
        assert!(MeshData::new(p, vec![], vec![], i, vec![[0; 4]; 3], vec![]).is_err(), "joints without weights");
    }

    #[test]
    fn weights_are_normalised() {
        assert_eq!(normalize_weights([2.0, 2.0, 0.0, 0.0]), [0.5, 0.5, 0.0, 0.0]);
        assert_eq!(normalize_weights([0.0; 4]), [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(normalize_weights([f32::NAN, 1.0, -3.0, 0.0]), [0.0, 1.0, 0.0, 0.0]);
    }
}
