use std::{fmt::Debug, any::Any};

use crate::{engine::{Engine, Config, Frame}, util::db_to_factor, midi::{MidiMessageChain, MidiMessage, MidiStatusByte, MidiStatusCode}};

pub trait Node: Any {
	fn get_input_count(&self) -> usize;
	fn get_output_count(&self) -> usize;
	fn get_input_kind(&self, input: usize) -> BusKind;
	fn get_output_kind(&self, output: usize) -> BusKind;

	fn render(
		&self,
		output: usize,
		buffer: Buffer,
		instance: &NodeInstance,
		engine: &Engine
	);
	
	fn advance(
		&mut self,
		frames: usize,
		config: &Config
	);

	fn seek(
		&mut self,
		position: usize,
		config: &Config,
	);
}

pub const BEAT_DIVISIONS: u32 = 24;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineUnit(pub usize);

pub trait TimelineNode: Node {
	fn get_length(&self) -> TimelineUnit;
	fn set_position(&mut self, pos: TimelineUnit) -> TimelineUnit;
	fn set_start_offset(&mut self, offset: TimelineUnit);
	fn set_end_offset(&mut self, offset: TimelineUnit);
}

pub struct NodeInstance {
	pub inputs: Vec<Option<OutputRef>>,
	pub node: Box<dyn Node + Send>,
}

impl NodeInstance {
	pub fn render(&self, output: usize, buffer: Buffer, engine: &Engine) {
		self.node.render(output, buffer, self, engine);
	}

	pub fn new(node: impl Node + Send + 'static) -> Self {
		Self::new_dyn(Box::new(node))
	}

	pub fn new_dyn(node: Box<dyn Node + Send>) -> Self {
		NodeInstance {
			inputs: vec![None; node.get_input_count()],
			node
		}
	}
}

#[derive(Debug, Copy, Clone)]
pub struct OutputRef {
	pub node: usize,
	pub output: usize,
}

#[derive(Debug, Copy, Clone)]
pub enum BusKind {
	Audio,
	Midi,
	Control,
}

pub enum Buffer<'buf> {
	Audio(&'buf mut [Frame]),
	Midi(&'buf mut [MidiMessageChain]),
	Control(&'buf mut [f32]),
}

impl<'buf> Buffer<'buf> {
	fn bus_kind(&self) -> BusKind {
		match self {
			Buffer::Audio(_) => BusKind::Audio,
			Buffer::Midi(_) => BusKind::Midi,
			Buffer::Control(_) => BusKind::Control,
		}
	}
}

pub trait Effect {
	fn render_effect(&self, buffer: Buffer);
	fn advance_effect(&mut self, frames: usize, config: &Config);
}

pub trait Generator {

}

pub struct Source;
pub struct Sink;

impl<T: Effect + 'static> Node for T {
	fn get_input_count(&self) -> usize {
		1
	}

	fn get_output_count(&self) -> usize {
		1
	}

	fn get_input_kind(&self, _: usize) -> BusKind {
		BusKind::Audio
	}

	fn get_output_kind(&self, _: usize) -> BusKind {
		BusKind::Audio
	}

	fn advance(&mut self, frames: usize, config: &Config) {
		self.advance_effect(frames, config);
	}
	
	fn seek(&mut self, _: usize, _: &Config) { }

	fn render(&self, _: usize, buffer: Buffer, instance: &NodeInstance, engine: &Engine) {
		let Some(input) = &instance.inputs[0] else {
			// Input not connected, don't render anything
			return
		};

		let Buffer::Audio(buffer) = buffer else {
			panic!()
		};

		engine
			.get_node(input.node)
			.unwrap()
			.render(input.output, Buffer::Audio(buffer), engine);
		
		self.render_effect(Buffer::Audio(buffer));

	}
}

impl Node for Sink {
	fn get_input_count(&self) -> usize {
		1
	}

	fn get_output_count(&self) -> usize {
		0
	}

	fn get_input_kind(&self, _: usize) -> BusKind {
		BusKind::Audio
	}

	fn get_output_kind(&self, _: usize) -> BusKind {
		unreachable!()
	}

	fn render(&self, _: usize, buffer: Buffer, instance: &NodeInstance, engine: &Engine) {
		let Some(input) = &instance.inputs[0] else {
			// Input not connected, don't render anything
			return
		};

		let input_node = engine.get_node(input.node).unwrap();
		
		input_node.node.render(input.output, buffer, input_node, engine);
	}

	fn advance(&mut self, _: usize, _: &Config) { }

	fn seek(&mut self, _: usize, _: &Config) { }
}

impl Node for Source {
	fn get_input_count(&self) -> usize {
		0
	}

	fn get_output_count(&self) -> usize {
		1
	}

	fn get_input_kind(&self, _: usize) -> BusKind {
		unreachable!()
	}

	fn get_output_kind(&self, _: usize) -> BusKind {
		BusKind::Audio
	}
	
	fn advance(&mut self, _: usize, _: &Config) { }
	
	fn seek(&mut self, _: usize, _: &Config) { }

	fn render(&self, _: usize, buffer: Buffer, _: &NodeInstance, _: &Engine) {
		let Buffer::Audio(buffer) = buffer else {
			panic!()
		};

		buffer.fill(Frame([1.0f32, 0.0f32]));
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
			rate
		}
	}
}

