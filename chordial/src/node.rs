use std::{collections::HashMap, fmt::{Debug, Display}, ops::Add, sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, RwLock, RwLockReadGuard}};

use crate::{engine::{Config, Engine, Frame}, midi::MidiMessageChain, param::{ParamKind, ParamValue, Parameter}, resource::ResourceHandleDyn, util::{inverse_lerp, lerp}};

pub mod effect;
pub mod io;
pub mod osc;
pub mod sampler;
pub mod timeline;

pub trait Node: Send {
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
	
	#[allow(unused_variables)]
	fn advance(&mut self, frames: usize, config: &Config) { }

	#[allow(unused_variables)]
	fn seek(&mut self, position: usize, config: &Config) { }


	// Resources
	//
	// A Resource is a way to store and exchange data between the engine and external
	// tools (such as Chordial Studio).	While Resources may have various underlying types,
	// they're exposed to the outside world through a homogenous ResourceAccess API.

	#[allow(unused_variables)]
	fn get_resource_names(&self) -> &'static [&'static str] { &[] }
	
	#[allow(unused_variables)]
	fn get_resource(&self, name: &str) -> &dyn ResourceHandleDyn { panic!() }


	// Timeline functionality
	//
	// A node with timeline support may override these functions.
	// To support timeline features, `is_timeline_node()` must return true when called.
	// `get_timeline_length()` should also be overridden to provide the node's base timeline length.
	// Timeline data can then be accessed from the node's NodeInstance at render time.

	fn is_timeline_node(&self) -> bool {
		false
	}

	#[allow(unused_variables)]
	fn get_timeline_length(&self, config: &Config) -> TlUnit {
		TlUnit(1)
	}

	fn process_outside_timeline_span(&self) -> bool {
		true
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
							.for_each(|(a, b)| a.append(&mut b.clone()))
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
						.for_each(|(a, b)| a.append(&mut b.clone()))
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

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TlUnit(pub usize);

impl Display for TlUnit {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

impl Add for TlUnit {
	type Output = TlUnit;
	
	fn add(self, rhs: Self) -> Self::Output {
		TlUnit(self.0 + rhs.0)
	}
}

#[derive(Default, Copy, Clone)]
pub struct TimelineTransform {
	pub position: TlUnit,
	pub start_offset: TlUnit,
	pub end_offset: TlUnit,
}


pub struct NodeInstance {
	pub inputs: Vec<(Vec<OutputRef>, RwLock<Buffer>)>,
	pub outputs: Vec<RwLock<Buffer>>,
	pub node: Box<dyn Node>,
	pub ctor: &'static str,
	metadata: HashMap<String, ParamValue>,
	tl_transform: Option<TimelineTransform>,
	params: Vec<(Parameter, ParamValue)>,
}

impl NodeInstance {
	pub fn new(node: impl Node + 'static, ctor: &'static str) -> Self {
		Self::new_dyn(Box::new(node), ctor)
	}

	pub fn new_dyn(node: Box<dyn Node>, ctor: &'static str) -> Self {
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
			
			tl_transform:
				if node.is_timeline_node() {
					Some(TimelineTransform::default())
				} else {
					None
				},
			
			metadata: HashMap::new(),
			node,
			ctor,
		}
	}

	pub fn get_metadata(&self, key: &str) -> Option<&ParamValue> {
		self.metadata.get(key)
	}

	pub fn set_metadata(&mut self, key: String, value: ParamValue) {
		assert!(!key.contains([' ', '\t', '\r', '\n']), "whitespace not allowed in metadata key!");

		self.metadata.insert(key, value);
	}

	pub fn metadata(&self) -> &HashMap<String, ParamValue> {
		&self.metadata
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

	pub fn is_timeline_node(&self) -> bool {
		self.tl_transform.is_some()
	}

	pub fn get_timeline_transform(&self) -> Option<&TimelineTransform> {
		self.tl_transform.as_ref()
	}
	
	pub fn get_timeline_position(&self) -> TlUnit {
		self.tl_transform.unwrap().position
	}

	pub fn get_timeline_start_offset(&self) -> TlUnit {
		self.tl_transform.unwrap().start_offset
	}

	pub fn get_timeline_end_offset(&self) -> TlUnit {
		self.tl_transform.unwrap().end_offset
	}

	pub fn set_timeline_transform(&mut self, tf: TimelineTransform) {
		*self.tl_transform.as_mut().unwrap() = tf
	}

	pub fn set_timeline_position(&mut self, pos: TlUnit) {
		self.tl_transform.as_mut().unwrap().position = pos
	}

	pub fn set_timeline_start_offset(&mut self, start_offset: TlUnit) {
		self.tl_transform.as_mut().unwrap().start_offset = start_offset
	}
	
	pub fn set_timeline_end_offset(&mut self, end_offset: TlUnit) {
		self.tl_transform.as_mut().unwrap().end_offset = end_offset
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
			Buffer::Audio(buf) => buf.resize(len, Frame::ZERO),
			Buffer::Midi(buf) => buf.resize(len, MidiMessageChain::default()),
			Buffer::Control(buf) => buf.resize(len, 0.0),
		}
	}
	
	pub fn audio(&self) -> Option<&[Frame]> {
		let Buffer::Audio(audio) = self else {
			panic!("called 'unwrap_audio' on a non-Audio Buffer!")
		};

		Some(audio)
	}
	
	pub fn midi(&self) -> Option<&[MidiMessageChain]> {
		let Buffer::Midi(midi) = self else {
			panic!("called 'unwrap_midi' on a non-Midi Buffer!")
		};
		
		Some(midi)
	}

	pub fn control(&self) -> Option<&[f32]> {
		let Buffer::Control(control) = self else {
			return None
		};
		
		Some(control)
	}

	pub fn audio_mut(&mut self) -> Option<&mut [Frame]> {
		let Buffer::Audio(audio) = self else {
			return None
		};

		Some(audio)
	}
	
	pub fn midi_mut(&mut self) -> Option<&mut [MidiMessageChain]> {
		let Buffer::Midi(midi) = self else {
			return None
		};
		
		Some(midi)
	}

	pub fn control_mut(&mut self) -> Option<&mut [f32]> {
		let Buffer::Control(control) = self else {
			return None
		};
		
		Some(control)
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
			BufferAccess::Audio(buf) => buf.fill(Frame::ZERO),
			BufferAccess::Control(buf) => buf.fill(0f32),
			BufferAccess::Midi(buf) => buf.fill(MidiMessageChain::default()),
		}
	}

	pub fn audio(&self) -> Option<&[Frame]> {
		let BufferAccess::Audio(audio) = self else {
			panic!("called 'unwrap_audio' on a non-Audio Buffer!")
		};

		Some(audio)
	}
	
	pub fn midi(&self) -> Option<&[MidiMessageChain]> {
		let BufferAccess::Midi(midi) = self else {
			panic!("called 'unwrap_midi' on a non-Midi Buffer!")
		};
		
		Some(midi)
	}

	pub fn control(&self) -> Option<&[f32]> {
		let BufferAccess::Control(control) = self else {
			return None
		};
		
		Some(control)
	}

	pub fn audio_mut(&mut self) -> Option<&mut [Frame]> {
		let BufferAccess::Audio(audio) = self else {
			return None
		};

		Some(audio)
	}
	
	pub fn midi_mut(&mut self) -> Option<&mut [MidiMessageChain]> {
		let BufferAccess::Midi(midi) = self else {
			return None
		};
		
		Some(midi)
	}

	pub fn control_mut(&mut self) -> Option<&mut [f32]> {
		let BufferAccess::Control(control) = self else {
			return None
		};
		
		Some(control)
	}
}


