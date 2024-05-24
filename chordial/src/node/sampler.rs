use std::sync::Mutex;

use rubato::FftFixedOut;

use crate::{engine::{Config, Engine}, midi::PolyVoiceTracker, resource::{AudioData, ResourceHandle}};

use super::{BufferAccess, BusKind, Node, NodeInstance, TlUnit};


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

	fn set_position(&mut self, pos: TlUnit) {
		self.position = pos
	}

	fn set_start_offset(&mut self, offset: TlUnit) {
		self.start_offset = offset
	}

	fn set_end_offset(&mut self, offset: TlUnit) {
		self.end_offset = offset
	}

	fn get_length(&self, config: &Config) -> TlUnit {
		let Some(inner) = &*self.sample.inner() else {
			return TlUnit(0)
		};

		let len_frames = inner.read().unwrap().data.data.len();
		let len_units = config.frames_to_tl_units(len_frames);

		len_units
	}
}



pub struct Sampler {
	voices: PolyVoiceTracker,
	sample: ResourceHandle<AudioData>,
	resampler: Mutex<FftFixedOut<f32>>,
	buffer: Mutex<Vec<f32>>,
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

	fn render(
		&self,
		_output: usize,
		buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		let sample_buffer = self.buffer.lock().unwrap();
		let resampler = self.resampler.lock().unwrap();

		//resampler.process_into_buffer(wave_in, wave_out, active_channels_mask)
	}

	fn advance(&mut self, frames: usize, config: &Config) {
		
	}
	
}