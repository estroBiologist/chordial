use std::{collections::HashMap, fmt::Debug};

use crate::node::{NodeInstance, Sink, Buffer, Node, TimelineUnit, BEAT_DIVISIONS};


pub struct Config {
	pub sample_rate: usize,
	pub bpm: f64,
}

pub struct Engine {
	pub config: Config,
	nodes: HashMap<usize, NodeInstance>,
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
	pub fn new() -> Self {
		let mut engine = Engine { 
			nodes: HashMap::new(),
			node_counter: 0,
			config: Config {
				sample_rate: 44100,
				bpm: 120.0,
			}
		};

		engine.add_node(Sink);
		engine
	}

	pub fn render(&mut self, buffer: &mut [Frame]) {
		let sink = &self.nodes[&0];
		sink.render(0, Buffer::Audio(buffer), self);
		
		for node in self.nodes.values_mut() {
			node.node.advance(buffer.len(), &self.config);
		}
	}

	pub fn add_node_instance(&mut self, node: NodeInstance) {
		self.nodes.insert(self.node_counter, node);
		self.node_counter += 1;
	}

    pub fn add_node(&mut self, node: impl Node + Send + 'static) {
        self.nodes.insert(self.node_counter, NodeInstance::new(node));
        self.node_counter += 1;
    }

	pub fn get_node(&self, node: usize) -> Option<&NodeInstance> {
		self.nodes.get(&node)
	}

	pub fn get_node_mut(&mut self, node: usize) -> Option<&mut NodeInstance> {
		self.nodes.get_mut(&node)
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