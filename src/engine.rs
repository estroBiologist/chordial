use std::{collections::{HashMap, BTreeMap}, fmt::Debug, sync::RwLockReadGuard, path::Path};

use crate::{node::{NodeInstance, Sink, Node, TimelineUnit, BEAT_DIVISIONS, BufferAccess, Buffer, OutputRef}, param::ParamValue};


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

pub type NodeConstructor = Box<dyn Fn() -> Box<dyn Node> + Send>;

pub struct Engine {
	pub config: Config,
	nodes: BTreeMap<usize, NodeInstance>,
	constructors: HashMap<&'static str, NodeConstructor>,
	node_counter: usize,
}

#[derive(Copy, Clone)]
pub struct Frame(pub [f32; 2]);

impl Debug for Frame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("({:?})", self.0))
	}
}

impl Engine {
	pub fn new(sample_rate: u32) -> Self {
		let mut engine = Engine { 
			nodes: BTreeMap::new(),
			constructors: HashMap::new(),
			node_counter: 0,
			config: Config {
				sample_rate,
				bpm: 120.0,
				tuning: 440.0,
			}
		};

		engine.register("chordial.sink", || Box::new(Sink));
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
			let (idx, name) = line.split_at(line.find(" ").unwrap());

			let name = &name[1..];
			let idx = idx.parse::<usize>().unwrap();

			let Some((id, ctor)) = self.constructors.get_key_value(name) else {
				panic!("unknown node constructor `{name}`");
			};

			let mut node = NodeInstance::new_dyn(ctor(), id);
			
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
					let inputs = line[3..].split(".").collect::<Vec<_>>();
					let [noderef, output] = inputs.as_slice() else {
						panic!()
					};

					node.inputs.push(Some(OutputRef {
						node: noderef.parse().unwrap(),
						output: output.parse().unwrap(),
					}));
				} else if line == "in" {
					node.inputs.push(None);
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

	pub fn register(
		&mut self, 
		name: &'static str, 
		ctor: impl Fn() -> Box<dyn Node> + Send + 'static
	) {
		if self.constructors.contains_key(name) {
			panic!("constructor `{name}` already registered!")
		}

		self.constructors.insert(name, Box::new(ctor));
	}

	pub fn render(&mut self, buffer: &mut [Frame]) {
		let sink = &self.nodes[&0];

		sink.node.render(0, BufferAccess::Audio(buffer), sink, self);
		
		for node in self.nodes.values_mut() {
			node.node.advance(buffer.len(), &self.config);
			node.clear_buffers();
		}
	}

	pub fn seek(&mut self, position: usize) {
		for node in &mut self.nodes {
			node.1.node.seek(position, &self.config)
		}
	}

	pub fn create_node(&mut self, name: &str) {
		let Some((id, ctor)) = self.constructors.get_key_value(name) else {
			panic!("unknown node constructor `{name}`!");
		};

		self.add_node_dyn(ctor(), id);
	}

	pub fn add_node_instance(&mut self, node: NodeInstance) {
		self.nodes.insert(self.node_counter, node);
		self.node_counter += 1;
	}

    pub fn add_node(&mut self, node: impl Node + 'static, id: &'static str) {
        self.nodes.insert(self.node_counter, NodeInstance::new(node, id));
        self.node_counter += 1;
    }

	pub fn add_node_dyn(&mut self, node: Box<dyn Node + 'static>, id: &'static str) {
		self.nodes.insert(self.node_counter, NodeInstance::new_dyn(node, id));
		self.node_counter += 1;
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

	pub fn nodes(&self) -> impl Iterator<Item = (&usize, &NodeInstance)> {
		self.nodes.iter()
	}

	pub fn nodes_mut(&mut self) -> impl Iterator<Item = (&usize, &mut NodeInstance)> {
		self.nodes.iter_mut()
	}

	pub fn poll_node_output(&self, output_ref: &OutputRef, buffer_len: usize) -> RwLockReadGuard<'_, Buffer> {
		let input_node = self.get_node(output_ref.node).unwrap();
		
		input_node.render(output_ref.output, buffer_len, self);

		input_node.outputs[output_ref.output].read().unwrap()
	}
}

impl Config {
	pub fn secs_per_beat(&self) -> f64 {
		1.0 / self.beats_per_sec()
	}
	
	pub fn beats_per_sec(&self) -> f64 {
		self.bpm / 60.0
	}

	pub fn tl_units_to_frames(&self, timeline_unit: TimelineUnit) -> usize {
		let beat = timeline_unit.0 as f64 / BEAT_DIVISIONS as f64;
		(beat * self.secs_per_beat() * self.sample_rate as f64) as usize
	}
}