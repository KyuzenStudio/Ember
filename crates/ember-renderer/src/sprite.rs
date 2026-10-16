use ember_core::Vec2;

use crate::frame::{CachedTexture, find_texture};
use crate::types::{Camera, DrawReq, GpuError, SpriteVertex};

pub fn fit_quad(tex_size: Vec2, surface_size: Vec2) -> [SpriteVertex; 4] {
    let blank = [SpriteVertex {
        position: [0.0, 0.0],
        uv: [0.0, 0.0],
        color: [1.0, 1.0, 1.0, 1.0],
    }; 4];
    if tex_size.x <= 0.0 || tex_size.y <= 0.0 || surface_size.x <= 0.0 || surface_size.y <= 0.0 {
        return blank;
    }
    let scale = (surface_size.x / tex_size.x).min(surface_size.y / tex_size.y) * 0.9;
    let hx = tex_size.x * scale / surface_size.x;
    let hy = tex_size.y * scale / surface_size.y;
    [
        SpriteVertex {
            position: [-hx, hy],
            uv: [0.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        SpriteVertex {
            position: [hx, hy],
            uv: [1.0, 0.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        SpriteVertex {
            position: [hx, -hy],
            uv: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
        SpriteVertex {
            position: [-hx, -hy],
            uv: [0.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        },
    ]
}

pub fn sprite_quad(center: Vec2, size: Vec2, rotation_degrees: f32) -> [Vec2; 4] {
    let angle = rotation_degrees.to_radians();
    let (sin, cos) = angle.sin_cos();
    let hx = size.x / 2.0;
    let hy = size.y / 2.0;
    [(-hx, hy), (hx, hy), (hx, -hy), (-hx, -hy)]
        .map(|(x, y)| Vec2::new(center.x + x * cos - y * sin, center.y + x * sin + y * cos))
}

pub fn scene_sprites(
    scene: &ember_scene::Scene,
    sizes: &std::collections::HashMap<String, (u32, u32)>,
) -> (Camera, Vec<DrawReq>) {
    let camera = scene
        .entities
        .iter()
        .find_map(|e| {
            e.camera.as_ref().map(|c| Camera {
                center: e.transform.position,
                height: c.height,
            })
        })
        .unwrap_or(Camera {
            center: Vec2::zero(),
            height: 12.0,
        });
    let mut ordered: Vec<(i32, DrawReq)> = Vec::new();
    for entity in &scene.entities {
        let Some(sprite) = &entity.sprite else {
            continue;
        };
        let Some(texture) = entity.frame_texture() else {
            continue;
        };
        let Some((w, h)) = sizes.get(texture).copied() else {
            continue;
        };
        let aspect = w as f32 / h.max(1) as f32;
        let size = Vec2::new(aspect * entity.transform.scale.x, entity.transform.scale.y);
        let corners = sprite_quad(
            entity.transform.position,
            size,
            entity.transform.rotation_degrees,
        );
        ordered.push((
            sprite.sorting_order,
            DrawReq {
                texture: texture.to_string(),
                corners,
                tint: sprite.tint,
            },
        ));
    }
    ordered.sort_by_key(|(order, _)| *order);
    (camera, ordered.into_iter().map(|(_, d)| d).collect())
}

pub fn pick_top(quads: &[[Vec2; 4]], point: Vec2) -> Option<usize> {
    quads.iter().rposition(|quad| {
        let signs: Vec<f32> = quad
            .iter()
            .enumerate()
            .map(|(i, corner)| {
                let next = quad[(i + 1) % 4];
                let edge = next - *corner;
                let to_point = point - *corner;
                edge.x * to_point.y - edge.y * to_point.x
            })
            .collect();
        signs.iter().all(|s| *s >= 0.0) || signs.iter().all(|s| *s <= 0.0)
    })
}

pub(crate) fn build_verts(
    camera: &Camera,
    aspect: f32,
    textures: &[CachedTexture],
    sprites: &[DrawReq],
) -> Result<Vec<SpriteVertex>, GpuError> {
    let uvs = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut verts = Vec::with_capacity(sprites.len() * 4);
    for sprite in sprites {
        find_texture(textures, &sprite.texture)
            .ok_or_else(|| GpuError(format!("texture '{}' not cached", sprite.texture)))?;
        for (i, corner) in sprite.corners.iter().enumerate() {
            let p = camera.ndc(*corner, aspect);
            verts.push(SpriteVertex {
                position: p,
                uv: uvs[i],
                color: sprite.tint,
            });
        }
    }
    Ok(verts)
}
