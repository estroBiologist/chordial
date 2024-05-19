use std::{collections::{BTreeMap, HashMap}, fmt::{Debug, Write}, ops::{Add, AddAssign}, path::Path, sync::{Arc, RwLock, RwLockReadGuard}, time::Instant};

use crate::{midi::MidiBlock, node::{effect::{Amplify, Gain}, io::{MidiSplit, Sink}, osc::{Osc, PolyOsc, Sine}, timeline::MidiClip, Buffer, BufferAccess, BusKind, ControlValue, Envelope, Node, NodeInstance, OutputRef, Step, Trigger}, param::ParamValue, resource::{Resource, ResourceHandle, ResourceHandleDyn}};


pub const BEAT_DIVISIONS: u32 = 24;

#[derive(Copy, Clone)]
pub struct Frame(pub [f32; 2]);

impl Debug for Frame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("({:?})", self.0))
	}
}

impl AddAssign for Frame {
	fn add_assign(&mut self, rhs: Self) {
		self.0[0] += rhs.0[0];
		self.0[1] += rhs.0[1];
	}
}

impl Add for Frame {
	type Output = Frame;

	fn add(self, rhs: Self) -> Self::Output {
		Frame([self.0[0] + rhs.0[0], self.0[1] + rhs.0[1]])
	}
}

impl Frame {
	pub const ZERO: Frame = Frame([0f32; 2]);
}


pub struct Config {
	pub sample_rate: u32,
	pub bpm: f64,
	pub tuning: f32,
}

impl Config {
	pub fn midi_note_to_freq(&self, note: u8) -> f32 {
		2.0f32.powf((note as f32 - 69.0) / 12.0) * self.tuning
	}
}

pub type NodeCtor = Arc<dyn Fn(&mut Engine) -> Box<dyn Node> + Send + Sync>;
pub type ResourceCtor = Arc<dyn Fn(&mut Engine) -> Box<dyn ResourceHandleDyn> + Send + Sync>;

pub struct Engine {
	pub config: Config,
	pub playing: bool,
	
	nodes: BTreeMap<usize, NodeInstance>,
	node_ctors: HashMap<&'static str, NodeCtor>,
	node_counter: usize,

	resources: HashMap<&'static str, Vec<Box<dyn ResourceHandleDyn>>>,
	resource_ctors: HashMap<&'static str, ResourceCtor>,

	position: usize,
	
	pub enable_buffer_readback: bool,
	pub buffer_readback: Vec<Frame>,

	pub dbg_buffer_size: u32,
	pub dbg_buffer_time: f32,
	pub dbg_process_time: f32,
}

impl Engine {
	pub fn new(sample_rate: u32) -> Self {
		let mut engine = Engine {
			config: Config {
				sample_rate,
				bpm: 120.0,
				tuning: 440.0,
			},

			playing: false,

			nodes: BTreeMap::new(),
			node_ctors: HashMap::new(),
			node_counter: 0,
			
			resources: HashMap::new(),
			resource_ctors: HashMap::new(),

			position: 0,
			
			enable_buffer_readback: false,
			buffer_readback: vec![],
			dbg_buffer_size: 0u32,
			dbg_buffer_time: 0f32,
			dbg_process_time: 0f32,
		};

		engine.register_resource(|_| MidiBlock::default());

		engine.register_node("chordial.amplify", |_| Box::new(Amplify));
		engine.register_node("chordial.sink", |_| Box::new(Sink));
		engine.register_node("chordial.sine", |_| Box::new(Sine::new(440.0)));
		engine.register_node("chordial.gain", |_| Box::new(Gain { gain: 0.0 }));
		engine.register_node("chordial.trigger", |_| Box::new(Trigger::new()));
		engine.register_node("chordial.envelope", |_| Box::new(Envelope::new()));
		engine.register_node("chordial.control_value", |_| Box::new(ControlValue { value: 0.0f32 }));
		engine.register_node("chordial.osc", |_| Box::new(Osc::new()));
		engine.register_node("chordial.polyosc", |_| Box::new(PolyOsc::new()));
		engine.register_node("chordial.midi_split", |_| Box::new(MidiSplit::new()));
		engine.register_node("chordial.midi_clip", |engine| Box::new(MidiClip::new(engine.add_resource(MidiBlock::default()))));

		engine.create_node("chordial.sink");
		engine
	}

