pub fn db_to_factor(db: f32) -> f32 {
    10.0f32.powf(db/10.0)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (1.0 - t) * a + b * t
}

pub fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    (v - a) / (b - a)
}