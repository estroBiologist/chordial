use crate::{engine::{Config, Engine}, midi::{MidiBlock, MidiMessage, MidiStatusByte, MidiStatusCode}, resource::{ResourceHandleDyn, ResourceHandle}};

use super::{BufferAccess, BusKind, Node, NodeInstance, TlUnit};


pub struct MidiClipNote {
	pub pos: TlUnit,
	pub len: TlUnit,
	pub note: u8,
	pub vel: u8
}

pub struct MidiClip {
	pub data: ResourceHandle<MidiBlock>,
	pub playback_pos: usize,
}

impl MidiClip {
	pub fn new(data: ResourceHandle<MidiBlock>) -> Self {
		MidiClip {
			data,
			playback_pos: 0,
		}
	}
}

impl Node for MidiClip {
	fn get_name(&self) -> &'static str {
		"MIDI Clip"
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Midi]
	}

	fn render(
		&self,
		_output: usize,
		mut buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		
		let buffer = buffer.midi_mut().unwrap();

		let Some(data) = &*self.data.inner() else {
			return
		};

		let data = data.read().unwrap();

		buffer
			.iter_mut()
			.enumerate()
			.for_each(|(i, m)| {
				let sample_pos = self.playback_pos + i;
				let tl_pos = engine.config.frames_to_tl_units(sample_pos);
				let prev_tl_pos = if sample_pos > 0 {
					engine.config.frames_to_tl_units(sample_pos - 1)
				} else {
					TlUnit(0)
				};
				
				for channel in 0..data.data.channels.len() {
					for note in &data.data.channels[channel] {
						let note_pos = note.pos + instance.get_timeline_position();
						let note_end = note_pos + note.len;

						if tl_pos >= note_pos && (sample_pos == 0 || prev_tl_pos < note_pos) {
							// emit note start
							m.push(MidiMessage::new(
								MidiStatusByte::new(MidiStatusCode::NoteOn, channel as u8),
								[note.note, note.vel]
							));
						} else if tl_pos >= note_end && prev_tl_pos < note_end && note.len.0 > 0 {
							// emit note end
							m.push(MidiMessage::new(
								MidiStatusByte::new(MidiStatusCode::NoteOff, channel as u8),
								[note.note, note.vel]
							));
						}
					}
				}
			});
	}

	fn advance(
		&mut self,
		frames: usize,
		_config: &Config
	) {
		self.playback_pos += frames;
	}

	fn seek(
		&mut self,
		position: usize,
		_config: &Config,
	) {
		self.playback_pos = position;
	}

	fn is_timeline_node(&self) -> bool {
		true
	}

	fn get_timeline_length(&self, _config: &Config) -> TlUnit {
		let Some(data) = &*self.data.inner() else {
			return TlUnit(1)
		};
		
		let channels = &data.read().unwrap().data.channels;
		
		let mut max = 0;

		for channel in channels {
			if let Some(channel_max) = channel.iter().max_by_key(|note| note.pos.0 + note.len.0) {
				max = max.max(channel_max.pos.0 + channel_max.len.0);
			}
		}

		TlUnit(max)
	}

	fn get_resource_names(&self) -> &'static [&'static str] {
		&[
			"data",
		]
	}

	fn get_resource(&self, resource: &str) -> &dyn ResourceHandleDyn {
		match resource {
			"data" => &self.data,

			_ => panic!()
		}
	}
}