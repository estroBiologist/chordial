use std::{any::Any, fmt::{Debug, Display}, sync::{atomic::{AtomicUsize, Ordering}, RwLock, RwLockReadGuard}};

use crate::{engine::{Engine, Config, Frame}, util::db_to_factor, midi::{MidiMessageChain, MidiNoteDesc}, adsr::EnvelopeDepr, param::{Parameter, ParamValue, ParamKind}};

pub trait Node: Send + Any {
	fn get_inputs(&self) -> &[BusKind] { &[] }
	fn get_outputs(&self) -> &[BusKind] { &[] }

	fn get_input_names(&self) -> &'static [&'static str] { &[] }
	fn get_output_names(&self) -> &'static [&'static str] { &[] }
	
	#[allow(unused_variables)]
	fn param_updated(&mut self, param: usize, value: &ParamValue) { }

	#[allow(unused_variables)]
	fn get_param_default_value(&self, param: usize) -> Option<ParamValue> { None }

	fn get_params(&self) -> &[Parameter] { &[] }

	fn get_name(&self) -> &'static str;

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

	// Timeline functionality
	//
	// A node with timeline support may override these functions.
	// To support timeline features, `is_timeline_node()` should return true,
	// and the relevant functions should be overridden to handle timeline repositioning.
	
	fn is_timeline_node(&self) -> bool {
		false
	}

	fn get_length(&self) -> TimelineUnit {
		panic!()
	}

	#[allow(unused_variables)]
	fn set_position(&mut self, pos: TimelineUnit) {
		panic!()
	}

	#[allow(unused_variables)]
	fn set_start_offset(&mut self, offset: TimelineUnit) {
		panic!()
	}

	#[allow(unused_variables)]
	fn set_end_offset(&mut self, offset: TimelineUnit) {
		panic!()
	}
}


pub trait NodeUtil {
	fn poll_input<'buf>(
		&self,
		input: usize,
		buffer_len: usize,
		instance: &'buf NodeInstance,
		engine: &'buf Engine
	) -> Option<RwLockReadGuard<'buf, Buffer>>;

	fn poll_input_into_buffer(
		&self,
		input: usize,
		buffer: &mut BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	);
}

impl<T: Node> NodeUtil for T {
	fn poll_input<'buf>(
		&self,
		input: usize,
		buffer_len: usize,
		instance: &'buf NodeInstance,
		engine: &'buf Engine
	) -> Option<RwLockReadGuard<'buf, Buffer>> {
		let refs = &instance.inputs[input];

		if refs.0.len() < 2 {
			let [output_ref] = instance.inputs[input].0.as_slice() else {
				return None
			};

			Some(engine.poll_node_output(output_ref, buffer_len))
		} else {
			let mut access = refs.1.write().unwrap();

			for output_ref in &refs.0 {
				let buf = &*engine.poll_node_output(output_ref, buffer_len);
				
				if access.len() != buffer_len {
					if access.len() == 0 {
						*access = Buffer::from_bus_kind(buf.get_bus_kind());
					}

					access.resize(buffer_len);
				}

				match (&mut *access, buf) {
					(Buffer::Audio(access), Buffer::Audio(buf)) => {
						access
							.iter_mut()
							.zip(buf)
							.for_each(|(a, b)| *a += *b);
							
					}
	
					(Buffer::Midi(access), Buffer::Midi(buf)) => {
						access
							.iter_mut()
							.zip(buf)
							.for_each(|(a, b)| a.append_chain(b.clone()))
					}
	
					(Buffer::Control(access), Buffer::Control(buf)) => {
						access
							.iter_mut()
							.zip(buf)
							.for_each(|(a, b)| *a += *b);
					}
	
					_ => panic!()
				}
			}
			
			drop(access);

			Some(refs.1.read().unwrap())
		}
	}

	fn poll_input_into_buffer(
		&self,
		input: usize,
		mut buffer: &mut BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {

		let refs = &instance.inputs[input];

		let mut access = refs.1.write().unwrap();

		for output_ref in &refs.0 {
			let buf = &*engine.poll_node_output(output_ref, buffer.len());
			
			if access.len() != buffer.len() {
				if access.len() == 0 {
					*access = Buffer::from_bus_kind(buf.get_bus_kind());
				}

				access.resize(buffer.len());
			}

			match (&mut buffer, buf) {
				(BufferAccess::Audio(access), Buffer::Audio(buf)) => {
					access
						.iter_mut()
						.zip(buf)
						.for_each(|(a, b)| *a += *b);
						
				}

				(BufferAccess::Midi(access), Buffer::Midi(buf)) => {
					access
						.iter_mut()
						.zip(buf)
						.for_each(|(a, b)| a.append_chain(b.clone()))
				}

				(BufferAccess::Control(access), Buffer::Control(buf)) => {
					access
						.iter_mut()
						.zip(buf)
						.for_each(|(a, b)| *a += *b);
				}

				_ => panic!(),
			}
		}
		
		drop(access);
	}
}

pub const BEAT_DIVISIONS: u32 = 24;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimelineUnit(pub usize);

impl Display for TimelineUnit {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}


