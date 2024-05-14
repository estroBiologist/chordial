use std::{fs::File, path::{Path, PathBuf}, sync::{mpsc::{self, Receiver}, Arc, Mutex, RwLock}, time::{Duration, Instant}};

use chordial::{engine::{Engine, Frame}, midi::{MidiMessage, MidiStatusByte}, node::{BusKind, Node}, param::{ParamKind, ParamValue, Parameter}};

use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, StreamConfig, SampleRate, SupportedBufferSize};
use midir::{MidiInput, MidiInputConnection};
use wav::{Header, WAV_FORMAT_IEEE_FLOAT, BitDepth};


struct MidiIn {
	connection: Option<MidiInputConnection<()>>,
	port_name: String,
	receiver: Option<Receiver<MidiMessage>>,
}

impl MidiIn {
	fn new() -> Self {
		MidiIn {
			connection: None,
			port_name: String::new(),
			receiver: None,
		}
	}
}

impl Node for MidiIn {
	fn get_name(&self) -> &'static str {
		"MIDI In"
	}

	fn get_inputs(&self) -> &[BusKind] {
		&[]
	}

	fn get_outputs(&self) -> &[BusKind] {
		&[BusKind::Midi]
	}

	fn render(
		&self,
		_output: usize,
		mut buffer: chordial::node::BufferAccess,
		_instance: &chordial::node::NodeInstance,
		_engine: &Engine
	) {
		let Some(receiver) = &self.receiver else {
			return
		};

		let buffer = buffer.midi_mut().unwrap();
		
		while let Ok(msg) = receiver.try_recv() {
			buffer[0].push(msg);
		}
	}

	fn advance(
		&mut self,
		_frames: usize,
		_config: &chordial::engine::Config
	) {}

	fn seek(
		&mut self,
		_position: usize,
		_config: &chordial::engine::Config,
	) { }

	fn get_params(&self) -> &[chordial::param::Parameter] {
		&[Parameter {
			kind: ParamKind::String,
			text: "port",
		}]
	}

	fn get_param_default_value(&self, _param: usize) -> Option<ParamValue> {
		Some(ParamValue::String(String::new()))
	}

	fn param_updated(&mut self, _param: usize, value: &ParamValue) {
		let ParamValue::String(port_name) = value else {
			panic!()
		};

		let midi = MidiInput::new("chordial-cli").unwrap();

		drop(self.connection.take());
		self.port_name = port_name.clone();

		for port in midi.ports() {
			let Ok(name) = midi.port_name(&port) else {
				continue
			};
			
			if &name == port_name {
				let (sender, receiver) = mpsc::channel();

				let result = midi.connect(
					&port, 
					&port_name,
					move |_, msg, _| {
						let mut bytes = [0, 0];

						if msg.len() > 1 {
							bytes[0] = msg[1];
						}

						if msg.len() > 2 {
							bytes[1] = msg[2];
						}

						let midi_message = MidiMessage::new(
							MidiStatusByte(msg[0]),
							bytes
						);

						let _ = sender.send(midi_message);
					},
					()
				);

				if let Ok(result) = result {
					self.connection = Some(result);
					self.receiver = Some(receiver);
				}

				break
			}
		}
	}
}


fn main() {
	println!("chordial audio engine - proof of concept");
	
	let host = cpal::default_host();
	let device = host.default_output_device().expect("no default output device available!");
	let mut out = File::create(Path::new("./output.wav")).unwrap();
	let out_buffer = Arc::new(RwLock::new(vec![]));
	let out_buffer_thread = out_buffer.clone();

	let midi = MidiInput::new("chordial-cli-test").unwrap();
	
	println!("available midi inputs:");

	for port in midi.ports() {
		println!("  {}", midi.port_name(&port).unwrap_or("(could not get port name)".to_string()));
	}

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
		buffer_size: cpal::BufferSize::Fixed(128),
	};

	let mut engine = Engine::new(config.sample_rate.0);

	engine.register("chordial.cli.midi-in", || Box::new(MidiIn::new()));
	engine.load_from_file(&PathBuf::from("midi.chrp"));
	engine.playing = true;

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

	let runtime_secs = 0.0;
	let start = Instant::now();
	
	loop {
		
		if runtime_secs > 0.0 && (Instant::now() - start).as_secs_f64() >= runtime_secs {
			break
		}

		std::thread::sleep(Duration::from_secs_f64(0.2));
		
		let (process_time, buffer_time, buffer_size) = {
			let lock = engine.lock().unwrap();
			(lock.dbg_process_time, lock.dbg_buffer_time, lock.dbg_buffer_size)
		};

		println!("ct/bt: {:.2}% - ct: {:.2}ms - bt: {:.2}ms - buf: {}",
			(process_time / buffer_time) * 100.0f32,
			process_time * 1000.0f32,
			buffer_time * 1000.0f32,
			buffer_size,
		)
	}

	
	stream.pause().unwrap();

	wav::write(
		Header::new(
			WAV_FORMAT_IEEE_FLOAT,
			2,
			config.sample_rate.0,
			32,
		), 
		&BitDepth::ThirtyTwoFloat(out_buffer.write().unwrap().drain(..).collect()), 
		&mut out
	).unwrap();
}