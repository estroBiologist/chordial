use std::{path::Path, sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard}};

use crate::{engine::Frame, param::ParamValue};


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


pub trait ResourceAccess {
	fn apply_action(&self, action: &'static str, args: &[ParamValue]);

	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue>;

	fn get_action_list(&self) -> &'static [&'static str];
	
	fn resource_kind_id(&self) -> &'static str;
}


pub struct ResourceBuffer<T: Resource + Send + Sync> {
	data: Arc<RwLock<T>>,
	path: Option<Arc<Path>>,
}

impl<T> ResourceBuffer<T>
where 
	T: Resource + Send + Sync
{
	pub fn read(&self) -> RwLockReadGuard<T> {
		self.data.read().unwrap()
	}

	pub fn write(&self) -> RwLockWriteGuard<T> {
		self.data.write().unwrap()
	}

	pub fn path(&self) -> Option<&Path> {
		if let Some(path) = &self.path {
			Some(path.as_ref())
		} else {
			None
		}
	}
}


impl<T> ResourceAccess for ResourceBuffer<T>
where
	T: Resource + Send + Sync
{
	fn apply_action(&self, action: &'static str, args: &[ParamValue]) {
		self.write().apply_action(action, args)
	}

	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> {
		self.read().get(keys)
	}
	
	fn resource_kind_id(&self) -> &'static str {
		self.read().resource_kind_id()
	}

	fn get_action_list(&self) -> &'static [&'static str] {
		self.read().get_action_list()
	}
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
