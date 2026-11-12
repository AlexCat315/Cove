use std::sync::Arc;

use cove_imaging::{DecodedImage, ImageAsset, ImageDecodeError};

pub(super) const WIDTH: u32 = 576;
pub(super) const HEIGHT: u32 = 352;
pub(super) const GRID: usize = 128;
pub(super) const FLOOR_MIN: f32 = -3.0;
pub(super) const GRID_STEP: f32 = -2.0 * FLOOR_MIN / GRID as f32;
pub(super) const LIGHT_SAMPLES: usize = 8;
pub(super) type ShadowMask = Vec<u8>;

// A rotated four-sample pattern, in pixels relative to the pixel center.
const PIXEL_SAMPLES: [[f32; 2]; 4] = [
    [-0.125, -0.375],
    [0.375, -0.125],
    [-0.375, 0.125],
    [0.125, 0.375],
];

type Point = [f32; 3];

struct Pixel {
    depth: [f32; PIXEL_SAMPLES.len()],
    color: [[u8; 3]; PIXEL_SAMPLES.len()],
}

impl Pixel {
    fn resolve(self) -> [u8; 4] {
        let [a, b, c, d] = self.color;
        if a == b && a == c && a == d {
            return [a[0], a[1], a[2], 255];
        }
        let average = |channel: usize| {
            let sum = u16::from(a[channel])
                + u16::from(b[channel])
                + u16::from(c[channel])
                + u16::from(d[channel]);
            ((sum + 2) / 4) as u8
        };
        [average(0), average(1), average(2), 255]
    }
}

#[derive(Clone, Copy)]
pub(super) struct Triangle {
    pub points: [Point; 3],
    normal: Point,
    color: Point,
    floor: bool,
}

