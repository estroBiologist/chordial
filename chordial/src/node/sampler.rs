use std::sync::Mutex;

use crate::{engine::{Config, Engine}, midi::PolyVoiceTracker, resource::{AudioData, ResourceHandle, ResourceHandleDyn}, util};

use super::{BufferAccess, BusKind, Node, NodeUtil, NodeInstance, TlUnit};


pub struct SampleNode {
	sample: ResourceHandle<AudioData>,
	playback_pos: usize,
	position: TlUnit,
	start_offset: TlUnit,
	end_offset: TlUnit,
}


impl Node for SampleNode {
	fn get_name(&self) -> &'static str {
		"Sample Node"
	}

	fn get_inputs(&self) -> &[BusKind] {
		&[]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn render(
		&self,
		_output: usize,
		mut buffer: BufferAccess,
		_instance: &NodeInstance,
		engine: &Engine
	) {
		let Some(sample) = &*self.sample.inner() else {
			return
		};

		let audio = buffer.audio_mut().unwrap();

		let pos = engine.config.tl_units_to_frames(self.position);
		let start_offset = engine.config.tl_units_to_frames(self.start_offset);

		let sample_lock = sample.read().unwrap();
		let mut length = sample_lock.data.data.len();

		length -= start_offset;
		length -= engine.config.tl_units_to_frames(self.end_offset);

		audio
			.iter_mut()
			.enumerate()
			.for_each(|(i, f)| {
				let frame_pos = self.playback_pos + i;

				if frame_pos >= pos && frame_pos < length {
					let relative = frame_pos - pos + start_offset;
					
					*f += sample_lock.data.data[relative];
				}
			});
		
	}

	fn advance(&mut self, frames: usize, _config: &Config) {
		self.playback_pos += frames;
	}

	fn seek(&mut self, position: usize, _config: &Config) {
		self.playback_pos = position;
	}
	
	fn is_timeline_node(&self) -> bool {
		true
	}

	fn get_timeline_length(&self, config: &Config) -> TlUnit {
		let Some(inner) = &*self.sample.inner() else {
			return TlUnit(0)
		};

		let len_frames = inner.read().unwrap().data.data.len();
		let len_units = config.frames_to_tl_units(len_frames);

		len_units
	}
}


pub struct Sampler {
	voices: Mutex<Option<PolyVoiceTracker>>,
	sample: ResourceHandle<AudioData>,
}

impl Sampler {
	pub fn new() -> Self {
		Sampler {
			voices: Mutex::new(Some(PolyVoiceTracker::new())),
			sample: ResourceHandle::nil("AudioData")
		}
	}
}

impl Node for Sampler {
	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Midi]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_name(&self) -> &'static str {
		"Sampler"
	}

	fn get_resource(&self, name: &str) -> &dyn ResourceHandleDyn {
		match name {
			"sample" => &self.sample,
			
			_ => panic!()
		}
	}

	fn render(
		&self,
		_output: usize,
		mut buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		let Some(sample) = &*self.sample.inner() else {
			return
		};

		let Some(midi) = self.poll_input(0, buffer.len(), instance, engine) else {
			return
		};

		let Some(mut tracker) = self.voices.lock().unwrap().take() else {
			return
		};

		let sample = sample.read().unwrap();
		let sample = &sample.data;
		let midi = midi.midi().unwrap();
		let audio = buffer.audio_mut().unwrap();
		
		audio
			.iter_mut()
			.zip(midi)
			.enumerate()
			.for_each(|(i, (f, chain))| {
				tracker.apply_midi_chain(chain, i as u32);

				for note in tracker.voices.values_mut() {
					let vel = note.velocity as f32 / 127.0;

					let pitch_scale = util::note_offset_to_pitch_scale(
						note.note as i32 - 72
					);

					*f += util::resample(
						&sample.data,
						sample.sample_rate as f32,
						engine.config.sample_rate as f32 / pitch_scale as f32,
						note.progress as usize,
						util::ResampleMethod::Linear
					) * vel;

					note.progress += 1;
				}
			});
		
		*self.voices.lock().unwrap() = Some(tracker);
	}
}