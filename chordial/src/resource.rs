use std::{path::PathBuf, sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard}};

use crate::{engine::Frame, param::ParamValue};

use self::private::ResourceHandleSealed;


mod private {
	pub trait ResourceHandleSealed {}
}


pub trait Resource: Clone + Send + Sync {

	fn resource_kind_id(&self) -> &'static str;

	#[allow(unused_variables)]
	fn apply_action(&mut self, action: &'static str, args: &[ParamValue]) { }

	#[allow(unused_variables)]
	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> { None }

}


pub trait ResourceHandleDyn: Send + private::ResourceHandleSealed {

	fn apply_action(&self, action: &'static str, args: &[ParamValue]);

	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue>;
	
	fn resource_kind_id(&self) -> &'static str;

	fn make_unique(&mut self);

}


#[derive(Clone)]
struct ResourceData<T: Resource> {
	data: Arc<RwLock<T>>,
	path: Arc<RwLock<Option<PathBuf>>>,
}


#[derive(Clone)]
pub struct ResourceHandle<T: Resource> {
	inner: Option<ResourceData<T>>,
}


impl<T: Resource> ResourceHandle<T> {

	// Non-empty ResourceHandles can only be given out by the engine,
	// use Engine::add_resource() or Engine::create_resource() instead
	pub(crate) fn new(data: Arc<RwLock<T>>, path: Arc<RwLock<Option<PathBuf>>>) -> Self {
		ResourceHandle {
			inner: Some(ResourceData {
				data,
				path
				}
			)
		}
	}

	pub fn nil() -> Self {
		ResourceHandle {
			inner: None,
		}
	}

	pub fn is_empty(&self) -> bool {
		self.inner.is_none()
	}

	pub fn data(&self) -> Option<&Arc<RwLock<T>>> {
		if let Some(inner) = &self.inner {
			Some(&inner.data)
		} else {
			None
		}
	}

	pub fn read(&self) -> RwLockReadGuard<T> {
		self.data().unwrap().read().unwrap()
	}

	pub fn write(&self) -> RwLockWriteGuard<T> {
		self.inner.as_ref().unwrap().data.write().unwrap()
	}

	pub fn path(&self) -> Option<PathBuf> {
		if let Some(path) = &*self.inner.as_ref().unwrap().path.read().unwrap() {
			Some(path.clone())
		} else {
			None
		}
	}

	pub fn is_external(&self) -> bool {
		self.path().is_some()
	}

	pub fn detach_from_external(&self) {
		*self.inner.as_ref().unwrap().path.write().unwrap() = None;
	}

}


impl<T: Resource> ResourceHandleSealed for ResourceHandle<T> {}


impl<T: Resource> ResourceHandleDyn for ResourceHandle<T> {

	fn apply_action(&self, action: &'static str, args: &[ParamValue]) {
		self.write().apply_action(action, args)
	}

	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> {
		self.read().get(keys)
	}
	
	fn resource_kind_id(&self) -> &'static str {
		self.read().resource_kind_id()
	}

	fn make_unique(&mut self) {
		let res = Arc::new(RwLock::new(self.read().clone()));
		
		self.inner = Some(ResourceData {
			data: res,
			path: Arc::default()
		});
	}

}


#[derive(Clone)]
pub struct AudioData {
	pub data: Vec<Frame>,
	pub sample_rate: u32,
}

impl Resource for AudioData {
	fn resource_kind_id(&self) -> &'static str {
		"AudioData"
	}
} 
