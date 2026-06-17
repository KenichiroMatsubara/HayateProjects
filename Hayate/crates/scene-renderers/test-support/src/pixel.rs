/// Canvas dimensions shared by every CSS pixel fixture.
pub const CANVAS_W: u32 = 100;
pub const CANVAS_H: u32 = 100;
pub const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

pub fn pixel(data: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * width + x) * 4) as usize;
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

pub fn assert_channel_min(px: [u8; 4], ch: usize, min: u8, label: &str) {
    assert!(
        px[ch] >= min,
        "{label}: channel {ch} expected >={min}, got {px:?}"
    );
}

pub fn assert_channel_max(px: [u8; 4], ch: usize, max: u8, label: &str) {
    assert!(
        px[ch] <= max,
        "{label}: channel {ch} expected <={max}, got {px:?}"
    );
}

pub fn assert_near(px: [u8; 4], expected: [u8; 4], tol: u8, label: &str) {
    for (i, (&got, &want)) in px.iter().zip(expected.iter()).enumerate() {
        let diff = got.abs_diff(want);
        assert!(diff <= tol, "{label}: ch{i} got {got} want {want}±{tol} (full {px:?})");
    }
}

pub fn assert_clear(px: [u8; 4], label: &str) {
    assert!(
        px[0] > 240 && px[1] > 240 && px[2] > 240,
        "{label}: expected clear/white background, got {px:?}"
    );
}

pub fn assert_not_clear(px: [u8; 4], label: &str) {
    assert!(
        px[0] < 230 || px[1] < 230 || px[2] < 230,
        "{label}: expected painted pixel, got clear {px:?}"
    );
}

/// Horizontal ink extent `(min_x, max_x)` for dark pixels on the canvas.
pub fn ink_extent_x(data: &[u8], width: u32, height: u32) -> Option<(u32, u32)> {
    let mut min_x = width;
    let mut max_x = 0;
    let mut found = false;
    for y in 0..height {
        for x in 0..width {
            let px = pixel(data, width, x, y);
            if px[0] < 240 || px[1] < 240 || px[2] < 240 {
                found = true;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
            }
        }
    }
    found.then_some((min_x, max_x))
}

/// How many distinct dominant-channel hues (red-, green-, blue-dominant) appear
/// among strongly-saturated painted pixels. A multi-colour (COLR) glyph spans
/// several; a monochrome glyph collapses to one. `sat_min` is the minimum
/// max-minus-min channel spread for a pixel to count as saturated, so faint
/// anti-aliased edges don't register as hues.
pub fn distinct_saturated_hues(data: &[u8], width: u32, height: u32, sat_min: u8) -> usize {
    let mut seen = [false; 3];
    for y in 0..height {
        for x in 0..width {
            let px = pixel(data, width, x, y);
            let max = px[0].max(px[1]).max(px[2]);
            let min = px[0].min(px[1]).min(px[2]);
            if max.saturating_sub(min) < sat_min {
                continue;
            }
            let dominant = if px[0] == max {
                0
            } else if px[1] == max {
                1
            } else {
                2
            };
            seen[dominant] = true;
        }
    }
    seen.iter().filter(|&&s| s).count()
}
