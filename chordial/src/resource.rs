use std::{path::Path, sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard}};

use crate::{engine::Frame, param::ParamValue};

type BoxedResource = Box<dyn Resource + Send + Sync>;

pub struct ResourceBuffer {
	data: Arc<RwLock<BoxedResource>>,
	path: Option<Arc<Path>>,
}

impl ResourceBuffer
{
	pub fn read(&self) -> RwLockReadGuard<BoxedResource> {
		self.data.read().unwrap()
	}

	pub fn write(&self) -> RwLockWriteGuard<BoxedResource> {
		self.data.write().unwrap()
	}

	pub fn path(&self) -> Option<&Path> {
		if let Some(path) = &self.path {
			Some(path.as_ref())
		} else {
			None
		}
	}

	pub fn apply_action(&self, action: &'static str, args: &[ParamValue]) {
		self.write().apply_action(action, args)
	}

	pub fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> {
		self.read().get(keys)
	}
}

pub trait Resource {
	fn resource_kind_id(&self) -> &'static str;

	fn get_action_list(&self) -> &'static [&'static str] {
		&[]
	}

	#[allow(unused_variables)]
	fn apply_action(&mut self, action: &'static str, args: &[ParamValue]) { }

	#[allow(unused_variables)]
	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> { None }
}


pub struct AudioData {
	pub data: Vec<Frame>,
	pub sample_rate: u32,
}

impl Resource for AudioData {
	fn resource_kind_id(&self) -> &'static str {
		"AudioData"
	}
} 

