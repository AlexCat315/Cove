//! Box-drawing (U+2500-257F), block elements (U+2580-259F) and the Powerline
//! separators (U+E0B0-E0B3) drawn as geometry instead of font glyphs, so lines
//! meet exactly at cell edges whatever font (or fallback) is in use.

use cove_core::{Color, DrawList, Point, Rect};

pub fn handles(scalar: u32) -> bool {
    (0x2500..=0x259F).contains(&scalar) || (0xE0B0..=0xE0B3).contains(&scalar)
}

/// Line weight per arm, in (up, right, down, left) order:
/// 0 none, 1 light, 2 heavy, 3 double.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Arms {
    pub up: i32,
    pub right: i32,
    pub down: i32,
    pub left: i32,
}

/// Four digits per character from U+2500 (`-` marks shapes drawn elsewhere).
const TABLE: [&str; 128] = [
    "0101", "0202", "1010", "2020", "-", "-", "-", "-", "-", "-", "-", "-", // 2500-250B
    "0110", "0210", "0120", "0220", "0011", "0012", "0021", "0022", // 250C-2513
    "1100", "1200", "2100", "2200", "1001", "1002", "2001", "2002", // 2514-251B
    "1110", "1210", "2110", "1120", "2120", "2210", "1220", "2220", // 251C-2523
    "1011", "1012", "2011", "1021", "2021", "2012", "1022", "2022", // 2524-252B
    "0111", "0112", "0211", "0212", "0121", "0122", "0221", "0222", // 252C-2533
    "1101", "1102", "1201", "1202", "2101", "2102", "2201", "2202", // 2534-253B
    "1111", "1112", "1211", "1212", "2111", "1121", "2121", "2112", // 253C-2543
    "2211", "1122", "1221", "2212", "1222", "2122", "2221", "2222", // 2544-254B
    "-", "-", "-", "-", // 254C-254F
    "0303", "3030", "0310", "0130", "0330", "0013", "0031", "0033", // 2550-2557
    "1300", "3100", "3300", "1003", "3001", "3003", "1310", "3130", // 2558-255F
    "3330", "1013", "3031", "3033", "0313", "0131", "0333", "1303", // 2560-2567
    "3101", "3303", "1313", "3131", "3333", // 2568-256C
    "-", "-", "-", "-", "-", "-", "-", // 256D-2573
    "0001", "1000", "0100", "0010", "0002", "2000", "0200", "0020", // 2574-257B
    "0201", "1020", "0102", "2010", // 257C-257F
];

pub fn arms(scalar: u32) -> Option<Arms> {
    if !(0x2500..=0x257F).contains(&scalar) {
        return None;
    }
    let entry = TABLE[(scalar - 0x2500) as usize].as_bytes();
    if entry.len() != 4 {
        return None;
    }
    let digit = |index: usize| (entry[index] as char).to_digit(10).unwrap_or(0) as i32;
    Some(Arms { up: digit(0), right: digit(1), down: digit(2), left: digit(3) })
}

pub fn draw(scalar: u32, cell: Rect, color: Color, list: &mut DrawList) {
    let mut painter = Painter::new(cell, color, list);
    match scalar {
        0x2580..=0x259F => painter.block(scalar),
        0xE0B0..=0xE0B3 => painter.powerline(scalar),
        0x2504..=0x250B | 0x254C..=0x254F => painter.dashes(scalar),
        0x256D..=0x2570 => painter.arc(scalar),
        0x2571..=0x2573 => painter.diagonal(scalar),
        _ => {
            if let Some(arms) = arms(scalar) {
                painter.lines(arms);
            }
        }
    }
}

struct Painter<'a> {
    cell: Rect,
    color: Color,
    list: &'a mut DrawList,
    light: f32,
    heavy: f32,
    /// Half the distance between the two strokes of a double line.
    gap: f32,
    center_x: f32,
    center_y: f32,
}