pub struct NodeInstance {
	pub inputs: Vec<(Vec<OutputRef>, RwLock<Buffer>)>,
	pub outputs: Vec<RwLock<Buffer>>,
	pub node: Box<dyn Node>,
	pub id: &'static str,
	params: Vec<(Parameter, ParamValue)>,
}

impl NodeInstance {
	pub fn new(node: impl Node + 'static, id: &'static str) -> Self {
		Self::new_dyn(Box::new(node), id)
	}

	pub fn new_dyn(node: Box<dyn Node>, id: &'static str) -> Self {
		NodeInstance {
			inputs: node
						.get_inputs()
						.iter()
						.map(|_| (vec![], RwLock::new(Buffer::from_bus_kind(BusKind::Control))))
						.collect(),
			outputs: node
						.get_outputs()
						.iter()
						.copied()
						.map(Buffer::from_bus_kind)
						.map(RwLock::new)
						.collect(),
			params: node
						.get_params()
						.iter()
						.copied()
						.map(|desc| (desc, ParamValue::from_desc(desc)))
						.collect(),
			node,
			id,
		}
	}

	pub fn get_params(&self) -> &[(Parameter, ParamValue)] {
		&self.params
	}

	pub fn set_param(&mut self, param: usize, value: ParamValue) {
		self.node.param_updated(param, &value);
		self.params[param].1.set(value);
	}

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
		for (_, buffer) in &mut self.inputs {
			if buffer.read().unwrap().len() > 0 {
				buffer.write().unwrap().clear();
			}
		}

		for buffer in &mut self.outputs {
			match &mut *buffer.write().unwrap() {
				Buffer::Audio(buf) => buf.clear(),
				Buffer::Control(buf) => buf.clear(),
				Buffer::Midi(buf) => buf.clear()
			}
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
	pub fn from_bus_kind(kind: BusKind) -> Self {
		match kind {
			BusKind::Audio => Buffer::Audio(vec![]),
			BusKind::Midi => Buffer::Midi(vec![]),
			BusKind::Control => Buffer::Control(vec![]),
		}
	}

	pub fn get_bus_kind(&self) -> BusKind {
		match self {
			Buffer::Audio(_) => BusKind::Audio,
			Buffer::Control(_) => BusKind::Control,
			Buffer::Midi(_) => BusKind::Midi,
		}
	}

	pub fn get_buffer_access(&mut self) -> BufferAccess {
		match self {
			Buffer::Audio(buf) => BufferAccess::Audio(buf),
			Buffer::Control(buf) => BufferAccess::Control(buf),
			Buffer::Midi(buf) => BufferAccess::Midi(buf),
		}
	}

	pub fn clear(&mut self) {
		match self {
			Buffer::Audio(buf) => buf.clear(),
			Buffer::Control(buf) => buf.clear(),
			Buffer::Midi(buf) => buf.clear(),
		}
	}

	pub fn len(&self) -> usize {
		match self {
			Buffer::Audio(buf) => buf.len(),
			Buffer::Midi(buf) => buf.len(),
			Buffer::Control(buf) => buf.len(),
		}
	}

	pub fn capacity(&self) -> usize {
		match self {
			Buffer::Audio(buf) => buf.capacity(),
			Buffer::Midi(buf) => buf.capacity(),
			Buffer::Control(buf) => buf.capacity(),
		}
	}

	pub fn resize(&mut self, len: usize) {
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

impl<'buf> BufferAccess<'buf> {
	fn len(&self) -> usize {
		match self {
			BufferAccess::Audio(buf) => buf.len(),
			BufferAccess::Midi(buf) => buf.len(),
			BufferAccess::Control(buf) => buf.len(),
		}
	}

	pub fn get_bus_kind(&self) -> BusKind {
		match self {
			BufferAccess::Audio(_) => BusKind::Audio,
			BufferAccess::Control(_) => BusKind::Control,
			BufferAccess::Midi(_) => BusKind::Midi,
		}
	}

	pub fn clear(&mut self) {
		match self {
			BufferAccess::Audio(buf) => buf.fill(Frame([0f32; 2])),
			BufferAccess::Control(buf) => buf.fill(0f32),
			BufferAccess::Midi(buf) => buf.fill(MidiMessageChain::default()),
		}
	}

	
}

pub trait Effect: Send {
	fn render_effect(&self, buffer: BufferAccess);
	fn advance_effect(&mut self, frames: usize, config: &Config);

	#[allow(unused_variables)]
	fn param_updated(&mut self, param: usize, value: &ParamValue) { }

	#[allow(unused_variables)]
	fn get_param_default_value(&self, param: usize) -> Option<ParamValue> { None }

	fn get_params(&self) -> &[Parameter] { &[] }

	fn get_name(&self) -> &'static str;
}

pub trait Generator {

}

pub struct Source;
pub struct Sink;

impl<T: Effect + 'static> Node for T {
	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_input_names(&self) -> &'static [&'static str] {
		&["in"]
	}

	fn get_output_names(&self) -> &'static [&'static str] {
		&["out"]
	}

