use std::{collections::{BTreeMap, HashMap}, fmt::{Debug, Write}, fs::File, io::{self, BufRead, BufReader, Read, Write as IoWrite}, ops::{Add, AddAssign, Mul, Sub}, path::{Path, PathBuf}, sync::{Arc, RwLock, RwLockReadGuard}, time::Instant};

use crate::{midi::MidiBlock, node::{effect::{Amplify, Gain}, io::{MidiSplit, Sink}, osc::{Osc, PolyOsc, Sine}, sampler::Sampler, timeline::MidiClip, Buffer, BufferAccess, BusKind, ControlValue, Envelope, Node, NodeInstance, OutputRef, TlUnit, Trigger}, param::ParamValue, resource::{Resource, ResourceHandle, ResourceHandleDyn, ResourceLoader, WavLoader}};


pub const STEP_DIVISIONS: u32 = 24;
pub const BEAT_DIVISIONS: u32 = 4;

#[derive(Copy, Clone)]
pub struct Frame(pub f32, pub f32);

impl Debug for Frame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("({:?})", self.0))
	}
}

impl AddAssign for Frame {
	fn add_assign(&mut self, rhs: Self) {
		self.0 += rhs.0;
		self.1 += rhs.1;
	}
}

impl Add for Frame {
	type Output = Frame;

	fn add(self, rhs: Self) -> Self::Output {
		Frame(self.0 + rhs.0, self.1 + rhs.1)
	}
}

impl Sub for Frame {
	type Output = Frame;

	fn sub(self, rhs: Self) -> Self::Output {
		Frame(self.0 - rhs.0, self.1 - rhs.1)
	}
}

impl Mul<f32> for Frame {
	type Output = Frame;

	fn mul(self, rhs: f32) -> Self::Output {
		Frame(self.0 * rhs, self.1 * rhs)
	}
}

impl Frame {
	pub const ZERO: Frame = Frame(0f32, 0f32);
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
pub type ResourceCtor = Arc<dyn Fn(&mut Engine, usize) -> Box<dyn ResourceHandleDyn> + Send + Sync>;
pub type ResourceLoadCtor = Arc<dyn Fn(&Path, &mut Engine, usize) -> Option<Box<dyn ResourceHandleDyn>> + Send + Sync>;

pub struct Engine {
	pub config: Config,
	pub playing: bool,
	
	nodes: BTreeMap<usize, NodeInstance>,
	node_ctors: HashMap<&'static str, NodeCtor>,
	node_counter: usize,

	resources_by_kind: HashMap<&'static str, Vec<Box<dyn ResourceHandleDyn>>>,
	resources: HashMap<usize, Box<dyn ResourceHandleDyn>>,
	resource_ctors: HashMap<&'static str, ResourceCtor>,
	resource_counter: usize,

	resource_loaders: HashMap<&'static str, ResourceLoadCtor>,

	position: usize,
	
	pub rendering_offline: bool,
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
			
			resources_by_kind: HashMap::new(),
			resources: HashMap::new(),
			resource_ctors: HashMap::new(),
			resource_counter: 0,

			resource_loaders: HashMap::new(),

			position: 0,

			rendering_offline: false,
			enable_buffer_readback: false,
			buffer_readback: vec![],
			dbg_buffer_size: 0u32,
			dbg_buffer_time: 0f32,
			dbg_process_time: 0f32,
		};

		engine.register_resource(|_| MidiBlock::default());
		
		engine.register_resource_loader(WavLoader);

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
		engine.register_node("chordial.midi_clip", |_| Box::new(MidiClip::new(ResourceHandle::nil("MidiBlock"))));
		engine.register_node("chordial.sampler", |_| Box::new(Sampler::new()));

