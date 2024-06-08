use crate::{engine::{Engine, Frame}, midi::{MidiMessage, MidiStatusByte}, node::NodeUtil, param::{ParamKind, ParamValue, Parameter}};

use super::{BufferAccess, BusKind, Node, NodeInstance};


pub struct Source;
pub struct Sink;

impl Node for Sink {
	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_name(&self) -> &'static str {
		"Sink"
	}

	fn render(&self, _: usize, mut buffer: BufferAccess, instance: &NodeInstance, engine: &Engine) {
		self.poll_input_into_buffer(0, &mut buffer, instance, engine);
	}
}

impl Node for Source {
	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}
	
	fn get_name(&self) -> &'static str {
		"Source"
	}
	
	fn render(&self, _: usize, buffer: BufferAccess, _: &NodeInstance, _: &Engine) {
		let BufferAccess::Audio(buffer) = buffer else {
			panic!()
		};

		buffer.fill(Frame(0.0f32, 0.0f32));
	}

	fn get_params(&self) -> &[Parameter] { 
		&[
			Parameter {
				kind: ParamKind::String,
				text: "input",
			}
		]
	}

	fn param_updated(&mut self, param: usize, value: &ParamValue) {
		assert!(param == 0);

		let ParamValue::String(string) = value else {
			panic!()
		};

		if string != "" {
			todo!()
		}
	}
}

pub struct MidiSplit {
	keep_channel: bool,
}

impl MidiSplit {
	pub fn new() -> Self {
		MidiSplit {
			keep_channel: false,
		}
	}
}

impl Node for MidiSplit {
	fn get_name(&self) -> &'static str {
		"MIDI Split"
	}

	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Midi]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Midi; 16]
	}

	fn get_input_names(&self) -> &'static [&'static str] {
		&["in"]
	}

	fn get_output_names(&self) -> &'static [&'static str] {
		&[
			"1",  "2",  "3",  "4",
			"5",  "6",  "7",  "8",
			"9",  "10", "11", "12",
			"13", "14", "15", "16",
		]
	}

	fn get_params(&self) -> &[Parameter] {
		&[
			Parameter {
				kind: ParamKind::Bool,
				text: "keep_channel",
			}
		]
	}

	fn param_updated(&mut self, _param: usize, value: &ParamValue) {
		let ParamValue::Bool(boolean) = value else {
			panic!()
		};

		self.keep_channel = *boolean;
	}

	fn render(
		&self,
		output: usize,
		mut buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		let Some(input) = self.poll_input(0, buffer.len(), instance, engine) else {
			return
		};
		
		let input = input.midi().unwrap();
		let buffer = buffer.midi_mut().unwrap();

		buffer
			.iter_mut()
			.zip(input)
			.for_each(|(b, m)| {
				
				for msg in m {
					if msg.status_byte().channel() != output as u8 {
						continue
					}

					if self.keep_channel {
						b.push(*msg);
					} else {
						b.push(MidiMessage::new(
							MidiStatusByte::new(msg.status_byte().code(), 0),
							[msg.data()[1], msg.data()[2]]
						));
					}
				}
			});
	}
}