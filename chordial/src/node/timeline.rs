use crate::{engine::{Config, Engine}, midi::{MidiBlock, MidiMessage, MidiStatusByte, MidiStatusCode}, resource::{ResourceAccess, ResourceBuffer}};

use super::{BufferAccess, BusKind, Node, NodeInstance, Step};


pub struct MidiClipNote {
	pub pos: Step,
	pub len: Step,
	pub note: u8,
	pub vel: u8
}

pub struct MidiClip {
	pub data: ResourceBuffer<MidiBlock>,
	pub position: Step,
	pub start_offset: Step,
	pub end_offset: Step,
	pub playback_pos: usize,
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
		_instance: &NodeInstance,
		engine: &Engine
	) {
		
		let buffer = buffer.midi_mut().unwrap();
		let data = self.data.read();

		buffer
			.iter_mut()
			.enumerate()
			.for_each(|(i, m)| {
				let sample_pos = self.playback_pos + i;
				let tl_pos = engine.config.frames_to_tl_units(sample_pos);
				let prev_tl_pos = if sample_pos > 0 {
					engine.config.frames_to_tl_units(sample_pos - 1)
				} else {
					Step(0)
				};
				
				for channel in 0..data.channels.len() {
					for note in &data.channels[channel] {
						let note_pos = note.pos + self.position;
						let note_end = note_pos + note.len;

						if tl_pos >= note_pos && (sample_pos == 0 || prev_tl_pos < note_pos) {
							// emit note start
							m.push(MidiMessage::new(
								MidiStatusByte::new(MidiStatusCode::NoteOn, channel as u8),
								[note.note, note.vel]
							));
						} else if tl_pos < note_end && prev_tl_pos >= note_end && note.len.0 > 0 {
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

	fn set_position(&mut self, pos: Step) {
		self.position = pos;
	}

	fn set_start_offset(&mut self, offset: Step) {
		self.start_offset = offset;
	}

	fn set_end_offset(&mut self, offset: Step) {
		self.end_offset = offset;
	}

	fn get_resource_names(&self) -> &'static [&'static str] {
		&[
			"data",
		]
	}

	fn get_resource(&self, resource: &str) -> &dyn ResourceAccess {
		match resource {
			"data" => &self.data,

			_ => panic!()
		}
	}
}