use std::collections::HashMap;

use smallvec::SmallVec;

use crate::{node::TlUnit, param::ParamValue, resource::Resource};

pub type MidiMessageChain = SmallVec<[MidiMessage; 4]>;

const MIDI_CODE_MASK   : u8 = 0xF0;
const MIDI_CHANNEL_MASK: u8 = 0x0F;

#[derive(Debug, Copy, Clone, Default)]
pub struct MidiMessage {
	data: [u8; 3],
}

impl MidiMessage {
	pub fn new(status: MidiStatusByte, data: [u8; 2]) -> Self {
		MidiMessage {
			data: [status.0, data[0], data[1]]
		}
	}

	pub fn status_byte(&self) -> MidiStatusByte {
		MidiStatusByte(self.data[0])
	}

	pub fn data(&self) -> &[u8; 3] {
		&self.data
	}
}

pub struct MidiStatusByte(pub u8);

#[repr(u8)]
pub enum MidiStatusCode {
	NoteOff         = 0b1000_0000,
	NoteOn          = 0b1001_0000,
	PolyKeyPressure = 0b1010_0000,
	CtrlChange      = 0b1011_0000,
	ChannelPressure = 0b1101_0000,
	PitchBendChange = 0b1110_0000,
}

impl MidiStatusCode {
	pub fn from_u8(byte: u8) -> Self {
		match byte {
			0b1000_0000 => MidiStatusCode::NoteOff,
			0b1001_0000 => MidiStatusCode::NoteOn,
			0b1010_0000 => MidiStatusCode::PolyKeyPressure,
			0b1011_0000 => MidiStatusCode::CtrlChange,
			0b1101_0000 => MidiStatusCode::ChannelPressure,
			0b1110_0000 => MidiStatusCode::PitchBendChange,
			_ => panic!("unrecognized midi status code: {:#b}", byte)
		}
	}
}

impl MidiStatusByte {
	pub fn new(code: MidiStatusCode, channel: u8) -> Self {
		assert!(channel == (channel & MIDI_CHANNEL_MASK));

		Self(code as u8 | channel)
	}

	pub fn from_u8(byte: u8) -> Self {
		Self(byte)
	}

	pub fn code(&self) -> MidiStatusCode {
		MidiStatusCode::from_u8(self.0 & MIDI_CODE_MASK)
	}

	pub fn channel(&self) -> u8 {
		self.0 & MIDI_CHANNEL_MASK
	}
}


#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct MidiVoiceDesc {
	pub note: u8,
	pub velocity: u8,
	pub progress: u32,
	pub release_point: u32,
	pub released: bool,
}

pub struct MonoVoiceTracker {
	pub channels: Box<[Option<MidiVoiceDesc>; 16]>,
	pub release_length: u32,
	pub zero_crossing: bool,
}

pub struct PolyVoiceTracker {
	pub channel_voices: Box<[HashMap<u8, MidiVoiceDesc>; 16]>,
    pub polyphony: u8,
	pub release_length: u32,
	pub zero_crossing: bool,
}

impl MonoVoiceTracker {
	pub fn new() -> Self {
		MonoVoiceTracker {
			channels: Box::new([None; 16]),
			release_length: 0,
			zero_crossing: true,
		}
	}

    pub fn apply_midi_chain(&mut self, chain: &MidiMessageChain, buffer_progress: u32) {
        for msg in chain.iter() {
			self.apply_midi_message(*msg, buffer_progress)
		}
    }

    pub fn apply_midi_message(&mut self, msg: MidiMessage, buffer_progress: u32) {
        let status = msg.status_byte();
		let channel = status.channel();

		match status.code() {
			MidiStatusCode::NoteOn => {
				let desc = MidiVoiceDesc {
					note: msg.data[1],
					velocity: msg.data[2],
					progress: 0,
					released: false,
					release_point: 0,
				};

				if desc.velocity != 0 {
					self.channels[channel as usize] = Some(desc);
				} else {
					self.release_voice(channel, msg.data[1], buffer_progress);
				}
			}

			MidiStatusCode::NoteOff => {
				self.release_voice(channel, msg.data[1], buffer_progress);
			}

			_ => { }
		}
    }

	pub fn release_voice(&mut self, channel: u8, note: u8, buffer_progress: u32) {
		let Some(active) = &mut self.channels[channel as usize] else {
			return
		};

		if active.note != note {
			return
		}

		if self.release_length == 0 {
			self.channels[channel as usize] = None;
		} else {
			active.released = true;
			active.release_point = active.progress + buffer_progress;
		}
	}

	pub fn advance(&mut self, samples: u32) {
		for channel in self.channels.iter_mut() {
			let Some(note) = channel else {
				continue
			};

			if note.released && note.progress - note.release_point >= self.release_length {
				*channel = None;
			} else {
				note.progress += samples;
			}
		}
	}
}

