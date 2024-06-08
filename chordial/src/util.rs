use crate::engine::Frame;

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

pub fn note_offset_to_pitch_scale(offset: i32) -> f64 {
	2.0f64.powf(offset as f64 / 12.0)
}

#[derive(Copy, Clone, Debug)]
pub enum ResampleMethod {
	Nearest,
	Linear,
	Hermite,
	Sinc8,
	Sinc16,
	Sinc32,
}

pub fn resample(
	input: &[Frame],
	input_rate: f32,
	output_rate: f32,
	output_offset: usize,
	method: ResampleMethod
) -> Frame {
	let ratio = output_rate / input_rate;

	match method {
		ResampleMethod::Nearest => {
			let j = output_offset as f32 / ratio;
			let j = (j as usize).clamp(0, input.len() - 1);

			input[j]
		}

		ResampleMethod::Linear => {
			let j = output_offset as f32 / ratio;
			let j = j.clamp(0.0, input.len() as f32 - 2.0);
			let j1 = j.floor() as usize;
			let j2 = j.ceil() as usize;
			let t = j - j.floor();

			Frame(
				lerp(input[j1].0, input[j2].0, t),
				lerp(input[j1].1, input[j2].1, t)
			)
		}

		_ => todo!()
	}
}