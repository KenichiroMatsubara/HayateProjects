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