	pub fn load_from_file(&mut self, path: &Path) {
		self.nodes.clear();
		self.node_counter = 0;

		let file = std::fs::read_to_string(path).unwrap();

		// fucking windows
		let file = file.replace("\r\n", "\n");

		let mut lines = file.split("\n");
		
		let mut current = lines.next();

		while let Some(line) = current {
			// skip comment lines
			if let Some(';') = line.chars().next() {
				current = lines.next();
				continue
			}
			// skip empty lines
			if line.is_empty() {
				current = lines.next();
				continue
			}

			let (idx, name) = line.split_at(line.find(" ").unwrap());

			let name = &name[1..];
			let idx = idx.parse::<usize>().unwrap();

			let Some(ctor) = self.node_ctors.get(name) else {
				panic!("unknown node constructor `{name}`")
			};

			let node = ctor.clone()(self);

			let (id, _) = self.node_ctors.get_key_value(name).unwrap();
			let mut node = NodeInstance::new_dyn(node, id);
			
			node.inputs.clear();
			current = lines.next();

			let mut param_counter = 0;

			// parse inputs and parameters
			loop {
				let Some(line) = current else {
					break
				};

				// skip empty lines
				if line.is_empty() {
					current = lines.next();
					continue
				}

				if line.starts_with("in ") {
					let inputs = line[3..].split(" ").collect::<Vec<_>>();
					let mut input_data = (vec![], RwLock::new(Buffer::from_bus_kind(BusKind::Control)));

					for input_node in &inputs {
						let input_node = input_node.split(".").collect::<Vec<_>>();
						let [noderef, output] = input_node.as_slice() else {
							panic!()
						};
					
						input_data.0.push(OutputRef {
							node: noderef.parse().unwrap(),
							output: output.parse().unwrap(),
						});
					}

					if input_data.0.len() > 2 {
						input_data.1 = RwLock::new(
							Buffer::from_bus_kind(node.node.get_inputs()[node.inputs.len()])
						);
					}

					node.inputs.push(input_data);
					
				} else if line == "in" {
					node.inputs.push((vec![], RwLock::new(Buffer::from_bus_kind(BusKind::Control))));
				} else if line.starts_with("param ") {
					node.set_param(param_counter, ParamValue::parse(&line[6..]));
					param_counter += 1;
				} else {
					break
				}

				current = lines.next();
			}

			self.nodes.insert(idx, node);
			self.node_counter = self.node_counter.max(idx + 1);
		}
	}

	pub fn render(&mut self, buffer: &mut [Frame]) {
		let start = Instant::now();

		if !self.playing {
			buffer.fill(Frame::ZERO);
			
			if self.enable_buffer_readback {
				self.buffer_readback.resize(buffer.len(), Frame::ZERO);
				self.buffer_readback.fill(Frame::ZERO);
			}

			return
		}

		let sink = &self.nodes[&0];

		sink.node.render(0, BufferAccess::Audio(buffer), sink, self);

		for node in self.nodes.values_mut() {
			node.node.advance(buffer.len(), &self.config);
			node.clear_buffers();
		}

		self.position += buffer.len();
		
		self.dbg_process_time = (Instant::now() - start).as_secs_f32();
		self.dbg_buffer_time = buffer.len() as f32 / self.config.sample_rate as f32;
		self.dbg_buffer_size = buffer.len() as u32;

		if self.enable_buffer_readback {
			self.buffer_readback.resize(buffer.len(), Frame::ZERO);
			self.buffer_readback.copy_from_slice(&buffer);
		}
	}

	pub fn seek(&mut self, position: usize) {
		self.position = position;

		for node in &mut self.nodes {
			node.1.node.seek(position, &self.config)
		}
	}

	pub fn position(&self) -> usize {
		self.position
	}

	pub fn register_node(
		&mut self, 
		name: &'static str, 
		ctor: impl Fn(&mut Engine) -> Box<dyn Node> + Send + Sync + 'static
	) {
		if self.node_ctors.contains_key(name) {
			panic!("constructor `{name}` already registered!")
		}

		self.node_ctors.insert(name, Arc::new(ctor));
	}

	pub fn create_node(&mut self, name: &str) -> Option<usize> {
		let Some(ctor) = self.node_ctors.get(name) else {
			eprintln!("warning: unknown node constructor `{name}`, skipping");
			return None
		};
		let node = ctor.clone()(self);

		let (id, _) = self.node_ctors.get_key_value(name).unwrap();
		
		Some(self.add_node_dyn(node, id))
	}

	pub fn add_node_instance(&mut self, node: NodeInstance) {
		self.nodes.insert(self.node_counter, node);
		self.node_counter += 1;
	}

    pub fn add_node(&mut self, node: impl Node + 'static, id: &'static str) -> usize {
        self.nodes.insert(self.node_counter, NodeInstance::new(node, id));
        self.node_counter += 1;
		self.node_counter - 1
    }

	pub fn add_node_dyn(&mut self, node: Box<dyn Node + 'static>, id: &'static str) -> usize {
		self.nodes.insert(self.node_counter, NodeInstance::new_dyn(node, id));
		self.node_counter += 1;
		self.node_counter - 1
	}