		engine.create_node("chordial.sink");
		engine
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
		while self.nodes.contains_key(&self.node_counter) {
			self.node_counter += 1;
		}
		self.nodes.insert(self.node_counter, node);
	}

    pub fn add_node(&mut self, node: impl Node + 'static, id: &'static str) -> usize {
		while self.nodes.contains_key(&self.node_counter) {
			self.node_counter += 1;
		}
        self.nodes.insert(self.node_counter, NodeInstance::new(node, id));
		self.node_counter
    }

	pub fn add_node_dyn(&mut self, node: Box<dyn Node + 'static>, id: &'static str) -> usize {
		while self.nodes.contains_key(&self.node_counter) {
			self.node_counter += 1;
		}
		self.nodes.insert(self.node_counter, NodeInstance::new_dyn(node, id));
		self.node_counter
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

	pub fn resources(&self) -> impl Iterator<Item = (&usize, &Box<dyn ResourceHandleDyn>)> {
		self.resources.iter()
	}

	pub fn resources_mut(&mut self) -> impl Iterator<Item = (&usize, &mut Box<dyn ResourceHandleDyn>)> {
		self.resources.iter_mut()
	}

	pub fn poll_node_output<'access>(
		&'access self,
		output_ref: &OutputRef,
		buffer_len: usize
	) -> RwLockReadGuard<'access, Buffer> {
		let input_node = self.get_node(output_ref.node).unwrap();

		// Optimization: don't render Timeline Nodes outside their timeline span
		// unless explicitly requested by the node
		if input_node.is_timeline_node() && !input_node.node.process_outside_timeline_span() {
			let tl_pos = self.config.frames_to_tl_units(self.position);
			let buffer_len_tl = self.config.frames_to_tl_units(buffer_len);

			let node_end = input_node.get_timeline_position().0
				+ input_node.node.get_timeline_length(&self.config).0
				- input_node.get_timeline_start_offset().0
				- input_node.get_timeline_end_offset().0;
			
			if tl_pos + buffer_len_tl < input_node.get_timeline_position() || tl_pos > TlUnit(node_end) {
				return input_node.outputs[output_ref.output].read().unwrap()
			}
			
		}
		
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
			writeln!(result, "  id:\t{}", node.1.ctor).unwrap();
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

			for name in node.1.node.get_resource_names() {
				let resource = node.1.node.get_resource(name);
				
				if resource.is_empty() {
					writeln!(result, "  resource {name}: (unlinked)").unwrap();
				} else {
					writeln!(result, "  resource {name}: {}", resource.id()).unwrap();
				}
			}

			for (meta, val) in node.1.metadata() {
				writeln!(result, "  meta {meta}: {val}").unwrap();
			}
		}

		result
	}

	pub fn register_resource_loader(
		&mut self,
		loader: impl ResourceLoader + 'static
	) {
		let extensions = loader.extensions();

		for ext in extensions {
			let loader = loader.clone();

			self.resource_loaders.insert(
				ext,
				Arc::new(move |path, engine, id| {
					let resource = loader.load_resource(path)?;
					let handle = engine.add_resource_with_id(resource, id);
				
					Some(Box::new(handle))
				}
			));
		}
	}
	
	pub fn register_resource<T: Resource + 'static>(
		&mut self,
		ctor: impl Fn(&mut Engine) -> T + Send + Sync + 'static,
	) {
		let kind = ctor(self).resource_kind();

		assert!(!kind.contains(['\n', '\r', ' ', '\t']), "whitespace not allowed in resource_kind!");

		let ctor: ResourceCtor = Arc::new(move |engine, id| {
			let resource = ctor(engine);
			let handle = engine.add_resource_with_id(resource, id);

			Box::new(handle)
		});

		self.resource_ctors.insert(kind, ctor);
	}

	pub fn add_resource<T>(&mut self, resource: T) -> ResourceHandle<T>
	where
		T: Resource + 'static
	{
		let id = self.get_next_resource_id();
		self.add_resource_with_id(resource, id)
	}

	pub fn add_resource_with_id<T>(&mut self, resource: T, id: usize) -> ResourceHandle<T>
	where
		T: Resource + 'static
	{
		let kind = resource.resource_kind();
		let handle = ResourceHandle::new(resource, None, id);
		
		self.resources.insert(id, Box::new(handle.clone()));

		if let Some(existing) = self.resources_by_kind.get_mut(kind) {
			existing.push(Box::new(handle.clone()));
		} else {
			self.resources_by_kind.insert(kind, vec![Box::new(handle.clone())]);			
		}

		handle
	}
	
	pub fn create_resource(&mut self, kind: &str) -> Box<dyn ResourceHandleDyn> {
		let id = self.get_next_resource_id();

		self.create_resource_with_id(kind, id)
	}

	pub fn create_resource_with_id(&mut self, kind: &str, id: usize) -> Box<dyn ResourceHandleDyn> {
		let ctor = self.resource_ctors[kind].clone();
		let resource = ctor(self, id);
		
		resource
	}

	pub fn load_resource(&mut self, path: &Path) -> Option<Box<dyn ResourceHandleDyn>> {
		let id = self.get_next_resource_id();

		self.load_resource_with_id(path, id)
	}

	pub fn load_resource_with_id(&mut self, path: &Path, id: usize) -> Option<Box<dyn ResourceHandleDyn>> {
		let ext = path.extension()?.to_str()?;
		let loader = self.resource_loaders.get(ext)?.clone();

		loader(path, self, id)
	}

	pub fn get_resources_by_kind(&self, kind: &str)
		-> impl Iterator<Item = &Box<dyn ResourceHandleDyn>>
	{
		if let Some(resources) = self.resources_by_kind.get(kind) {
			resources.iter()
		} else {
			[].iter()
		}
	}
	
	pub fn get_resource_count_by_kind(&self, kind: &str) -> usize {
		if let Some(resources) = self.resources_by_kind.get(kind) {
			resources.len()
		} else {
			0
		}
	}

	pub fn get_resource_by_kind(&self, kind: &str, idx: usize) -> Option<&Box<dyn ResourceHandleDyn>> {
		if let Some(resources) = self.resources_by_kind.get(kind) {
			resources.get(idx)
		} else {
			None
		}
	}

	pub fn get_resource_by_id(&self, id: usize) -> Option<&Box<dyn ResourceHandleDyn>> {
		self.resources.get(&id)
	}

	pub fn make_resource_unique(&mut self, id: usize) {
		todo!()
	}

	pub fn link_resource(&self, node: usize, resource: &str, id: usize) {
		let linked = &**self.resources.get(&id).unwrap();

		self
			.get_node(node)
			.unwrap()
			.node
			.get_resource(resource)
			.link_dyn(linked.as_any());
	}

	// TODO: Reuse purged IDs like node counter does
	fn get_next_resource_id(&mut self) -> usize {
		while self.resources.contains_key(&self.resource_counter) {
			self.resource_counter += 1;
		}
		self.resource_counter
	}

	pub fn save(&self, f: &mut File) -> io::Result<()> {
		for (idx, resource) in self.resources() {
			let kind = resource.resource_kind();

			if resource.is_external() {
				writeln!(f, "res {idx} {kind} external {:?}", resource.path().unwrap())?;
			} else {
				let data = resource.save();

				writeln!(f, "res {idx} {kind} internal {}", data.len())?;

				f.write_all(&data)?;
				
				writeln!(f)?;
			}

			writeln!(f)?;
		}

		for (idx, node) in self.nodes() {
			write!(f, "node {idx} {}\n", node.ctor)?;
			
			for input in &node.inputs {
				write!(f, "in")?;

				for input_node in &input.0 {
					write!(f, " {}.{}", input_node.node, input_node.output)?;
				}

				write!(f, "\n")?;
			}

			for (_, value) in node.get_params() {
				writeln!(f, "param {value}")?;
			}

			for res in node.node.get_resource_names() {
				if node.node.get_resource(res).is_empty() {
					writeln!(f, "r {res}")?;
				} else {
					writeln!(f, "r {res} {}", node.node.get_resource(res).id())?;
				}
			}

			for (meta, val) in node.metadata() {
				writeln!(f, "meta {meta} {val}")?;
			}

			writeln!(f)?;
		}

		Ok(())
	}

	pub fn load(&mut self, path: &Path) {
		self.nodes.clear();
		self.node_counter = 0;
		self.resources.clear();
		self.resources_by_kind.clear();
		self.resource_counter = 0;

		let file = File::open(path).unwrap();
		let mut reader = BufReader::new(file);
		let mut buf = vec![];
		
		let mut last_read = reader.read_until(b'\n', &mut buf).unwrap();
		 
		while last_read != 0 {
			let line = String::from_utf8(buf).unwrap();
			let line = line.trim();
			buf = vec![];
			
			// skip comment lines
			if let Some(';') = line.chars().next() {
				last_read = reader.read_until(b'\n', &mut buf).unwrap();
				continue
			}

			// skip empty lines
			if line.is_empty() {
				last_read = reader.read_until(b'\n', &mut buf).unwrap();
				continue
			}

			let (t, line) = line.split_at(line.find(' ').unwrap());
			let line = &line[1..];

			match t {
				"res" => {
					let line = line.trim();
					let (id,      line) = line.split_at(line.find(" ").unwrap());
					let line = line.trim();
					let (kind,    line) = line.split_at(line.find(" ").unwrap());
					let line = line.trim();
					let (storage, line) = line.split_at(line.find(" ").unwrap());
					let line = line.trim();
					
					let id = id.trim().parse::<usize>().unwrap();
					let kind = kind.trim();
					
					match storage {
						"internal" => {
							let size = line.parse::<usize>().unwrap();
							let mut data = vec![0; size];

							let mut resource = self.create_resource_with_id(kind, id);

							reader.read_exact(&mut data).unwrap();
							resource.load(&data);
						}

						"external" => {
							self.load_resource_with_id(&PathBuf::from(line), id);
						}

						other => panic!("invalid storage specifier: {other}")
					}
					
				}

				"node" => {
					let (idx, name) = line.split_at(line.find(" ").unwrap());

					let name = name.trim();
					let idx = idx.parse::<usize>().unwrap();
		
					let Some(ctor) = self.node_ctors.get(name) else {
						panic!("unknown node constructor `{name}`")
					};
		
					let node = ctor.clone()(self);
		
					let (id, _) = self.node_ctors.get_key_value(name).unwrap();
					let mut node = NodeInstance::new_dyn(node, id);
					
					node.inputs.clear();
					
					last_read = reader.read_until(b'\n', &mut buf).unwrap();
		
					let mut param_counter = 0;
		
					// parse inputs and parameters
					while last_read != 0 {
						let line_raw = String::from_utf8(buf).unwrap();
						let line = line_raw.trim();
						buf = vec![];
						
						// skip empty lines
						if line.is_empty() {
							last_read = reader.read_until(b'\n', &mut buf).unwrap();
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
						
						} else if line.starts_with("r ") {
							let line = line[2..].trim();

							if let Some(split) = line.find(' ') {
								let (resource, id) = line.split_at(split);
								let linked = self.get_resource_by_id(id.trim().parse().unwrap()).unwrap();

								node.node.get_resource(resource).link_dyn(linked.as_any());
							}

						} else if line.starts_with("meta ") {
							let line = line[5..].trim();
							let (key, val) = line.split_at(line.find(" ").unwrap());

							node.set_metadata(key.trim().to_string(), ParamValue::parse(val.trim()));
						} else {
							buf = line_raw.into_bytes();
							break
						}
						
						last_read = reader.read_until(b'\n', &mut buf).unwrap();
					}
		
					self.nodes.insert(idx, node);
				}

				other => panic!("unrecognnized file element: {other}"),
			}
			
		}

		while self.nodes.contains_key(&self.node_counter) {
			self.node_counter += 1;
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

	pub fn tl_units_to_frames(&self, timeline_unit: TlUnit) -> usize {
		let beat = timeline_unit.0 as f64 / (STEP_DIVISIONS * BEAT_DIVISIONS) as f64;
		(beat * self.secs_per_beat() * self.sample_rate as f64) as usize
	}

	pub fn frames_to_tl_units(&self, frames: usize) -> TlUnit {
		let beat = frames as f64 / self.sample_rate as f64 / self.secs_per_beat();
		TlUnit((beat * (STEP_DIVISIONS * BEAT_DIVISIONS) as f64) as usize)
	}
}