impl<'a> Painter<'a> {
    fn new(cell: Rect, color: Color, list: &'a mut DrawList) -> Self {
        let light = (cell.height / 16.0).round().max(1.0);
        Self {
            cell,
            color,
            list,
            light,
            heavy: light * 2.0,
            gap: light,
            center_x: (cell.x + cell.width / 2.0).floor(),
            center_y: (cell.y + cell.height / 2.0).floor(),
        }
    }

    fn weight(&self, arm: i32) -> f32 {
        if arm == 2 { self.heavy } else { self.light }
    }

    fn rect(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let _ = self.list.add_rect(Rect::new(x0, y0, x1 - x0, y1 - y0), self.color);
    }

    fn lines(&mut self, arms: Arms) {
        let (top, bottom) = (self.cell.y, self.cell.y + self.cell.height);
        let (left, right) = (self.cell.x, self.cell.x + self.cell.width);
        // Single strokes reach past the centre by half the widest crossing
        // stroke, so corners and tees close without notches.
        let horizontal_reach = if arms.left == 3 || arms.right == 3 {
            self.gap + self.light / 2.0
        } else if arms.left == 2 || arms.right == 2 {
            self.heavy / 2.0
        } else {
            self.light / 2.0
        };
        let vertical_reach = if arms.up == 3 || arms.down == 3 {
            self.gap + self.light / 2.0
        } else if arms.up == 2 || arms.down == 2 {
            self.heavy / 2.0
        } else {
            self.light / 2.0
        };
        // A double stroke ends on the near (inner) side when a line turns towards
        // it and runs to the far (outer) side otherwise; that is what makes the
        // double-line corners and tees read as two parallel lines.
        let join = |turns: bool, near: f32, far: f32| if turns { near } else { far };

        if arms.up == 3 {
            let end = join(arms.left != 0, self.center_y - self.gap, self.center_y + self.gap);
            let x0 = self.center_x - self.gap;
            let x1 = self.center_x + self.gap;
            let (y0, y1) = (top, end);
            self.vertical(x0, y0, y1);
            let end = join(arms.right != 0, self.center_y - self.gap, self.center_y + self.gap);
            self.vertical(x1, top, end);
        } else if arms.up > 0 {
            let half = self.weight(arms.up) / 2.0;
            self.rect(self.center_x - half, top, self.center_x + half, self.center_y + horizontal_reach);
        }
        if arms.down == 3 {
            let start = join(arms.left != 0, self.center_y + self.gap, self.center_y - self.gap);
            self.vertical(self.center_x - self.gap, start, bottom);
            let start = join(arms.right != 0, self.center_y + self.gap, self.center_y - self.gap);
            self.vertical(self.center_x + self.gap, start, bottom);
        } else if arms.down > 0 {
            let half = self.weight(arms.down) / 2.0;
            self.rect(self.center_x - half, self.center_y - horizontal_reach, self.center_x + half, bottom);
        }
        if arms.left == 3 {
            let end = join(arms.up != 0, self.center_x - self.gap, self.center_x + self.gap);
            self.horizontal(self.center_y - self.gap, left, end);
            let end = join(arms.down != 0, self.center_x - self.gap, self.center_x + self.gap);
            self.horizontal(self.center_y + self.gap, left, end);
        } else if arms.left > 0 {
            let half = self.weight(arms.left) / 2.0;
            self.rect(left, self.center_y - half, self.center_x + vertical_reach, self.center_y + half);
        }
        if arms.right == 3 {
            let start = join(arms.up != 0, self.center_x + self.gap, self.center_x - self.gap);
            self.horizontal(self.center_y - self.gap, start, right);
            let start = join(arms.down != 0, self.center_x + self.gap, self.center_x - self.gap);
            self.horizontal(self.center_y + self.gap, start, right);
        } else if arms.right > 0 {
            let half = self.weight(arms.right) / 2.0;
            self.rect(self.center_x - vertical_reach, self.center_y - half, right, self.center_y + half);
        }
    }

