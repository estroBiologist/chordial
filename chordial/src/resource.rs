use std::{path::PathBuf, sync::{Arc, Mutex, MutexGuard, RwLock}};

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

	fn is_empty(&self) -> bool;

	fn id(&self) -> usize;

}


#[derive(Clone)]
pub struct ResourceData<T: Resource> {
	pub data: T,
	pub path: Option<PathBuf>,
	pub id: usize,
}


type ResourceHandleInner<T> = Option<Arc<RwLock<ResourceData<T>>>>;

pub struct ResourceHandle<T: Resource> {
	inner: Mutex<ResourceHandleInner<T>>,
	kind: &'static str,
}

impl<T: Resource> Clone for ResourceHandle<T> {
	fn clone(&self) -> Self {
		ResourceHandle {
			inner: Mutex::new(self.inner.lock().unwrap().clone()),
			kind: self.kind,
		}
	}
}


impl<T: Resource> ResourceHandle<T> {

	// Non-empty ResourceHandles can only be given out by the engine,
	// use Engine::add_resource() or Engine::create_resource() instead
	pub(crate) fn new(data: T, path: Option<PathBuf>, id: usize) -> Self {
		let kind = data.resource_kind_id();
		ResourceHandle {
			inner: Mutex::new(Some(Arc::new(RwLock::new(ResourceData {
					data,
					path,
					id
				})))
			),
			kind
		}
	}

	pub fn nil(kind: &'static str) -> Self {
		ResourceHandle {
			inner: Mutex::default(),
			kind,
		}
	}

	pub fn is_empty(&self) -> bool {
		self.inner().is_none()
	}

	pub fn path(&self) -> Option<PathBuf> {
		if let Some(path) = &self.inner().as_ref().unwrap().read().unwrap().path {
			Some(path.clone())
		} else {
			None
		}
	}

	pub fn is_external(&self) -> bool {
		self.path().is_some()
	}

	pub fn detach_from_external(&self) {
		self.inner().as_ref().unwrap().write().unwrap().path = None;
	}

	pub fn inner(&self) -> MutexGuard<ResourceHandleInner<T>> {
		self.inner.lock().unwrap()
	}
}


impl<T: Resource> ResourceHandleSealed for ResourceHandle<T> {}


impl<T: Resource> ResourceHandleDyn for ResourceHandle<T> {

	fn apply_action(&self, action: &'static str, args: &[ParamValue]) {
		self.inner().as_ref().unwrap().write().unwrap().data.apply_action(action, args)
	}

	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> {
		self.inner().as_ref().unwrap().read().unwrap().data.get(keys)
	}
	
	fn resource_kind_id(&self) -> &'static str {
		self.kind
	}

	fn id(&self) -> usize {
		self.inner().as_ref().unwrap().read().unwrap().id
	}

	fn is_empty(&self) -> bool {
		self.inner().is_none()
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
