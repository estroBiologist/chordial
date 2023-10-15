use std::{fs::File, path::Path, sync::{RwLock, Arc}, time::Duration};

use engine::{Engine, Frame};
use node::{OutputRef, Sine};

use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig, SampleRate};
use wav::{Header, WAV_FORMAT_IEEE_FLOAT, BitDepth};

use crate::node::{Gain, Trigger, TimelineUnit, TimelineNode};

pub mod engine;
pub mod midi;
pub mod node;
mod util;

fn main() {
	println!("chordial audio engine - proof of concept");
	
	let host = cpal::default_host();
	let device = host.default_output_device().expect("no default output device available!");
	let mut out = File::create(Path::new("./output.wav")).unwrap();
	let out_buffer = Arc::new(RwLock::new(vec![]));
	let out_buffer_thread = out_buffer.clone();

	println!("using output device `{}`", device.name().unwrap_or("(could not get device name)".to_string()));

	let config = StreamConfig {
		channels: 2,
		sample_rate: SampleRate(44100),
		buffer_size: cpal::BufferSize::Default,
	};

	let mut engine = Engine::new();

	engine.add_node(Sine::new(440.0));
	engine.add_node(Gain { gain: -10.0 });
	engine.add_node(Trigger { 
		node_pos: TimelineUnit(24*4),
		tl_pos: 0,
	});

	//let trigger = engine.get_node(3).unwrap();
	
	let gain = engine.get_node_mut(2).unwrap();

	gain.inputs[0] = Some(OutputRef {
		node: 1,
		output: 0,
	});

	let sink = engine.get_node_mut(0).unwrap();
	
	sink.inputs[0] = Some(OutputRef {
		node: 2,
		output: 0,
	});
	
	let mut buffer = vec![];
	
	let stream = device.build_output_stream(
		&config,

		move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
			buffer.resize(data.len() / 2, Frame([0f32; 2]));
			engine.render(&mut buffer);
			
			let mut out_buffer = out_buffer_thread.write().unwrap();

			for (i, frame) in buffer.iter().enumerate() {
				data[i*2] = frame.0[0];
				data[i*2+1] = frame.0[1];
			}

			out_buffer.extend_from_slice(data);

			buffer.fill(Frame([0f32; 2]));
		},

		move |_| {
			todo!()
		},

		None
	).unwrap();

	stream.play().unwrap();
	std::thread::sleep(Duration::from_secs(5));
	stream.pause().unwrap();

	wav::write(
		Header::new(
			WAV_FORMAT_IEEE_FLOAT,
			2,
			44100,
			32,
		), 
		&BitDepth::ThirtyTwoFloat(out_buffer.write().unwrap().drain(..).collect()), 
		&mut out
	).unwrap();
}