    /// One stroke of a double line, centred on `x`.
    fn vertical(&mut self, x: f32, y0: f32, y1: f32) {
        let half = self.light / 2.0;
        self.rect(x - half, y0.min(y1) - half, x + half, y0.max(y1) + half);
    }

    /// One stroke of a double line, centred on `y`.
    fn horizontal(&mut self, y: f32, x0: f32, x1: f32) {
        let half = self.light / 2.0;
        self.rect(x0.min(x1) - half, y - half, x0.max(x1) + half, y + half);
    }

    fn dashes(&mut self, scalar: u32) {
        let (count, is_heavy, is_vertical): (i32, bool, bool) = match scalar {
            0x2504 => (3, false, false),
            0x2505 => (3, true, false),
            0x2506 => (3, false, true),
            0x2507 => (3, true, true),
            0x2508 => (4, false, false),
            0x2509 => (4, true, false),
            0x250A => (4, false, true),
            0x250B => (4, true, true),
            0x254C => (2, false, false),
            0x254D => (2, true, false),
            0x254E => (2, false, true),
            _ => (2, true, true),
        };
        let width = if is_heavy { self.heavy } else { self.light };
        let length = if is_vertical { self.cell.height } else { self.cell.width };
        let step = length / count as f32;
        let dash = (step * 0.6).ceil();
        for index in 0..count {
            let start = (if is_vertical { self.cell.y } else { self.cell.x })
                + index as f32 * step
                + (step - dash) / 2.0;
            if is_vertical {
                self.rect(self.center_x - width / 2.0, start, self.center_x + width / 2.0, start + dash);
            } else {
                self.rect(start, self.center_y - width / 2.0, start + dash, self.center_y + width / 2.0);
            }
        }
    }

    /// Rounded corners: a quarter circle joining the two arms.
    fn arc(&mut self, scalar: u32) {
        let radius = self.cell.width.min(self.cell.height) / 2.0;
        // Direction of the vertical (dy) and horizontal (dx) arms.
        let (dy, dx): (f32, f32) = match scalar {
            0x256D => (1.0, 1.0),  // down and right
            0x256E => (1.0, -1.0), // down and left
            0x256F => (-1.0, -1.0), // up and left
            _ => (-1.0, 1.0),      // up and right
        };
        let arc_center_x = self.center_x + dx * radius;
        let arc_center_y = self.center_y + dy * radius;
        let segments = 8;
        let mut previous = Point::new(self.center_x, self.center_y + dy * radius);
        for step in 1..=segments {
            let angle = step as f32 / segments as f32 * std::f32::consts::FRAC_PI_2;
            let point = Point::new(
                arc_center_x - dx * radius * angle.cos(),
                arc_center_y - dy * radius * angle.sin(),
            );
            let _ = self.list.add_line(previous, point, self.light, self.color);
            previous = point;
        }
        let half = self.light / 2.0;
        if dy > 0.0 {
            self.rect(self.center_x - half, self.center_y + radius, self.center_x + half, self.cell.y + self.cell.height);
        } else {
            self.rect(self.center_x - half, self.cell.y, self.center_x + half, self.center_y - radius);
        }
        if dx > 0.0 {
            self.rect(self.center_x + radius, self.center_y - half, self.cell.x + self.cell.width, self.center_y + half);
        } else {
            self.rect(self.cell.x, self.center_y - half, self.center_x - radius, self.center_y + half);
        }
    }

    fn diagonal(&mut self, scalar: u32) {
        let (left, right) = (self.cell.x, self.cell.x + self.cell.width);
        let (top, bottom) = (self.cell.y, self.cell.y + self.cell.height);
        if scalar != 0x2572 {
            let _ = self.list.add_line(Point::new(right, top), Point::new(left, bottom), self.light, self.color);
        }
        if scalar != 0x2571 {
            let _ = self.list.add_line(Point::new(left, top), Point::new(right, bottom), self.light, self.color);
        }
    }

