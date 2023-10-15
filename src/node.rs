use std::{fmt::Debug, any::Any, sync::{RwLock, atomic::{Ordering, AtomicUsize}}};

use crate::{engine::{Engine, Config, Frame}, util::db_to_factor, midi::MidiMessageChain};

pub trait Node: Any {
	fn get_input_count(&self) -> usize;
	fn get_output_count(&self) -> usize;
	fn get_input_kind(&self, input: usize) -> BusKind;
	fn get_output_kind(&self, output: usize) -> BusKind;

	fn render(
		&self,
		output: usize,
		buffer: BufferAccess,
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
	pub outputs: Vec<RwLock<Buffer>>,
	pub node: Box<dyn Node + Send>,
}

impl NodeInstance {
	pub fn render(&self, output: usize, samples: usize, engine: &Engine) {
		let buf = &mut *self.outputs[output].write().unwrap();

		if buf.len() >= samples {
			return
		} else {
			buf.resize(samples);
		}

		self.node.render(output, buf.get_buffer_access(), self, engine);
	}

	pub fn clear_buffers(&mut self) {
		for buffer in &mut self.outputs {
			match &mut *buffer.write().unwrap() {
				Buffer::Audio(buf) => buf.clear(),
				Buffer::Control(buf) => buf.clear(),
				Buffer::Midi(buf) => buf.clear()
			}
		}
	}

	pub fn new(node: impl Node + Send + 'static) -> Self {
		Self::new_dyn(Box::new(node))
	}

	pub fn new_dyn(node: Box<dyn Node + Send>) -> Self {
		let mut outputs = Vec::with_capacity(node.get_output_count());

		for output in 0..node.get_output_count() {
			outputs.push(RwLock::new(
				Buffer::from_bus_kind(node.get_output_kind(output))
			))
		}

		NodeInstance {
			inputs: vec![None; node.get_input_count()],
			outputs,
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

pub enum Buffer {
	Audio(Vec<Frame>),
	Midi(Vec<MidiMessageChain>),
	Control(Vec<f32>),
}

impl Buffer {
	fn from_bus_kind(kind: BusKind) -> Self {
		match kind {
			BusKind::Audio => Buffer::Audio(vec![]),
			BusKind::Midi => Buffer::Midi(vec![]),
			BusKind::Control => Buffer::Control(vec![]),
		}
	}

	fn get_buffer_access(&mut self) -> BufferAccess {
		match self {
			Buffer::Audio(buf) => BufferAccess::Audio(buf),
			Buffer::Control(buf) => BufferAccess::Control(buf),
			Buffer::Midi(buf) => BufferAccess::Midi(buf),
		}
	}

	fn len(&self) -> usize {
		match self {
			Buffer::Audio(buf) => buf.len(),
			Buffer::Midi(buf) => buf.len(),
			Buffer::Control(buf) => buf.len(),
		}
	}

	fn resize(&mut self, len: usize) {
		match self {
			Buffer::Audio(buf) => buf.resize(len, Frame([0.0; 2])),
			Buffer::Midi(buf) => buf.resize(len, MidiMessageChain::default()),
			Buffer::Control(buf) => buf.resize(len, 0.0),
		}

	}
}

pub enum BufferAccess<'buf> {
	Audio(&'buf mut [Frame]),
	Midi(&'buf mut [MidiMessageChain]),
	Control(&'buf mut [f32]),
}

pub trait Effect {
	fn render_effect(&self, buffer: BufferAccess);
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

	fn render(&self, _: usize, buffer: BufferAccess, instance: &NodeInstance, engine: &Engine) {
		let Some(input) = &instance.inputs[0] else {
			// Input not connected, don't render anything
			return
		};

		let BufferAccess::Audio(buffer) = buffer else {
			panic!()
		};

		let Buffer::Audio(buf) = &*engine.poll_node_output(input, buffer.len()) else {
			panic!()
		};

		buffer.copy_from_slice(buf);
		
		self.render_effect(BufferAccess::Audio(buffer));

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

	fn render(&self, _: usize, buffer: BufferAccess, instance: &NodeInstance, engine: &Engine) {
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

	fn render(&self, _: usize, buffer: BufferAccess, _: &NodeInstance, _: &Engine) {
		let BufferAccess::Audio(buffer) = buffer else {
			panic!()
		};

		buffer.fill(Frame([1.0f32, 0.0f32]));
	}
}

pub struct Sine {
	pos: usize,
	rate: f64,
	start: AtomicUsize,
	len: f64,
}

impl Sine {
	pub fn new(rate: f64) -> Self {
		Sine {
			pos: 0,
			rate,
			start: AtomicUsize::new(std::usize::MAX),
			len: 1.0,
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
		BusKind::Control
	}

	fn get_output_kind(&self, _: usize) -> BusKind {
		BusKind::Audio
	}

	fn render(&self, _: usize, buffer: BufferAccess, instance: &NodeInstance, engine: &Engine) {
		let BufferAccess::Audio(buffer) = buffer else {
			panic!()
		};
		
		let Some(input) = &instance.inputs[0] else {
			// Input not connected, don't render anything
			return
		};

		let Buffer::Control(input_buf) = &*engine.poll_node_output(input, buffer.len()) else {
			panic!()
		};
		
		buffer
			.iter_mut()
			.enumerate()
			.for_each(|(i, f)| {
				if input_buf[i] > 0.5 {
					self.start.store(self.pos + i, Ordering::Release)
				}

				let time = (self.pos + i) as f64 / engine.config.sample_rate as f64;
				let start = self.start.load(Ordering::Acquire);

				if start < std::usize::MAX {
					let amp = 1.0 - (time - start as f64 / engine.config.sample_rate as f64 * self.len).clamp(0.0, 1.0);

					f.0[0] = ((std::f64::consts::TAU * time * self.rate).sin() * amp) as f32;
					f.0[1] = ((std::f64::consts::TAU * time * self.rate).sin() * amp) as f32;
				}
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
	fn render_effect(&self, buffer: BufferAccess) {
		let BufferAccess::Audio(buffer) = buffer else {
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
		BusKind::Control
	}

	fn render(
		&self,
		_output: usize,
		buffer: BufferAccess,
		_instance: &NodeInstance,
		engine: &Engine
	) {
		let BufferAccess::Control(buffer) = buffer else {
			return
		};

		let node_pos_tl = engine.config.tl_units_to_frames(self.node_pos);
		
		if node_pos_tl > self.tl_pos {
			let relative = node_pos_tl - self.tl_pos;
			
			if relative < buffer.len() {
				buffer[relative] = 1.0;
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