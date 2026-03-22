pub(crate) fn to_u64_saturating(value: u128) -> u64 {
    value.min(u64::MAX as u128) as u64
}

pub(crate) fn sanitize_scale_factor(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    }
}

pub(crate) fn logical_to_physical(value: i32, scale_factor: f64) -> i32 {
    ((value as f64) * scale_factor).round() as i32
}

pub(crate) fn point_in_rect(
    x: i32,
    y: i32,
    rect_x: i32,
    rect_y: i32,
    rect_width: i32,
    rect_height: i32,
) -> bool {
    if rect_width <= 0 || rect_height <= 0 {
        return false;
    }
    let max_x = rect_x.saturating_add(rect_width);
    let max_y = rect_y.saturating_add(rect_height);
    x >= rect_x && y >= rect_y && x < max_x && y < max_y
}

pub(crate) fn scale_for_logical_point_in_rects(
    logical_x: i32,
    logical_y: i32,
    candidates: &[(i32, i32, i32, i32, f64)],
) -> Option<f64> {
    for (rect_x, rect_y, rect_width, rect_height, raw_scale) in candidates {
        let scale = sanitize_scale_factor(*raw_scale);
        let px = logical_to_physical(logical_x, scale);
        let py = logical_to_physical(logical_y, scale);
        if point_in_rect(px, py, *rect_x, *rect_y, *rect_width, *rect_height) {
            return Some(scale);
        }
    }

    None
}