	pub fn get_node(&self, node: usize) -> Option<&NodeInstance> {
		self.nodes.get(&node)
	}

	pub fn get_node_mut(&mut self, node: usize) -> Option<&mut NodeInstance> {
		self.nodes.get_mut(&node)
	}

	pub fn get_node_count(&self) -> usize {
		self.nodes.len()
	}

	pub fn has_node(&self, node: usize) -> bool {
		self.nodes.contains_key(&node)
	}

	pub fn delete_node(&mut self, node: usize) {
		let Some(_) = self.nodes.remove(&node) else {
			return
		};

		for other in self.nodes.values_mut() {
			for input in &mut other.inputs {
				input.0.retain(|input_node| input_node.node != node);
			}
		}
	}

	pub fn nodes(&self) -> impl Iterator<Item = (&usize, &NodeInstance)> {
		self.nodes.iter()
	}

	pub fn nodes_mut(&mut self) -> impl Iterator<Item = (&usize, &mut NodeInstance)> {
		self.nodes.iter_mut()
	}

	pub fn poll_node_output<'access>(
		&'access self,
		output_ref: &OutputRef,
		buffer_len: usize
	) -> RwLockReadGuard<'access, Buffer> {
		let input_node = self.get_node(output_ref.node).unwrap();
		
		input_node.render(output_ref.output, buffer_len, self);

		input_node.outputs[output_ref.output].read().unwrap()
	}

	pub fn node_constructors(&self) -> impl Iterator<Item = &str> {
		self.node_ctors.keys().copied()
	}

	pub fn get_debug_info(&self) -> String {
		let mut result = String::new();

		for node in &self.nodes {
			writeln!(result, "node {}:", node.0).unwrap();
			writeln!(result, "  id:\t{}", node.1.id).unwrap();
			writeln!(result, "  name:\t{}", node.1.node.get_name()).unwrap();
			
			for i in 0..node.1.inputs.len() {
				let input = &node.1.inputs[i];

				writeln!(result, "  input {}:", i).unwrap();
				
				for out_ref in &input.0 {
					writeln!(result, "    {}.{}", out_ref.node, out_ref.output).unwrap();
				}

				let buf = input.1.read().unwrap();

				writeln!(result, "    buffer capacity: {}", buf.capacity()).unwrap();
			}

			for i in 0..node.1.outputs.len() {
				let output = &node.1.outputs[i];

				writeln!(result, "  output {}:", i).unwrap();

				let buf = output.read().unwrap();

				writeln!(result, "    buffer capacity: {}", buf.capacity()).unwrap();
			}
		}

		result
	}
	
	pub fn register_resource<T: Resource + 'static>(
		&mut self,
		ctor: impl Fn(&mut Engine) -> T + Send + Sync + 'static,
	) {
		let ctor: ResourceCtor = Arc::new(move |engine| {
			let resource = ctor(engine);
			let handle = engine.add_resource(resource);

			Box::new(handle)
		});

		let kind = ctor(self).resource_kind_id();

		self.resource_ctors.insert(kind, ctor);
	}

	pub fn add_resource<T>(&mut self, resource: T) -> ResourceHandle<T>
	where
		T: Resource + 'static
	{
		let kind = resource.resource_kind_id();
		let handle = ResourceHandle::new(
			Arc::new(RwLock::new(resource)),
			Arc::default(),
		);

		if let Some(existing) = self.resources.get_mut(kind) {
			existing.push(Box::new(handle.clone()));
		} else {
			self.resources.insert(kind, vec![Box::new(handle.clone())]);			
		}

		handle
	}

	pub fn create_resource(&mut self, kind: &'static str) -> Box<dyn ResourceHandleDyn> {
		let ctor = self.resource_ctors[kind].clone();
		ctor(self)
	}

	pub fn get_resources_by_kind(&self, kind: &'static str)
		-> impl Iterator<Item = &Box<dyn ResourceHandleDyn>>
	{
		if let Some(resources) = self.resources.get(kind) {
			resources.iter()
		} else {
			[].iter()
		}
	}
}

impl Config {
	pub fn secs_per_beat(&self) -> f64 {
		1.0 / self.beats_per_sec()
	}
	
	pub fn beats_per_sec(&self) -> f64 {
		self.bpm / 60.0
	}

	pub fn tl_units_to_frames(&self, timeline_unit: Step) -> usize {
		let beat = timeline_unit.0 as f64 / BEAT_DIVISIONS as f64;
		(beat * self.secs_per_beat() * self.sample_rate as f64) as usize
	}

	pub fn frames_to_tl_units(&self, frames: usize) -> Step {
		let beat = frames as f64 / self.sample_rate as f64 / self.secs_per_beat();
		Step((beat * BEAT_DIVISIONS as f64) as usize)
	}
}