//! Primitive-emission half of the DrawList.swift translation.
use crate::{
    Color, DrawError, DrawList, GlyphAtlasFormat, GlyphAtlasInfo, LinearGradient, Point, Rect,
    RoundedRect, TextureId, Vertex, color_glyph_atlas_id,
};
unsafe extern "C" {
    fn cove_shape_rounded(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        scale: f32,
        segments: usize,
        color: u32,
        vertices: *mut Vertex,
        indices: *mut u32,
    );
    fn cove_shape_line(
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        thickness: f32,
        scale: f32,
        color: u32,
        vertices: *mut Vertex,
        indices: *mut u32,
        vertex_count: *mut usize,
        index_count: *mut usize,
    );
    fn cove_shape_image_corner(
        cx: f32,
        cy: f32,
        radius: f32,
        start: f32,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: u32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        segments: usize,
        vertices: *mut Vertex,
        indices: *mut u32,
    );
}
impl DrawList {
    pub fn add_rect(&mut self, rect: Rect, color: Color) -> Result<(), DrawError> {
        let rect = self.snapped(rect);
        self.quad(
            rect,
            color,
            0,
            Point { x: -1.0, y: 0.0 },
            Point { x: -1.0, y: 0.0 },
        )
    }
    pub fn add_rounded_rect(
        &mut self,
        rect: Rect,
        radius: f32,
        color: Color,
    ) -> Result<(), DrawError> {
        let rect = self.snapped(rect);
        let radius = radius.min(rect.width.min(rect.height) * 0.5).max(0.0);
        if radius <= 0.0 {
            return self.add_rect(rect, color);
        }
        let half = (0.5 / self.pixel_scale.max(1.0)).min(rect.width.min(rect.height) * 0.5);
        let segments = self.corner_segment_count(radius + half);
        let pairs = 4 * (segments + 1);
        let mut vertices = vec![Vertex::default(); 1 + pairs * 2];
        let mut indices = vec![0; pairs * 9];
        // SAFETY: native code writes exactly 1 + pairs*2 vertices and pairs*9 indices into these buffers.
        unsafe {
            cove_shape_rounded(
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                radius,
                self.pixel_scale,
                segments,
                color.rgba8(),
                vertices.as_mut_ptr(),
                indices.as_mut_ptr(),
            );
        }
        self.append_geometry(&vertices, &indices, 0)
    }
    pub fn add_rounded_rect_stroke(
        &mut self,
        rect: Rect,
        radius: f32,
        width: f32,
        color: Color,
    ) -> Result<(), DrawError> {
        if rect.width <= 0.0 || rect.height <= 0.0 || width <= 0.0 || color.a <= 0.0 {
            return Ok(());
        }
        let rect = self.snapped(rect);
        let stroke = width.min(rect.width.min(rect.height) * 0.5);
        let outer_radius = radius.min(rect.width.min(rect.height) * 0.5).max(0.0);
        if rect.width - 2.0 * stroke <= 0.0 || rect.height - 2.0 * stroke <= 0.0 {
            return self.add_rounded_rect(rect, outer_radius, color);
        }
        let center = Rect::new(
            rect.x + stroke / 2.0,
            rect.y + stroke / 2.0,
            rect.width - stroke,
            rect.height - stroke,
        );
        let radius = (outer_radius - stroke / 2.0).max(0.0);
        let points =
            RoundedRect::new(center, radius).outline(self.corner_segment_count(radius.max(1.0)));
        self.add_path_stroke(&points, stroke, color, true)
    }
    pub fn add_linear_gradient(
        &mut self,
        rect: Rect,
        radius: f32,
        gradient: LinearGradient,
    ) -> Result<(), DrawError> {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return Ok(());
        }
        let first = self.vertices.len();
        self.add_rounded_rect(rect, radius, Color::WHITE)?;
        self.recolor(first, |x, y, coverage| {
            gradient
                .color_at(Point { x, y }, rect)
                .multiplied_alpha(coverage)
        });
        Ok(())
    }
    pub fn add_triangle(
        &mut self,
        a: Point,
        b: Point,
        c: Point,
        color: Color,
    ) -> Result<(), DrawError> {
        let vertices = [a, b, c].map(|p| Vertex {
            x: p.x,
            y: p.y,
            u: -1.0,
            v: 0.0,
            color: color.rgba8(),
        });
        self.append_geometry(&vertices, &[0, 1, 2], 0)
    }
    pub fn add_line(
        &mut self,
        from: Point,
        to: Point,
        thickness: f32,
        color: Color,
    ) -> Result<(), DrawError> {
        let length = ((to.x - from.x).powi(2) + (to.y - from.y).powi(2)).sqrt();
        if !length.is_finite() || length <= 0.001 {
            return Ok(());
        }
        let mut vertices = vec![Vertex::default(); 50];
        let mut indices = vec![0; 162];
        let mut vertex_count = 0;
        let mut index_count = 0;
        // SAFETY: the translated fixed eight-segment cap implementation requires at most 50 vertices and 162 indices.
        unsafe {
            cove_shape_line(
                from.x,
                from.y,
                to.x,
                to.y,
                thickness,
                self.pixel_scale,
                color.rgba8(),
                vertices.as_mut_ptr(),
                indices.as_mut_ptr(),
                &mut vertex_count,
                &mut index_count,
            );
        }
        vertices.truncate(vertex_count);
        indices.truncate(index_count);
        self.append_geometry(&vertices, &indices, 0)
    }
    pub fn add_atlas_glyph(
        &mut self,
        info: GlyphAtlasInfo,
        origin: Point,
        color: Color,
        texture_id: TextureId,
    ) -> Result<(), DrawError> {
        let rect = Rect::new(origin.x, origin.y, info.width, info.height);
        let min = Point {
            x: info.uv_min_x,
            y: info.uv_min_y,
        };
        let max = Point {
            x: info.uv_max_x,
            y: info.uv_max_y,
        };
        if info.format == GlyphAtlasFormat::Color {
            self.add_image_quad(
                rect,
                color_glyph_atlas_id(texture_id),
                Color {
                    a: color.a,
                    ..Color::WHITE
                },
                min,
                max,
            )
        } else {
            self.add_glyph_quad(rect, texture_id, color, min, max)
        }
    }
    pub fn add_glyph_quad(
        &mut self,
        rect: Rect,
        texture_id: TextureId,
        color: Color,
        min: Point,
        max: Point,
    ) -> Result<(), DrawError> {
        self.quad(rect, color, texture_id, min, max)
    }
    pub fn add_image_quad(
        &mut self,
        rect: Rect,
        texture_id: TextureId,
        tint: Color,
        min: Point,
        max: Point,
    ) -> Result<(), DrawError> {
        self.quad(
            rect,
            tint,
            texture_id,
            Point {
                x: min.x + 10.0,
                y: min.y,
            },
            Point {
                x: max.x + 10.0,
                y: max.y,
            },
        )
    }
    pub fn add_image_mask_quad(
        &mut self,
        rect: Rect,
        texture_id: TextureId,
        tint: Color,
        min: Point,
        max: Point,
    ) -> Result<(), DrawError> {
        self.quad(
            rect,
            tint,
            texture_id,
            Point {
                x: min.x + 20.0,
                y: min.y,
            },
            Point {
                x: max.x + 20.0,
                y: max.y,
            },
        )
    }
    pub fn add_clamped_image_quad(
        &mut self,
        rect: Rect,
        texture_id: TextureId,
        texel_size: Point,
        tint: Color,
        min: Point,
        max: Point,
    ) -> Result<(), DrawError> {
        if rect.width <= 0.0
            || rect.height <= 0.0
            || max.x <= min.x
            || max.y <= min.y
            || texel_size.x <= 0.0
            || texel_size.y <= 0.0
        {
            return Ok(());
        }
        let inset_x = (texel_size.x / 2.0).min((max.x - min.x) / 2.0);
        let inset_y = (texel_size.y / 2.0).min((max.y - min.y) / 2.0);
        let edge_x = rect.width * inset_x / (max.x - min.x);
        let edge_y = rect.height * inset_y / (max.y - min.y);
        let xs = [
            rect.x,
            rect.x + edge_x,
            rect.x + rect.width - edge_x,
            rect.x + rect.width,
        ];
        let ys = [
            rect.y,
            rect.y + edge_y,
            rect.y + rect.height - edge_y,
            rect.y + rect.height,
        ];
        let us = [
            min.x + inset_x,
            min.x + inset_x,
            max.x - inset_x,
            max.x - inset_x,
        ];
        let vs = [
            min.y + inset_y,
            min.y + inset_y,
            max.y - inset_y,
            max.y - inset_y,
        ];
        for y in 0..3 {
            for x in 0..3 {
                if xs[x + 1] > xs[x] && ys[y + 1] > ys[y] {
                    self.add_image_quad(
                        Rect::new(xs[x], ys[y], xs[x + 1] - xs[x], ys[y + 1] - ys[y]),
                        texture_id,
                        tint,
                        Point { x: us[x], y: vs[y] },
                        Point {
                            x: us[x + 1],
                            y: vs[y + 1],
                        },
                    )?;
                }
            }
        }
        Ok(())
    }
    pub fn add_rounded_image_quad(
        &mut self,
        rect: Rect,
        radius: f32,
        texture_id: TextureId,
        tint: Color,
        min: Point,
        max: Point,
    ) -> Result<(), DrawError> {
        let radius = radius.min(rect.width.min(rect.height) * 0.5).max(0.0);
        if radius <= 0.0 {
            return self.add_image_quad(rect, texture_id, tint, min, max);
        }
        for part in [
            Rect::new(
                rect.x + radius,
                rect.y,
                rect.width - 2.0 * radius,
                rect.height,
            ),
            Rect::new(rect.x, rect.y + radius, radius, rect.height - 2.0 * radius),
            Rect::new(
                rect.x + rect.width - radius,
                rect.y + radius,
                radius,
                rect.height - 2.0 * radius,
            ),
        ] {
            if part.width <= 0.0 || part.height <= 0.0 {
                continue;
            }
            let map = |x, y| Point {
                x: min.x + (max.x - min.x) * (x - rect.x) / rect.width,
                y: min.y + (max.y - min.y) * (y - rect.y) / rect.height,
            };
            self.add_image_quad(
                part,
                texture_id,
                tint,
                map(part.x, part.y),
                map(part.x + part.width, part.y + part.height),
            )?;
        }
        let segments = self.corner_segment_count(radius);
        for (center, start) in [
            (
                Point {
                    x: rect.x + radius,
                    y: rect.y + radius,
                },
                std::f32::consts::PI,
            ),
            (
                Point {
                    x: rect.x + rect.width - radius,
                    y: rect.y + radius,
                },
                -std::f32::consts::PI / 2.0,
            ),
            (
                Point {
                    x: rect.x + rect.width - radius,
                    y: rect.y + rect.height - radius,
                },
                0.0,
            ),
            (
                Point {
                    x: rect.x + radius,
                    y: rect.y + rect.height - radius,
                },
                std::f32::consts::PI / 2.0,
            ),
        ] {
            let mut vertices = vec![Vertex::default(); segments + 2];
            let mut indices = vec![0; segments * 3];
            // SAFETY: the corner fan writes segments+2 vertices and segments*3 indices; all input values are by value.
            unsafe {
                cove_shape_image_corner(
                    center.x,
                    center.y,
                    radius,
                    start,
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    tint.rgba8(),
                    min.x,
                    min.y,
                    max.x,
                    max.y,
                    segments,
                    vertices.as_mut_ptr(),
                    indices.as_mut_ptr(),
                );
            }
            self.append_geometry(&vertices, &indices, texture_id)?;
        }
        Ok(())
    }
    fn quad(
        &mut self,
        rect: Rect,
        color: Color,
        texture_id: TextureId,
        min: Point,
        max: Point,
    ) -> Result<(), DrawError> {
        let color = color.rgba8();
        let vertices = [
            Vertex {
                x: rect.x,
                y: rect.y,
                u: min.x,
                v: min.y,
                color,
            },
            Vertex {
                x: rect.x + rect.width,
                y: rect.y,
                u: max.x,
                v: min.y,
                color,
            },
            Vertex {
                x: rect.x + rect.width,
                y: rect.y + rect.height,
                u: max.x,
                v: max.y,
                color,
            },
            Vertex {
                x: rect.x,
                y: rect.y + rect.height,
                u: min.x,
                v: max.y,
                color,
            },
        ];
        self.append_geometry(&vertices, &[0, 1, 2, 0, 2, 3], texture_id)
    }
}
