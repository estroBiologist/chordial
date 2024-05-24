use std::{f64::consts::TAU, sync::Mutex};

use crate::{engine::{Config, Engine}, midi::{MonoVoiceTracker, PolyVoiceTracker}, param::{ParamKind, ParamValue, Parameter}, util};

use super::{BufferAccess, BusKind, Node, NodeInstance, NodeUtil};


pub struct Osc {
	pos: usize,
	notes: Mutex<Option<MonoVoiceTracker>>,
}

impl Osc {
	pub fn new() -> Self {
		Osc {
			pos: 0,
			notes: Mutex::new(Some(MonoVoiceTracker::new()))
		}
	}
}

impl Node for Osc {
	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Midi]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_name(&self) -> &'static str {
		"Osc"
	}

	fn render(
		&self,
		_output: usize,
		mut buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		let Some(midi) = self.poll_input(0, buffer.len(), instance, engine) else {
			return
		};

		let Some(mut tracker) = self.notes.lock().unwrap().take() else {
			return
		};

		let midi = midi.midi().unwrap();
		let audio = buffer.audio_mut().unwrap();

		audio
			.iter_mut()
			.zip(midi)
			.enumerate()
			.for_each(|(i, (f, m))| {
				tracker.apply_midi_chain(m, i as u32);

				for channel in tracker.channels.iter_mut() {
					let Some(note) = channel else {
						continue
					};
					
					let time = note.progress as f64 / engine.config.sample_rate as f64;
					let rate = util::midi_to_freq(note.note);
					let vel = note.velocity as f32 / 127.0;

					f.0[0] += (TAU * time * rate).sin() as f32 * vel;
					f.0[1] += (TAU * time * rate).sin() as f32 * vel;

					note.progress += 1;
				}
			});
		
		tracker.purge_dead_voices();

		*self.notes.lock().unwrap() = Some(tracker);
	}

	fn advance(
		&mut self,
		frames: usize,
		_config: &Config
	) {
		self.pos += frames;
	}

	fn seek(
		&mut self,
		position: usize,
		_config: &Config,
	) {
		self.pos = position;
	}
}


pub struct PolyOsc {
	pos: usize,
	notes: Mutex<Option<PolyVoiceTracker>>,
}


impl PolyOsc {
	pub fn new() -> Self {
		PolyOsc {
			pos: 0,
			notes: Mutex::new(Some(PolyVoiceTracker::new()))
		}
	}
}

impl Node for PolyOsc {
	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Midi]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_name(&self) -> &'static str {
		"PolyOsc"
	}

	fn render(
		&self,
		_output: usize,
		mut buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		let Some(midi) = self.poll_input(0, buffer.len(), instance, engine) else {
			return
		};

		let Some(mut tracker) = self.notes.lock().unwrap().take() else {
			return
		};

		let midi = midi.midi().unwrap();
		let audio = buffer.audio_mut().unwrap();

		audio
			.iter_mut()
			.zip(midi)
			.enumerate()
			.for_each(|(i, (f, m))| {
				tracker.apply_midi_chain(m, i as u32);

				for channel in tracker.channel_voices.iter_mut() {
					for (_, note) in channel.iter_mut() {
						let time = note.progress as f64 / engine.config.sample_rate as f64;
						let rate = util::midi_to_freq(note.note);
						let vel = note.velocity as f32 / 127.0;

						f.0[0] += (TAU * time * rate).sin() as f32 * vel;
						f.0[1] += (TAU * time * rate).sin() as f32 * vel;
						
						note.progress += 1;
					}
				}
			});
		
		tracker.purge_dead_voices();

		*self.notes.lock().unwrap() = Some(tracker);
	}

	fn advance(
		&mut self,
		frames: usize,
		_config: &Config
	) {
		self.pos += frames;
	}

	fn seek(
		&mut self,
		position: usize,
		_config: &Config,
	) {
		self.pos = position;

		let Some(lock) = &mut *self.notes.lock().unwrap() else {
			panic!()
		};

		lock.kill_all_voices();
	}
}


pub struct Sine {
	pos: usize,
	rate: f64,
}

impl Sine {
	pub fn new(rate: f64) -> Self {
		Sine {
			pos: 0,
			rate,
		}
	}
}

impl Node for Sine {
	fn get_inputs(&self) -> &[BusKind] {
		&[]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_name(&self) -> &'static str {
		"Sine"
	}

	fn get_input_names(&self) -> &'static [&'static str] {
		&["trigger"]
	}

	fn get_output_names(&self) -> &'static [&'static str] {
		&["out"]
	}

	fn render(&self, _: usize, buffer: BufferAccess, _instance: &NodeInstance, engine: &Engine) {
		let BufferAccess::Audio(buffer) = buffer else {
			panic!()
		};
		
		buffer
			.iter_mut()
			.enumerate()
			.for_each(|(i, f)| {
				let time = (self.pos + i) as f64 / engine.config.sample_rate as f64;
				f.0[0] = (TAU * time * self.rate).sin() as f32;
				f.0[1] = (TAU * time * self.rate).sin() as f32;
			});
	}

	fn advance(&mut self, frames: usize, _config: &Config) {
		self.pos += frames;
	}

	fn seek(&mut self, position: usize, _config: &Config) {
		self.pos = position;
	}

	fn get_params(&self) -> &[Parameter] {
		&[
			Parameter {
				kind: ParamKind::Float,
				text: "freq",
			}
		]
	}
	
	fn get_param_default_value(&self, _: usize) -> Option<ParamValue> {
		Some(ParamValue::Float(440.0))
	}

	fn param_updated(&mut self, _: usize, value: &ParamValue) {
		let ParamValue::Float(val) = value else {
			panic!()
		};

		self.rate = *val;
	}
}
