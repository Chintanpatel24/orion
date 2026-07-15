use eframe::egui;

const SOURCE_ICON_SVG: &str = include_str!("../assets/dev.orion.Orion.svg");
const SIZE: usize = 64;

pub fn app_icon() -> egui::IconData {
    let _keep_asset_linked = SOURCE_ICON_SVG.len();
    let mut rgba = vec![0u8; SIZE * SIZE * 4];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let alpha = rounded_rect_alpha(fx, fy, 0.0, 0.0, SIZE as f32, SIZE as f32, 14.0);
            if alpha <= 0.0 {
                continue;
            }

            let t = ((fx + fy) / (SIZE as f32 * 2.0)).clamp(0.0, 1.0);
            let bg = mix([30.0, 41.0, 59.0], [15.0, 23.0, 42.0], t);
            put(&mut rgba, x, y, [bg[0], bg[1], bg[2], 255.0 * alpha]);
        }
    }

    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;

            let a = rotated_ellipse_stroke_alpha(fx, fy, 32.0, 32.0, 20.0, 8.5, -30.0_f32.to_radians(), 2.5);
            if a > 0.0 {
                let t = ((fx - 10.0) / 44.0).clamp(0.0, 1.0);
                let color = if t < 0.5 {
                    mix([226.0, 232.0, 240.0], [148.0, 163.0, 184.0], t / 0.5)
                } else {
                    mix([148.0, 163.0, 184.0], [100.0, 116.0, 139.0], (t - 0.5) / 0.5)
                };
                blend(&mut rgba, x, y, [color[0], color[1], color[2], 255.0 * a]);
            }

            let b = rotated_ellipse_stroke_alpha(fx, fy, 32.0, 32.0, 20.0, 8.5, 35.0_f32.to_radians(), 2.5);
            if b > 0.0 {
                let t = ((54.0 - fx) / 44.0).clamp(0.0, 1.0);
                let color = if t < 0.5 {
                    mix([203.0, 213.0, 225.0], [124.0, 138.0, 160.0], t / 0.5)
                } else {
                    mix([124.0, 138.0, 160.0], [71.0, 85.0, 105.0], (t - 0.5) / 0.5)
                };
                blend(&mut rgba, x, y, [color[0], color[1], color[2], 255.0 * b]);
            }
        }
    }

    egui::IconData { rgba, width: SIZE as u32, height: SIZE as u32 }
}

#[allow(clippy::too_many_arguments)]
fn rotated_ellipse_stroke_alpha(
    x: f32,
    y: f32,
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    angle: f32,
    half_width: f32,
) -> f32 {
    let dx = x - cx;
    let dy = y - cy;
    let cos = angle.cos();
    let sin = angle.sin();
    let px = dx * cos + dy * sin;
    let py = -dx * sin + dy * cos;
    let q = ((px / rx).powi(2) + (py / ry).powi(2)).sqrt();
    let scale = (rx + ry) * 0.5;
    let dist = (q - 1.0).abs() * scale;
    (half_width + 0.55 - dist).clamp(0.0, 1.0)
}

fn rounded_rect_alpha(x: f32, y: f32, left: f32, top: f32, right: f32, bottom: f32, radius: f32) -> f32 {
    let px = if x < left + radius {
        left + radius - x
    } else if x > right - radius {
        x - (right - radius)
    } else {
        0.0
    };
    let py = if y < top + radius {
        top + radius - y
    } else if y > bottom - radius {
        y - (bottom - radius)
    } else {
        0.0
    };
    let dist = (px * px + py * py).sqrt();
    (radius + 0.5 - dist).clamp(0.0, 1.0)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

fn put(rgba: &mut [u8], x: usize, y: usize, color: [f32; 4]) {
    let idx = (y * SIZE + x) * 4;
    rgba[idx] = color[0].round().clamp(0.0, 255.0) as u8;
    rgba[idx + 1] = color[1].round().clamp(0.0, 255.0) as u8;
    rgba[idx + 2] = color[2].round().clamp(0.0, 255.0) as u8;
    rgba[idx + 3] = color[3].round().clamp(0.0, 255.0) as u8;
}

fn blend(rgba: &mut [u8], x: usize, y: usize, color: [f32; 4]) {
    let idx = (y * SIZE + x) * 4;
    let src_a = (color[3] / 255.0).clamp(0.0, 1.0);
    let dst_a = rgba[idx + 3] as f32 / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= 0.0 {
        return;
    }

    for channel in 0..3 {
        let src = color[channel] / 255.0;
        let dst = rgba[idx + channel] as f32 / 255.0;
        let out = (src * src_a + dst * dst_a * (1.0 - src_a)) / out_a;
        rgba[idx + channel] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    rgba[idx + 3] = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
}
