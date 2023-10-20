use crate::util;


#[derive(Debug, Copy, Clone)]
pub struct Envelope {
	pub attack: f32,
	pub decay: f32,
	pub sustain: f32,
	pub release: f32,
}

impl Envelope {
	pub fn get_gain_released(&self, start: f32, release: f32, current: f32) -> f32 {
		let gain = self.get_gain(start, release);
		let time = current - release;
		
		if time > self.release {
			return 0.0
		}
		
		gain * util::inverse_lerp(self.release, 0.0, time)
	}

	pub fn get_gain(&self, start: f32, current: f32) -> f32 {
		let mut time = current - start;
		
		if time < self.attack {
			return util::inverse_lerp(0.0, self.attack, time)
		}

		time -= self.attack;

		if time < self.decay {
			let t = util::inverse_lerp(0.0, self.decay, time);
			return util::lerp(0.0, self.sustain, t)
		}

		return self.sustain
	}
}