impl Node for Sine {
	fn get_input_count(&self) -> usize {
		1
	}

	fn get_output_count(&self) -> usize {
		1
	}

	fn get_input_kind(&self, _: usize) -> BusKind {
		BusKind::Midi
	}

	fn get_output_kind(&self, _: usize) -> BusKind {
		BusKind::Audio
	}

	fn render(&self, _: usize, buffer: Buffer, instance: &NodeInstance, engine: &Engine) {
		let Buffer::Audio(buffer) = buffer else {
			panic!()
		};

		buffer
			.iter_mut()
			.enumerate()
			.for_each(|(i, f)| {
				let time = (self.pos + i) as f64 / engine.config.sample_rate as f64;
				f.0[0] = (std::f64::consts::TAU * time * self.rate).sin() as f32;
				f.0[1] = (std::f64::consts::TAU * time * self.rate).sin() as f32;
			});
	}

	fn advance(&mut self, frames: usize, _config: &Config) {
		self.pos += frames;
	}

	fn seek(&mut self, position: usize, _config: &Config) {
		self.pos = position;
	}
}

pub struct Gain {
	pub gain: f32,
}

impl Effect for Gain {
	fn render_effect(&self, buffer: Buffer) {
		let Buffer::Audio(buffer) = buffer else {
			panic!()
		};

		let fac = db_to_factor(self.gain);
		
		buffer
			.iter_mut()
			.for_each(|Frame([l, r])| {
				*l *= fac;
				*r *= fac;
			})
	}

	fn advance_effect(&mut self, _: usize, _: &Config) { }
}

pub struct Trigger {
	pub node_pos: TimelineUnit,
	pub tl_pos: usize,
}


impl Node for Trigger {
	fn get_input_count(&self) -> usize {
		0
	}

	fn get_output_count(&self) -> usize {
		1
	}

	fn get_input_kind(&self, _: usize) -> BusKind {
		unreachable!()
	}

	fn get_output_kind(&self, _: usize) -> BusKind {
		BusKind::Midi
	}

	fn render(
		&self,
		_output: usize,
		buffer: Buffer,
		_instance: &NodeInstance,
		engine: &Engine
	) {
		let Buffer::Midi(buffer) = buffer else {
			return
		};

		let node_pos_tl = engine.config.tl_units_to_frames(self.node_pos);
		
		if node_pos_tl > self.tl_pos {
			let relative = node_pos_tl - self.tl_pos;
			
			if relative < buffer.len() {
				buffer[relative].append(MidiMessage::new(
					MidiStatusByte::new(
						MidiStatusCode::NoteOn,
						0,
					),
					[0, 0]
				))
			}
		}
	}

	fn advance(
		&mut self,
		frames: usize,
		_config: &Config
	) {
		self.tl_pos += frames;
	}

	fn seek(
		&mut self,
		position: usize,
		_config: &Config,
	) {
		self.tl_pos = position;
	}
}

impl TimelineNode for Trigger {
	fn get_length(&self) -> TimelineUnit {
		todo!()
	}

	fn set_position(&mut self, pos: TimelineUnit) -> TimelineUnit {
		todo!()
	}

	fn set_start_offset(&mut self, offset: TimelineUnit) {
		todo!()
	}

	fn set_end_offset(&mut self, offset: TimelineUnit) {
		todo!()
	}
}