pub(super) fn occluders() -> Vec<Triangle> {
    [
        cuboid(
            [-1.47, 0.0, -0.72],
            [-0.48, 1.48, 0.25],
            [104.0, 145.0, 246.0],
        ),
        cuboid(
            [0.24, 0.48, -0.86],
            [1.37, 1.28, 0.21],
            [248.0, 165.0, 89.0],
        ),
        cuboid([-0.40, 0.0, 0.79], [0.51, 0.61, 1.64], [89.0, 197.0, 180.0]),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(super) fn light_center(position: u8) -> Point {
    [-3.0 + 0.375 * f32::from(position), 4.2, -2.65]
}

pub(super) fn light_samples(position: u8) -> [Point; LIGHT_SAMPLES] {
    let center = light_center(position);
    [
        [-0.58, 0.0, -0.28],
        [-0.28, 0.0, -0.58],
        [0.28, 0.0, -0.58],
        [0.58, 0.0, -0.28],
        [0.58, 0.0, 0.28],
        [0.28, 0.0, 0.58],
        [-0.28, 0.0, 0.58],
        [-0.58, 0.0, 0.28],
    ]
    .map(|offset| add(center, offset))
}

pub(super) fn raster_shadow_mask(geometry: &[Triangle], light: u8) -> ShadowMask {
    let mut mask = vec![0; GRID * GRID];
    let mut silhouette = vec![false; mask.len()];
    for source in light_samples(light) {
        silhouette.fill(false);
        for triangle in geometry {
            let projected = triangle.points.map(|point| {
                let scale = source[1] / (source[1] - point[1]);
                let floor = add(source, mul(sub(point, source), scale));
                [
                    (floor[0] - FLOOR_MIN) / GRID_STEP,
                    (floor[2] - FLOOR_MIN) / GRID_STEP,
                ]
            });
            rasterize(projected, GRID, GRID, [[0.0, 0.0]], |pixel, _, _| {
                silhouette[pixel] = true;
            });
        }
        for (value, blocked) in mask.iter_mut().zip(&silhouette) {
            *value += u8::from(*blocked);
        }
    }
    mask
}

pub(super) fn render(
    geometry: &[Triangle],
    mask: &[u8],
    light: u8,
) -> Result<Arc<ImageAsset>, ImageDecodeError> {
    let width = WIDTH as usize;
    let height = HEIGHT as usize;
    let mut raster: Vec<_> = (0..width * height)
        .map(|index| {
            let x = (index % width) as f32 / WIDTH as f32 - 0.5;
            let y = (index / width) as f32 / HEIGHT as f32 - 0.5;
            let glow = (1.0 - 2.0 * x * x - 2.4 * y * y).max(0.0);
            let color = [
                channel(14.0 + 12.0 * glow),
                channel(19.0 + 16.0 * glow),
                channel(30.0 + 24.0 * glow),
            ];
            Pixel {
                depth: [f32::NEG_INFINITY; PIXEL_SAMPLES.len()],
                color: [color; PIXEL_SAMPLES.len()],
            }
        })
        .collect();
    let camera = normalize([4.5, 4.1, 6.3]);
    let right = normalize([camera[2], 0.0, -camera[0]]);
    let up = cross(camera, right);
    let screen_scale = 57.0;
    let light = light_center(light);
    let stage = cuboid(
        [FLOOR_MIN, -0.18, FLOOR_MIN],
        [-FLOOR_MIN, 0.0, -FLOOR_MIN],
        [153.0, 169.0, 191.0],
    );
    for triangle in stage.iter().chain(geometry) {
        let facing = dot(triangle.normal, camera);
        if facing <= 0.0 {
            continue;
        }
        // Intersect each subpixel's orthographic ray with the plane of the face.
        let depth_offsets = PIXEL_SAMPLES.map(|[x, y]| {
            (y * dot(triangle.normal, up) - x * dot(triangle.normal, right))
                / (screen_scale * facing)
        });
        let screen = triangle.points.map(|point| {
            [
                WIDTH as f32 * 0.5 + screen_scale * dot(point, right),
                HEIGHT as f32 * 0.50 - screen_scale * dot(sub(point, [0.0, 0.33, 0.0]), up),
            ]
        });
        rasterize(
            screen,
            width,
            height,
            PIXEL_SAMPLES,
            |index, weights, mut coverage| {
                let point = add(
                    add(
                        mul(triangle.points[0], weights[0]),
                        mul(triangle.points[1], weights[1]),
                    ),
                    mul(triangle.points[2], weights[2]),
                );
                let distance = dot(point, camera);
                let pixel = &mut raster[index];
                for (sample, (stored_depth, offset)) in
                    pixel.depth.iter_mut().zip(depth_offsets).enumerate()
                {
                    let bit = 1 << sample;
                    if coverage & bit == 0 {
                        continue;
                    }
                    let sample_depth = distance + offset;
                    if sample_depth > *stored_depth {
                        *stored_depth = sample_depth;
                    } else {
                        coverage &= !bit;
                    }
                }
                if coverage == 0 {
                    return;
                }
                let to_light = sub(light, point);
                let direct = dot(triangle.normal, normalize(to_light)).max(0.0);
                let (illumination, grid) = if triangle.floor {
                    let visibility = 1.0 - sample_mask(mask, point) / LIGHT_SAMPLES as f32;
                    let grid = (line(point[0]).max(line(point[2]))) * 0.10;
                    (0.44 + direct * (0.09 + 0.68 * visibility), grid)
                } else {
                    (0.46 + 0.72 * direct + 0.06 * triangle.normal[1], 0.0)
                };
                let rgb = triangle
                    .color
                    .map(|value| channel(value * illumination * (1.0 - grid)));
                for (sample, color) in pixel.color.iter_mut().enumerate() {
                    if coverage & (1 << sample) != 0 {
                        *color = rgb;
                    }
                }
            },
        );
    }
    let pixels: Vec<_> = raster.into_iter().flat_map(Pixel::resolve).collect();
    let image = DecodedImage::new(WIDTH, HEIGHT, pixels).expect("fixed RGBA preview dimensions");
    ImageAsset::new(image)
}

fn cuboid(min: Point, max: Point, color: Point) -> [Triangle; 12] {
    let [x0, y0, z0] = min;
    let [x1, y1, z1] = max;
    let points = [
        [x0, y0, z0],
        [x1, y0, z0],
        [x1, y0, z1],
        [x0, y0, z1],
        [x0, y1, z0],
        [x1, y1, z0],
        [x1, y1, z1],
        [x0, y1, z1],
    ];
    let faces = [
        ([0, 1, 2, 3], [0.0, -1.0, 0.0]),
        ([4, 7, 6, 5], [0.0, 1.0, 0.0]),
        ([0, 4, 5, 1], [0.0, 0.0, -1.0]),
        ([3, 2, 6, 7], [0.0, 0.0, 1.0]),
        ([0, 3, 7, 4], [-1.0, 0.0, 0.0]),
        ([1, 5, 6, 2], [1.0, 0.0, 0.0]),
    ];
    std::array::from_fn(|index| {
        let (corners, normal) = faces[index / 2];
        let corners = if index % 2 == 0 {
            [corners[0], corners[1], corners[2]]
        } else {
            [corners[0], corners[2], corners[3]]
        };
        Triangle {
            points: corners.map(|corner| points[corner]),
            normal,
            color,
            floor: y1 == 0.0 && normal[1] == 1.0,
        }
    })
}

fn rasterize<const N: usize>(
    triangle: [[f32; 2]; 3],
    width: usize,
    height: usize,
    samples: [[f32; 2]; N],
    mut draw: impl FnMut(usize, Point, u8),
) {
    let edge = |a: [f32; 2], b: [f32; 2], p: [f32; 2]| {
        (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
    };
    let [a, b, c] = triangle;
    let area = edge(a, b, c);
    if area.abs() < 0.0001 {
        return;
    }
    let bounds = |axis: usize, limit: usize| {
        let min = a[axis].min(b[axis]).min(c[axis]).floor().max(0.0) as usize;
        let max = a[axis].max(b[axis]).max(c[axis]).ceil() as usize;
        min..max.min(limit)
    };
    let inverse_area = 1.0 / area;
    let edges = [(b, c), (c, a), (a, b)];
    let sample_deltas = samples.map(|[x, y]| {
        edges.map(|(from, to)| ((to[0] - from[0]) * y - (to[1] - from[1]) * x) * inverse_area)
    });
    let min_delta: Point = std::array::from_fn(|axis| {
        sample_deltas
            .iter()
            .map(|delta| delta[axis])
            .fold(f32::INFINITY, f32::min)
    });
    let max_delta: Point = std::array::from_fn(|axis| {
        sample_deltas
            .iter()
            .map(|delta| delta[axis])
            .fold(f32::NEG_INFINITY, f32::max)
    });
    let inside = |weights: Point, delta: Point| {
        weights[0] + delta[0] >= -0.00001
            && weights[1] + delta[1] >= -0.00001
            && weights[2] + delta[2] >= -0.00001
    };
    let x_range = bounds(0, width);
    for y in bounds(1, height) {
        for x in x_range.clone() {
            let point = [x as f32 + 0.5, y as f32 + 0.5];
            let weights = [edge(b, c, point), edge(c, a, point), edge(a, b, point)]
                .map(|weight| weight * inverse_area);
            // Pixels wholly inside or outside skip the individual edge tests.
            let coverage = if inside(weights, min_delta) {
                ((1_u16 << N) - 1) as u8
            } else if inside(weights, max_delta) {
                sample_deltas
                    .iter()
                    .enumerate()
                    .fold(0, |bits, (index, delta)| {
                        bits | (u8::from(inside(weights, *delta)) << index)
                    })
            } else {
                0
            };
            if coverage != 0 {
                draw(y * width + x, weights, coverage);
            }
        }
    }
}

fn sample_mask(mask: &[u8], point: Point) -> f32 {
    let grid_coord =
        |axis: usize| ((point[axis] - FLOOR_MIN) / GRID_STEP - 0.5).clamp(0.0, (GRID - 1) as f32);
    let (x, y) = (grid_coord(0), grid_coord(2));
    let (ix, iy) = (x as usize, y as usize);
    let (right, bottom) = ((ix + 1).min(GRID - 1), (iy + 1).min(GRID - 1));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let at = |x, y| f32::from(mask[y * GRID + x]);
    lerp(
        lerp(at(ix, iy), at(right, iy), x.fract()),
        lerp(at(ix, bottom), at(right, bottom), x.fract()),
        y.fract(),
    )
}

fn line(value: f32) -> f32 {
    let distance = (value - value.round()).abs();
    (1.0 - distance * 40.0).max(0.0)
}

fn channel(value: f32) -> u8 {
    value.clamp(0.0, 255.0).round() as u8
}

fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn mul(point: Point, factor: f32) -> Point {
    point.map(|value| value * factor)
}

fn dot(a: Point, b: Point) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn normalize(point: Point) -> Point {
    mul(point, dot(point, point).sqrt().recip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagonal_edges_resolve_partial_pixel_coverage() {
        let mut pixel = Pixel {
            depth: [f32::NEG_INFINITY; PIXEL_SAMPLES.len()],
            color: [[0; 3]; PIXEL_SAMPLES.len()],
        };
        rasterize(
            [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            1,
            1,
            PIXEL_SAMPLES,
            |_, _, coverage| {
                assert_eq!(coverage.count_ones(), 2);
                for (sample, color) in pixel.color.iter_mut().enumerate() {
                    if coverage & (1 << sample) != 0 {
                        *color = [255; 3];
                    }
                }
            },
        );
        assert_eq!(pixel.resolve(), [128, 128, 128, 255]);
    }

    #[test]
    fn raster_fallback_moves_the_shadow_and_renders_the_scene() {
        let geometry = occluders();
        let left = raster_shadow_mask(&geometry, 0);
        let right = raster_shadow_mask(&geometry, 16);
        assert!(left.contains(&0) && left.contains(&(LIGHT_SAMPLES as u8)));
        assert!(
            left.iter()
                .any(|&value| value > 0 && value < LIGHT_SAMPLES as u8)
        );
        assert_ne!(left, right);

        let image = render(&geometry, &left, 0).unwrap();
        let unshadowed = render(&geometry, &vec![0; GRID * GRID], 0).unwrap();
        assert_eq!((image.image.width(), image.image.height()), (WIDTH, HEIGHT));
        assert_ne!(image.image.pixels(), unshadowed.image.pixels());
    }
}
