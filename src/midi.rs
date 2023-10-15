
#[derive(Debug, Clone, Default)]
pub struct MidiMessageChain {
	pub message: Option<MidiMessage>,
	pub next: Option<Box<MidiMessageChain>>,
}

impl MidiMessageChain {
	pub fn append(&mut self, msg: MidiMessage) {
		if self.message.is_none() {
			self.message = Some(msg);
		} else if let Some(next) = &mut self.next {
			next.append(msg)
		} else {
			self.next = Some(Box::new(MidiMessageChain { 
				message: Some(msg), 
				next: None
			}))
		}
	}
}

#[derive(Debug, Copy, Clone, Default)]
pub struct MidiMessage {
	pub data: [u8; 3],
}

impl MidiMessage {
	pub fn new(status: MidiStatusByte, data: [u8; 2]) -> Self {
		MidiMessage {
			data: [status.0, data[0], data[1]]
		}
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

impl MidiStatusByte {
	pub fn new(code: MidiStatusCode, channel: u8) -> Self {
		Self(code as u8 | (channel & 0xF))
	}
}