impl PolyVoiceTracker {
	pub fn new() -> Self {
		PolyVoiceTracker {
			channel_voices: Box::default(),
			polyphony: 0,
			release_length: 0,
			zero_crossing: true,
		}
	}

    pub fn apply_midi_chain(&mut self, chain: &MidiMessageChain, buffer_progress: u32) {
        for msg in chain.iter() {
			self.apply_midi_message(*msg, buffer_progress)
		}
    }

    pub fn apply_midi_message(&mut self, msg: MidiMessage, buffer_progress: u32) {
        let status = msg.status_byte();
		let channel = status.channel();
		let polyphony = self.polyphony as usize;

		match status.code() {
			MidiStatusCode::NoteOn => {
				let voices = &mut self.channel_voices[channel as usize];
				let desc = MidiVoiceDesc {
					note: msg.data[1],
					velocity: msg.data[2],
					progress: 0,
					released: false,
					release_point: 0,
				};

				if desc.velocity != 0 {
					if voices.len() < polyphony || polyphony == 0 {
						voices.insert(desc.note, desc);
					}
				} else {
					self.release_voice(channel, msg.data[1], buffer_progress);
				}
			}

			MidiStatusCode::NoteOff => {
				self.release_voice(channel, msg.data[1], buffer_progress)
			}

			_ => {
				
			}
		}
    }

	pub fn advance(&mut self, samples: u32) {
		for channel in self.channel_voices.iter_mut() {
			channel.retain(|_, v| !v.released || v.progress - v.release_point < self.release_length);
			
			for note in channel.values_mut() {
				note.progress += samples;
			}

		}
	}

	pub fn release_voice(&mut self, channel: u8, note: u8, buffer_progress: u32) {
		let voices = &mut self.channel_voices[channel as usize];

		if self.release_length == 0 {
			voices.remove(&note);
		} else {
			let Some(voice) = voices.get_mut(&note) else {
				return
			};

			voice.release_point = voice.progress + buffer_progress;
			voice.released = true;
		}
	}

	pub fn kill_all_voices(&mut self) {
		for channel in self.channel_voices.iter_mut() {
			channel.clear();
		}
	}
}

#[derive(Copy, Clone)]
pub struct MidiNoteDesc {
	pub pos: TlUnit,
	pub len: TlUnit,
	pub note: u8,
	pub vel: u8
}

#[derive(Clone, Default)]
pub struct MidiBlock {
	pub channels: [Vec<MidiNoteDesc>; 16],
}

impl Resource for MidiBlock {
	fn resource_kind_id(&self) -> &'static str {
		"MidiBlock"
	}

	fn apply_action(&mut self, action: &str, args: &[ParamValue]) {
		let [ParamValue::Int(channel), args @ ..] = args else {
			return
		};

		let channel = *channel as usize;

		match action {

			"add_note" => {
				let [
					ParamValue::Int(note),
					ParamValue::Int(len),
					ParamValue::Int(pos),
					ParamValue::Int(vel)
				] = args else {
					panic!()
				};

				self.channels[channel].push(MidiNoteDesc {
					pos: TlUnit(*pos as usize),
					len: TlUnit(*len as usize),
					note: *note as u8,
					vel: *vel as u8,
				});
			}
			
			"update_note" => {
				let [
					ParamValue::Int(idx),
					ParamValue::Int(value),
					ParamValue::Int(len),
					ParamValue::Int(pos),
					ParamValue::Int(vel)
				] = args else {
					panic!()
				};
				
				let note = &mut self.channels[channel][*idx as usize];

				note.pos = TlUnit(*pos as usize);
				note.len = TlUnit(*len as usize);
				note.note = *value as u8;
				note.vel = *vel as u8;
			}
			
			"remove_note" => {
				let [ParamValue::Int(idx)] = args else {
					panic!()
				};

				self.channels[channel].remove(*idx as usize);
			}

			_ => panic!()
		}
	}

	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> {
		let [ParamValue::String(request), args @ ..] = keys else {
			return None
		};

		match request.as_str() {

			"get_note_pos" | "get_note_len" | "get_note_value" | "get_note_vel" => {
				let [ParamValue::Int(channel), ParamValue::Int(note)] = args else {
					return None
				};

				let note = self.channels[*channel as usize].get(*note as usize)?;

				match request.as_str() {
					"get_note_pos" => Some(ParamValue::Int(note.pos.0 as i64)),
					"get_note_len" => Some(ParamValue::Int(note.len.0 as i64)),
					"get_note_value" => Some(ParamValue::Int(note.note as i64)),
					"get_note_vel" => Some(ParamValue::Int(note.vel as i64)),

					_ => unreachable!()
				}
			}

			"get_channel_note_count" => {
				let [ParamValue::Int(channel)] = args else {
					return None
				};

				Some(ParamValue::Int(self.channels[*channel as usize].len() as i64))
			}

			_ => None
		}
	}
}