	fn get_name(&self) -> &'static str {
		Effect::get_name(self)
	}

	fn advance(&mut self, frames: usize, config: &Config) {
		self.advance_effect(frames, config);
	}
	
	fn seek(&mut self, _: usize, _: &Config) { }

	fn render(&self, _: usize, mut buffer: BufferAccess, instance: &NodeInstance, engine: &Engine) {
		self.poll_input_into_buffer(0, &mut buffer, instance, engine);
		self.render_effect(buffer);
	}

	fn get_param_default_value(&self, param: usize) -> Option<ParamValue> {
		Effect::get_param_default_value(self, param)
	}

	fn get_params(&self) -> &[Parameter] {
		Effect::get_params(self)
	}

	fn param_updated(&mut self, param: usize, value: &ParamValue) {
		Effect::param_updated(self, param, value)
	}
}

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

	fn advance(&mut self, _: usize, _: &Config) { }

	fn seek(&mut self, _: usize, _: &Config) { }
}

impl Node for Source {
	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}
	
	fn get_name(&self) -> &'static str {
		"Source"
	}
	
	fn advance(&mut self, _: usize, _: &Config) { }
	
	fn seek(&mut self, _: usize, _: &Config) { }

	fn render(&self, _: usize, buffer: BufferAccess, _: &NodeInstance, _: &Engine) {
		let BufferAccess::Audio(buffer) = buffer else {
			panic!()
		};

		buffer.fill(Frame([1.0f32, 0.0f32]));
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
	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Control]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_name(&self) -> &'static str {
		"Sine"
	}

	fn get_input_names(&self) -> &'static [&'static str] {
		&["trigger"]
	}

	fn get_output_names(&self) -> &'static [&'static str] {
		&["out"]
	}

	fn render(&self, _: usize, buffer: BufferAccess, instance: &NodeInstance, engine: &Engine) {
		let BufferAccess::Audio(buffer) = buffer else {
			panic!()
		};
		
		let Some(input_buf) = self.poll_input(0, buffer.len(), instance, engine) else {
			return
		};


		let Buffer::Control(input_buf) = &*input_buf else {
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

	fn get_params(&self) -> &[Parameter] {
		&[
			Parameter {
				kind: ParamKind::Float,
				text: "freq",
			}
		]
	}
	
	fn get_param_default_value(&self, _: usize) -> Option<ParamValue> {
		Some(ParamValue::Float(440.0))
	}

	fn param_updated(&mut self, _: usize, value: &ParamValue) {
		let ParamValue::Float(val) = value else {
			panic!()
		};

		self.rate = *val;
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

	fn get_params(&self) -> &[Parameter] {
		&[
			Parameter {
				kind: ParamKind::Float,
				text: "gain",
			}
		]
	}

	fn param_updated(&mut self, _: usize, value: &ParamValue) {
		let ParamValue::Float(val) = value else {
			panic!()
		};

		self.gain = *val as f32;
	}

	fn get_name(&self) -> &'static str {
		"Gain"
	}
}

pub struct Trigger {
	pub node_pos: TimelineUnit,
	pub tl_pos: usize,
}

impl Trigger {
	pub fn new() -> Self {
		Trigger {
			node_pos: TimelineUnit(0),
			tl_pos: 0,
		}
	}
}

impl Node for Trigger {
	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Control]
	}

	fn get_name(&self) -> &'static str {
		"Trigger"
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
		
		if node_pos_tl >= self.tl_pos {
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

	fn is_timeline_node(&self) -> bool {
		true
	}

	fn get_length(&self) -> TimelineUnit {
		TimelineUnit(1)
	}

	fn set_position(&mut self, pos: TimelineUnit) {
		self.node_pos = pos
	}

	fn set_start_offset(&mut self, offset: TimelineUnit) {
	}

	fn set_end_offset(&mut self, offset: TimelineUnit) {
	}
}

struct Osc {
	pos: usize,
	notes: RwLock<Vec<MidiNoteDesc>>,
	envelope: EnvelopeDepr,
}

impl Node for Osc {
	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Midi]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_name(&self) -> &'static str {
		"Osc"
	}

	fn render(
		&self,
		output: usize,
		buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		todo!()
	}

	fn advance(
		&mut self,
		frames: usize,
		config: &Config
	) {
		todo!()
	}

	fn seek(
		&mut self,
		position: usize,
		config: &Config,
	) {
		todo!()
	}
}


pub struct Envelope {

}

impl Node for Envelope {
	fn get_name(&self) -> &'static str {
		"Envelope"
	}

	fn get_inputs(&self) -> &[BusKind] {
		// A, D, S, R, Trigger
		&[BusKind::Control; 5]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Control]
	}

	fn get_input_names(&self) -> &'static [&'static str] {
		&["atk", "dec", "sus", "rel", "trig"]
	}

	fn get_output_names(&self) -> &'static [&'static str] {
		&["amp"]
	}

	fn render(
		&self,
		output: usize,
		buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		
	}

	fn advance(
		&mut self,
		frames: usize,
		config: &Config
	) {
		
	}

	fn seek(
		&mut self,
		position: usize,
		config: &Config,
	) {
		
	}
}