pub struct Envelope {
	pos: usize,
	start: AtomicUsize,
	end: AtomicUsize,
	active: AtomicBool,
}

impl Envelope {
	pub fn new() -> Self {
		Envelope {
			pos: 0,
			start: AtomicUsize::new(usize::MAX),
			end: AtomicUsize::new(usize::MAX),
			active: AtomicBool::new(false),
		}
	}

	pub fn get_gain_released(
		atk: f32,
		dec: f32,
		sus: f32,
		rel: f32,
		start_time: f32,
		release_time: f32,
		current_time: f32
	) -> f32 {
		let gain = Self::get_gain(
			atk, dec, sus, rel,
			start_time,
			release_time
		);

		let time = current_time - release_time;
		
		if time > rel {
			return 0.0
		}
		
		gain * inverse_lerp(rel, 0.0, time)
	}

	pub fn get_gain(
		atk: f32,
		dec: f32,
		sus: f32,
		_rel: f32,
		start_time: f32,
		current_time: f32
	) -> f32 {
		let mut time = current_time - start_time;
		
		if time < atk {
			return inverse_lerp(0.0, atk, time)
		}

		time -= atk;

		if time < dec {
			let t = inverse_lerp(0.0, dec, time);
			return lerp(0.0, sus, t)
		}

		sus
	}
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
		_output: usize,
		mut buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		let Some(atk_buf) = self.poll_input(0, buffer.len(), instance, engine) else {
			return
		};

