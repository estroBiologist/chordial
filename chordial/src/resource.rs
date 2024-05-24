use std::{any::Any, mem::size_of, path::PathBuf, sync::{Arc, Mutex, MutexGuard, RwLock}};

use crate::{engine::Frame, param::ParamValue};

use self::private::ResourceHandleSealed;


mod private {
	pub trait ResourceHandleSealed {}
}


pub trait Resource: Clone + Send + Sync {

	fn resource_kind(&self) -> &'static str;

	#[allow(unused_variables)]
	fn apply_action(&mut self, action: &str, args: &[ParamValue]) { }

	#[allow(unused_variables)]
	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> { None }

	fn save(&self) -> Vec<u8>;

	fn load(&mut self, data: &[u8]);
}


pub trait ResourceLoader {
	fn resource_kind(&self) -> &'static str;

	fn extensions(&self) -> &'static [&'static str];

	fn load_resource(&self) -> Option<Box<dyn ResourceHandleDyn>>;
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


impl<T: Resource + 'static> ResourceHandle<T> {

	// Non-empty ResourceHandles can only be given out by the engine,
	// use Engine::add_resource() or Engine::create_resource() instead
	pub(crate) fn new(data: T, path: Option<PathBuf>, id: usize) -> Self {
		let kind = data.resource_kind();
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

	pub fn link_dyn(&self, resource: &dyn Any) {
		let resource = resource.downcast_ref::<ResourceHandle<T>>();

		*self.inner() = resource.unwrap().inner.lock().unwrap().clone();
	}
}


impl<T: Resource> ResourceHandleSealed for ResourceHandle<T> {}


pub trait ResourceHandleDyn: Send + private::ResourceHandleSealed {

	fn resource_kind(&self) -> &'static str;

	fn apply_action(&self, action: &str, args: &[ParamValue]);
	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue>;
	
	fn id(&self) -> usize;

	fn is_empty(&self) -> bool;

	fn link_dyn(&self, resource: &dyn Any);
	fn as_any(&self) -> &dyn Any;

	fn save(&self) -> Vec<u8>;
	fn load(&mut self, data: &[u8]);

	fn is_external(&self) -> bool;
	fn detach_from_external(&self);
	fn path(&self) -> Option<PathBuf>;
}

impl<T: Resource + 'static> ResourceHandleDyn for ResourceHandle<T> {

	fn apply_action(&self, action: &str, args: &[ParamValue]) {
		self.inner().as_ref().unwrap().write().unwrap().data.apply_action(action, args)
	}

	fn get(&self, keys: &[ParamValue]) -> Option<ParamValue> {
		self.inner().as_ref().unwrap().read().unwrap().data.get(keys)
	}
	
	fn resource_kind(&self) -> &'static str {
		self.kind
	}

	fn id(&self) -> usize {
		self.inner().as_ref().unwrap().read().unwrap().id
	}

	fn is_empty(&self) -> bool {
		self.inner().is_none()
	}

	fn link_dyn(&self, resource: &dyn Any) {
		self.link_dyn(resource);
	}

	fn as_any(&self) -> &dyn Any {
		self
	}

	fn is_external(&self) -> bool {
		self.is_external()
	}

	fn detach_from_external(&self) {
		self.detach_from_external()
	}

	fn save(&self) -> Vec<u8> {
		self.inner().as_ref().unwrap().read().unwrap().data.save()
	}

	fn load(&mut self, data: &[u8]) {
		self.inner().as_ref().unwrap().write().unwrap().data.load(data)
	}

	fn path(&self) -> Option<PathBuf> {
		self.path()
	}
}


#[derive(Clone)]
pub struct AudioData {
	pub data: Vec<Frame>,
	pub sample_rate: u32,
}

impl Resource for AudioData {
	fn resource_kind(&self) -> &'static str {
		"AudioData"
	}

	fn save(&self) -> Vec<u8> {
		let size = self.data.len() * size_of::<Frame>() + 4;
		let mut result = vec![];

		result.reserve(size);
		
		result.extend_from_slice(&self.sample_rate.to_ne_bytes());

		for Frame([l, r]) in self.data.iter() {
			result.extend_from_slice(&l.to_ne_bytes());
			result.extend_from_slice(&r.to_ne_bytes());
		}

		result
	}

	fn load(&mut self, data: &[u8]) {
		self.sample_rate = u32::from_ne_bytes(data[0..4].try_into().unwrap());
		
		let frame_size = size_of::<Frame>();
		let sample_size = size_of::<f32>();

		let data = &data[4..];
		let size = data.len() / frame_size;
		
		self.data.clear();
		self.data.reserve(size);

		for i in 0..size {
			let offset = i * frame_size;

			let l_slice = &data[offset..(offset + sample_size)];
			let r_slice = &data[(offset + sample_size)..(offset + frame_size)];

			self.data.push(Frame([
				f32::from_ne_bytes(l_slice.try_into().unwrap()),
				f32::from_ne_bytes(r_slice.try_into().unwrap())
			]));
		}
	}
} 