    fn block(&mut self, scalar: u32) {
        let (x, y, w, h) = (self.cell.x, self.cell.y, self.cell.width, self.cell.height);
        let mut fill = |fx: f32, fy: f32, fw: f32, fh: f32, alpha: f32| {
            let color = if alpha >= 1.0 { self.color } else { self.color.multiplied_alpha(alpha) };
            let _ = self.list.add_rect(Rect::new(x + fx * w, y + fy * h, fw * w, fh * h), color);
        };
        match scalar {
            0x2580 => fill(0.0, 0.0, 1.0, 0.5, 1.0),
            0x2581..=0x2588 => {
                let eighths = (scalar - 0x2580) as f32 / 8.0;
                fill(0.0, 1.0 - eighths, 1.0, eighths, 1.0);
            }
            0x2589..=0x258F => {
                let eighths = (0x2590 - scalar) as f32 / 8.0;
                fill(0.0, 0.0, eighths, 1.0, 1.0);
            }
            0x2590 => fill(0.5, 0.0, 0.5, 1.0, 1.0),
            0x2591 => fill(0.0, 0.0, 1.0, 1.0, 0.25),
            0x2592 => fill(0.0, 0.0, 1.0, 1.0, 0.5),
            0x2593 => fill(0.0, 0.0, 1.0, 1.0, 0.75),
            0x2594 => fill(0.0, 0.0, 1.0, 0.125, 1.0),
            0x2595 => fill(0.875, 0.0, 0.125, 1.0, 1.0),
            _ => {
                // Quadrants as bits: upper-left, upper-right, lower-left, lower-right.
                let quadrants: (bool, bool, bool, bool) = match scalar {
                    0x2596 => (false, false, true, false),
                    0x2597 => (false, false, false, true),
                    0x2598 => (true, false, false, false),
                    0x2599 => (true, false, true, true),
                    0x259A => (true, false, false, true),
                    0x259B => (true, true, true, false),
                    0x259C => (true, true, false, true),
                    0x259D => (false, true, false, false),
                    0x259E => (false, true, true, false),
                    0x259F => (false, true, true, true),
                    _ => return,
                };
                if quadrants.0 {
                    fill(0.0, 0.0, 0.5, 0.5, 1.0);
                }
                if quadrants.1 {
                    fill(0.5, 0.0, 0.5, 0.5, 1.0);
                }
                if quadrants.2 {
                    fill(0.0, 0.5, 0.5, 0.5, 1.0);
                }
                if quadrants.3 {
                    fill(0.5, 0.5, 0.5, 0.5, 1.0);
                }
            }
        }
    }

    fn powerline(&mut self, scalar: u32) {
        let (left, right) = (self.cell.x, self.cell.x + self.cell.width);
        let (top, bottom) = (self.cell.y, self.cell.y + self.cell.height);
        let middle = self.cell.y + self.cell.height / 2.0;
        match scalar {
            0xE0B0 => {
                let _ = self.list.add_triangle(Point::new(left, top), Point::new(right, middle), Point::new(left, bottom), self.color);
            }
            0xE0B2 => {
                let _ = self.list.add_triangle(Point::new(right, top), Point::new(left, middle), Point::new(right, bottom), self.color);
            }
            0xE0B1 => {
                let _ = self.list.add_line(Point::new(left, top), Point::new(right, middle), self.light, self.color);
                let _ = self.list.add_line(Point::new(right, middle), Point::new(left, bottom), self.light, self.color);
            }
            _ => {
                let _ = self.list.add_line(Point::new(right, top), Point::new(left, middle), self.light, self.color);
                let _ = self.list.add_line(Point::new(left, middle), Point::new(right, bottom), self.light, self.color);
            }
        }
    }
}