		let Some(dec_buf) = self.poll_input(1, buffer.len(), instance, engine) else {
			return
		};

		let Some(sus_buf) = self.poll_input(2, buffer.len(), instance, engine) else {
			return
		};

		let Some(rel_buf) = self.poll_input(3, buffer.len(), instance, engine) else {
			return
		};

		let Some(trig_buf) = self.poll_input(4, buffer.len(), instance, engine) else {
			return
		};

		let buffer = buffer.control_mut().unwrap();
		let atk_buf = atk_buf.control().unwrap();
		let dec_buf = dec_buf.control().unwrap();
		let sus_buf = sus_buf.control().unwrap();
		let rel_buf = rel_buf.control().unwrap();
		let trig_buf = trig_buf.control().unwrap();
		
		buffer
			.iter_mut()
			.enumerate()
			.for_each(|(i, f)| {
				let mut active = self.active.load(Ordering::Acquire);

				if !active && trig_buf[i] >= 0.5 {
					self.start.store(self.pos + i, Ordering::Release);
					self.active.store(true, Ordering::Release);
					active = true;

				} else if active && trig_buf[i] < 0.5 {
					self.end.store(self.pos + i, Ordering::Release);
					self.active.store(false, Ordering::Release);
					active = false;
				}
				
				let start = self.start.load(Ordering::Acquire);

				if self.pos + i < start {
					return
				}

				let start_secs = start as f32 / engine.config.sample_rate as f32;
				let time_secs = (self.pos + i) as f32 / engine.config.sample_rate as f32;
				
				if active {
					*f = Self::get_gain(
						atk_buf[i], dec_buf[i], sus_buf[i], rel_buf[i],
						start_secs,
						time_secs
					);
				} else {
					let end_secs = self.end.load(Ordering::Acquire) as f32 / engine.config.sample_rate as f32;

					*f = Self::get_gain_released(
						atk_buf[i], dec_buf[i], sus_buf[i], rel_buf[i],
						start_secs,
						end_secs,
						time_secs
					);
				}
			})
	}

	fn advance(
		&mut self,
		frames: usize,
		_config: &Config
	) {
		self.pos += frames;
	}

	fn seek(
		&mut self,
		position: usize,
		_config: &Config,
	) {
		self.pos = position;
	}
}




pub struct Trigger {
	pub tl_pos: usize,
}

impl Trigger {
	pub fn new() -> Self {
		Trigger {
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
		mut buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		let buffer = buffer.control_mut().unwrap();
		let node_pos_tl = engine.config.tl_units_to_frames(instance.get_timeline_position());
		
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
}

pub struct ControlValue {
	pub value: f32,
}

impl Node for ControlValue {
	fn get_name(&self) -> &'static str {
		"Control Value"
	}

	fn render(
			&self,
			_output: usize,
			buffer: BufferAccess,
			_instance: &NodeInstance,
			_engine: &Engine
		) {
		let BufferAccess::Control(control) = buffer else {
			return
		};

		control.fill(self.value);
	}

	fn advance(
		&mut self,
		_frames: usize,
		_config: &Config
	) { }

	fn seek(
		&mut self,
		_position: usize,
		_config: &Config,
	) { }

	fn get_inputs(&self) -> &[BusKind] {
		&[]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Control]
	}

	fn get_output_names(&self) -> &'static [&'static str] {
		&["out"]
	}

	fn get_params(&self) -> &[Parameter] {
		&[
			Parameter {
				kind: ParamKind::Float,
				text: "value",
			}
		]
	}

	fn get_param_default_value(&self, _param: usize) -> Option<ParamValue> {
		Some(ParamValue::Float(0.0))
	}

	fn param_updated(&mut self, _param: usize, value: &ParamValue) {
		let ParamValue::Float(value) = value else {
			panic!()
		};

		self.value = *value as f32;
	}
}