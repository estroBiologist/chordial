use std::{fs::File, path::{Path, PathBuf}, sync::{RwLock, Arc, Mutex}, time::Duration, io::Write};

use engine::{Engine, Frame};

use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig, SampleRate, SupportedBufferSize};
use wav::{Header, WAV_FORMAT_IEEE_FLOAT, BitDepth};

use crate::node::{Gain, Sine, Trigger, TimelineUnit};

pub mod adsr;
pub mod engine;
pub mod format;
pub mod midi;
pub mod node;
pub mod param;
mod util;

fn main() {
	println!("chordial audio engine - proof of concept");
	
	let host = cpal::default_host();
	let device = host.default_output_device().expect("no default output device available!");
	let mut out = File::create(Path::new("./output.wav")).unwrap();
	let out_buffer = Arc::new(RwLock::new(vec![]));
	let out_buffer_thread = out_buffer.clone();

	println!("using output device `{}`", device.name().unwrap_or("(could not get device name)".to_string()));
	
	println!("\nsupported configurations:\n");
	
	for config in device.supported_output_configs().unwrap() {
		println!("  sample rate range: ({} - {})", 
			config.min_sample_rate().0,
			config.max_sample_rate().0,
		);
		
		match config.buffer_size() {
			SupportedBufferSize::Range { min, max } => {
				println!("  buffer size: ({} - {})", min, max);
			}
			SupportedBufferSize::Unknown => {
				println!("  buffer size: unknown");
			}
		}

		println!("  channels: {}", config.channels());
		println!();
	}

	let config = StreamConfig {
		channels: 2,
		sample_rate: SampleRate(48000),
		buffer_size: cpal::BufferSize::Default,
	};

	let mut engine = Engine::new(config.sample_rate.0);

	register_builtin_nodes(&mut engine);
	engine.load_from_file(&PathBuf::from("state.chrp"));

	let mut state_file = File::create("state.chrp").unwrap();
	write!(state_file, "{engine}").unwrap();

	let engine = Arc::new(Mutex::new(engine));
	let thread_engine = engine.clone();

	let mut buffer = vec![];
	
	let stream = device.build_output_stream(
		&config,

		move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
			buffer.resize(data.len() / 2, Frame([0f32; 2]));
			thread_engine.lock().unwrap().render(&mut buffer);
			
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

	println!("
stream opened with config:
  channels: {}
  sample rate: {}
  buffer size: {:?}",
		config.channels,
		config.sample_rate.0,
		config.buffer_size
	);

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

fn register_builtin_nodes(engine: &mut Engine) {
	engine.register("chordial.sine", || {
		Box::new(Sine::new(440.0))
	});

	engine.register("chordial.gain", || {
		Box::new(Gain { gain: 0.0 })
	});
	
	engine.register("chordial.trigger", || {
		Box::new(Trigger { 
			node_pos: TimelineUnit(96),
			tl_pos: 0
		})
	});
}