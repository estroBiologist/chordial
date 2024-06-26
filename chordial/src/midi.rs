use std::{collections::HashMap, mem::size_of};
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
	pub channel: u8,
	pub velocity: u8,
	pub progress: u32,
	pub release_point: u32,
	pub released: bool,
}

pub struct MonoVoiceTracker {
	pub voice: Option<MidiVoiceDesc>,
	pub release_length: u32,
	pub zero_crossing: bool,
}

pub struct PolyVoiceTracker {
	pub voices: HashMap<(u8, u8), MidiVoiceDesc>,
    pub polyphony: u8,
	pub release_length: u32,
	pub zero_crossing: bool,
}

impl MonoVoiceTracker {
	pub fn new() -> Self {
		MonoVoiceTracker {
			voice: None,
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
					channel,
					note: msg.data[1],
					velocity: msg.data[2],
					progress: 0,
					released: false,
					release_point: 0,
				};

				if desc.velocity != 0 {
					self.voice = Some(desc);
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
		let Some(active) = &mut self.voice else {
			return
		};

		if active.note != note || active.channel != channel {
			return
		}

		if self.release_length == 0 {
			self.voice = None;
		} else {
			active.released = true;
			active.release_point = active.progress + buffer_progress;
		}
	}

	pub fn advance(&mut self, samples: u32) {
		let Some(note) = &mut self.voice else {
			return
		};

		note.progress += samples;
		self.purge_dead_voices();
	}

	pub fn purge_dead_voices(&mut self) {
		let Some(note) = &mut self.voice else {
			return
		};

		if note.released && note.progress - note.release_point >= self.release_length {
			self.voice = None;
		}
	}
}

impl PolyVoiceTracker {
	pub fn new() -> Self {
		PolyVoiceTracker {
			voices: HashMap::new(),
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

		if self.voices.capacity() < polyphony {
			self.voices.reserve(polyphony - self.voices.len());
		}

		match status.code() {
			MidiStatusCode::NoteOn => {
				let desc = MidiVoiceDesc {
					note: msg.data[1],
					channel,
					velocity: msg.data[2],
					progress: 0,
					released: false,
					release_point: 0,
				};

				if desc.velocity != 0 {
					if self.voices.len() < polyphony || polyphony == 0 {
						self.voices.insert((desc.channel, desc.note), desc);
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
		for note in self.voices.values_mut() {
			note.progress += samples;
		}

		self.purge_dead_voices()
	}

	pub fn purge_dead_voices(&mut self) {
		self.voices.retain(|_, v| !v.released || v.progress - v.release_point < self.release_length);
	}

	pub fn release_voice(&mut self, channel: u8, note: u8, buffer_progress: u32) {
		if self.release_length == 0 {
			self.voices.remove(&(channel, note));
		} else {
			let Some(voice) = self.voices.get_mut(&(channel, note)) else {
				return
			};

			voice.release_point = voice.progress + buffer_progress;
			voice.released = true;
		}
	}

	pub fn kill_all_voices(&mut self) {
		self.voices.clear();
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
	fn resource_kind(&self) -> &'static str {
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

	fn save(&self) -> Vec<u8> {
		let mut result = vec![];
		
		for i in 0..self.channels.len() {
			if self.channels[i].is_empty() {
				continue
			}

			let channel_len = self.channels[i].len() as u64;

			result.push(i as u8);
			result.extend_from_slice(&channel_len.to_ne_bytes());
			result.reserve(channel_len as usize * size_of::<MidiNoteDesc>());

			for note in &self.channels[i] {
				result.extend_from_slice(&note.pos.0.to_ne_bytes());
				result.extend_from_slice(&note.len.0.to_ne_bytes());
				result.push(note.note);
				result.push(note.vel);
			}

		}

		result
	}

	fn load(&mut self, data: &[u8]) {
		*self = Self::default();

		let mut i = 0;

		while i < data.len() {
			let channel = data[i] as usize;
			
			i += 1;
			
			let channel_len = u64::from_ne_bytes(data[i..(i+8)].try_into().unwrap()) as usize;

			i += 8;

			self.channels[channel].reserve(channel_len);

			for _ in 0..channel_len {
				let pos = usize::from_ne_bytes(data[i..(i+8)].try_into().unwrap());
				
				i += 8;

				let len = usize::from_ne_bytes(data[i..(i+8)].try_into().unwrap());
				
				i += 8;

				let note = data[i];
				let vel = data[i+1];

				i += 2;

				self.channels[channel].push(MidiNoteDesc {
					pos: TlUnit(pos),
					len: TlUnit(len),
					note,
					vel
				});
			}
		}
	}
}