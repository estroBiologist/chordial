use crate::{engine::{Config, Engine, Frame}, node::NodeUtil, param::{ParamKind, ParamValue, Parameter}, util::db_to_factor};

use super::{Buffer, BufferAccess, BusKind, Node, NodeInstance};


pub trait Effect: Send {
	fn render_effect(&self, buffer: BufferAccess);
	fn advance_effect(&mut self, frames: usize, config: &Config);

	#[allow(unused_variables)]
	fn param_updated(&mut self, param: usize, value: &ParamValue) { }

	#[allow(unused_variables)]
	fn get_param_default_value(&self, param: usize) -> Option<ParamValue> { None }

	fn get_params(&self) -> &[Parameter] { &[] }

	fn get_name(&self) -> &'static str;
}


impl<T: Effect + 'static> Node for T {
	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_input_names(&self) -> &'static [&'static str] {
		&["in"]
	}

	fn get_output_names(&self) -> &'static [&'static str] {
		&["out"]
	}

	fn get_name(&self) -> &'static str {
		Effect::get_name(self)
	}

	fn advance(&mut self, frames: usize, config: &Config) {
		self.advance_effect(frames, config);
	}
	
	fn render(&self, _: usize, mut buffer: BufferAccess, instance: &NodeInstance, engine: &Engine) {
		self.poll_input_into_buffer(0, &mut buffer, instance, engine);
		self.render_effect(buffer);
	}

	fn get_param_default_value(&self, param: usize) -> Option<ParamValue> {
		Effect::get_param_default_value(self, param)
	}

	fn get_params(&self) -> &[Parameter] {
		Effect::get_params(self)
	}

	fn param_updated(&mut self, param: usize, value: &ParamValue) {
		Effect::param_updated(self, param, value)
	}
}


pub struct Gain {
	pub gain: f32,
}

impl Effect for Gain {
	fn render_effect(&self, mut buffer: BufferAccess) {
		let buffer = buffer.audio_mut().unwrap();
		let fac = db_to_factor(self.gain);
		
		buffer
			.iter_mut()
			.for_each(|Frame([l, r])| {
				*l *= fac;
				*r *= fac;
			})
	}

	fn advance_effect(&mut self, _: usize, _: &Config) { }

	fn get_params(&self) -> &[Parameter] {
		&[
			Parameter {
				kind: ParamKind::Float,
				text: "gain",
			}
		]
	}

	fn param_updated(&mut self, _: usize, value: &ParamValue) {
		let ParamValue::Float(val) = value else {
			panic!()
		};

		self.gain = *val as f32;
	}

	fn get_name(&self) -> &'static str {
		"Gain"
	}
}

pub struct Amplify;

impl Node for Amplify {
	fn get_name(&self) -> &'static str {
		"Amplify"
	}

	fn render(
		&self,
		_output: usize,
		mut buffer: BufferAccess,
		instance: &NodeInstance,
		engine: &Engine
	) {
		self.poll_input_into_buffer(0, &mut buffer, instance, engine);

		let Some(amp_buf) = self.poll_input(1, buffer.len(), instance, engine) else {
			return
		};

		let audio = buffer.audio_mut().unwrap();

		let Buffer::Control(amp) = &*amp_buf else {
			panic!()
		};

		audio
			.iter_mut()
			.zip(amp.iter().copied())
			.for_each(|(a, b)| {
				a.0[0] *= b;
				a.0[1] *= b;
			})
	}

	fn get_inputs(&self) -> &[BusKind] {
		&[BusKind::Audio, BusKind::Control]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Audio]
	}

	fn get_input_names(&self) -> &'static [&'static str] {
		&["in", "amp"]
	}

	fn get_output_names(&self) -> &'static [&'static str] {
		&["out"]
	}
}
