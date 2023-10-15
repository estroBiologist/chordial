pub fn db_to_factor(db: f32) -> f32 {
    10.0f32.powf(db/10.0)
}