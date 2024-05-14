pub fn db_to_factor(db: f32) -> f32 {
    10.0f32.powf(db / 10.0)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    (1.0 - t) * a + b * t
}

pub fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    (v - a) / (b - a)
}

pub fn midi_to_freq_with_tuning(note: u8, a4: f64) -> f64 {
    2.0f64.powf((note as f64 - 69.0) / 12.0) * a4
}

pub fn midi_to_freq(note: u8) -> f64 {
    midi_to_freq_with_tuning(note, 440.0)
}