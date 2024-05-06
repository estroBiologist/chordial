use std::collections::BTreeMap;

use crate::node::NodeInstance;


pub struct Module {
	nodes: BTreeMap<usize, NodeInstance>,
	node_counter: usize,
}