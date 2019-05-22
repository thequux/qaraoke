#![allow(dead_code, unused_variables)]
/**
In many ways, the audio driver controls what the rest of the system
is doing, as the audio hardware works on its own time and controls
when "now" is. Further, we can't switch audio sources until the
hardware asks for packets.

Commands are passed in via a queue. Incoming commands are batched and
only executed upon receipt of a Commit command. The application of
commits is indicated in the command ID field of the timecode in the shared
driver state.

A time of ~0 indicates an unrecoverable error in a stream; a command
ID of ~0 in the timecode indicates a complete device failure.

Drivers are guaranteed to 
*/


use types;
use std::error::Error;
use std::sync::atomic;
use std::sync::Arc;
use rt::AtomicOption;
use rt;


pub type Stream = Option<rt::ringbuffer::Reader<types::Sample>>;

#[derive(Debug)]
enum DriverCommand {
    /// Change to a new stream. If the argument is None, plays silence
    /// at the last played sample value.
    ChangeStream(Stream),

    /// Resets the time counter to 0
    ZeroTime,

    /// (IMMEDIATE) Commits any outstanding changes. The argument is the new value
    /// for the command ID in the time counter.
    Commit(u16),

    /// (IMMEDIATE) Clear the command queue of any commands that may have been
    /// received.
    Abort,

    /// (IMMEDIATE) Do nothing. This is used for internal purposes, and there is
    /// unlikely to be a reason to use it externally.
    Nop, 
}

#[derive(Copy,Clone,Debug)]
pub struct AoStatus {
    // Guard 1 is incremented before writing to last_command or
    // timestamp, and guard2 is incremented after. Both are
    // initialized to 0.
    pub last_command: u16,
    // Time, in seconds
    pub timestamp: f64,
}

pub struct DriverBackend {
    shared: Arc<AtomicOption<AoStatus>>,
    /// The command queue is guaranteed to be able to hold exactly one
    /// of each type of deferred command. Later instances of a command replace earlier ones.
    deferred_commands: [DriverCommand; 2],

    /// The time as of the last ZeroTime command processed
    time_base: f64,

    /// The ID of the last Commit command
    command_id: u16,
    
    command_queue: rt::ringbuffer::Reader<DriverCommand>,

    current_stream: Stream,
}

pub struct DriverFrontend {
    shared: Arc<AtomicOption<AoStatus>>,
    cached_state: AoStatus,
    command_queue: rt::ringbuffer::Writer<DriverCommand>,
    last_cmd_sent: u16,
    driver: self::pa::Driver,
}

impl DriverBackend {
    fn new(shared: Arc<AtomicOption<AoStatus>>, queue: rt::ringbuffer::Reader<DriverCommand>) -> Self {
        let mut backend = DriverBackend{
            shared: shared,
            deferred_commands: Self::default_deferred_commands(),
            time_base: 0.,
            command_id: 0,
            command_queue: queue,
            current_stream: None,
        };
        backend.receive_command(DriverCommand::ZeroTime, 0.);
        backend
    }

    /// This time value is in seconds since some arbitrary epoch.
    fn handle_commands(&mut self, time: f64) {
        // Handle comands

        // TODO: Do this with less synchronization; each pop costs
        // ~1e2 cycles
        while let Some(command) = self.command_queue.pop() {
            self.receive_command(command, time);
        }
    }

    fn receive_command(&mut self, command: DriverCommand, time: f64) {
        use std::mem::replace;
        match command {
            DriverCommand::ChangeStream(_) => self.deferred_commands[0] = command,
            DriverCommand::ZeroTime => self.deferred_commands[1] = command,
            DriverCommand::Commit(v) => {
                println!("Committing");
                let mut cmdlist = replace(&mut self.deferred_commands, Self::default_deferred_commands());
                for dcmd in cmdlist.iter_mut() {
                    self.process_command(replace(dcmd, DriverCommand::Nop), time);
                }

                self.command_id = v;
                self.shared.swap(AoStatus{
                    last_command: v,
                    timestamp: self.time_base,
                }, atomic::Ordering::Release);
            },
            DriverCommand::Abort => self.deferred_commands = Self::default_deferred_commands(),
            DriverCommand::Nop => (),
        }
    }

    fn default_deferred_commands() -> [DriverCommand; 2] {
        [DriverCommand::Nop,
         DriverCommand::Nop,
        ]
    }

    fn process_command(&mut self, command: DriverCommand, time: f64) {
        match command {
            DriverCommand::ChangeStream(stream) => self.current_stream = stream,
            DriverCommand::ZeroTime => self.time_base = time,
            _ => (),
        }
    }

    fn signal(&mut self) -> DriverSignal {
        DriverSignal{
            iter: self.current_stream.as_mut().map(|s| s.iter()),
            underrun_count: 0,
        }
    }
}

struct DriverSignal<'a> {
    iter: Option<rt::ringbuffer::ReadIter<'a, types::Sample>>,
    underrun_count: usize,
}

impl <'a> Iterator for DriverSignal<'a> {
    type Item = types::Sample;
    fn next(&mut self) -> Option<types::Sample> {
        Some(self.iter.as_mut().and_then(|i| i.next()).unwrap_or_else(|| {
            self.underrun_count += 1;
            [0.0,0.0]
        }))
    }
}

impl <'a> Drop for DriverSignal<'a> {
    fn drop(&mut self) {
        if self.underrun_count != 0 && self.iter.is_some() {
            println!("Dropped {} samples", self.underrun_count);
        }
    }
}

impl DriverFrontend {
    fn new(shared: Arc<AtomicOption<AoStatus>>, queue: rt::ringbuffer::Writer<DriverCommand>, hw: pa::Driver) -> Self {
        DriverFrontend {
            shared: shared,
            cached_state: AoStatus{
                last_command: 0,
                timestamp: 0.,
            },
            command_queue: queue,
            last_cmd_sent: 0,
            driver: hw,
        }
    }

    /// Changes the current stream to the provided stream, or returns
    /// it if the process fails for some reason.
    pub fn change_stream(&mut self, stream: Stream) -> Result<(), Stream> {
        self.command_queue.push(DriverCommand::ChangeStream(stream)).map_err(|cmd| match cmd {
            DriverCommand::ChangeStream(stream) => stream,
            _ => unreachable!(),
        })
    }

    pub fn zero_time(&mut self) -> Result<(), ()> {
        self.command_queue.push(DriverCommand::ZeroTime).map_err(|_|())
    }

    pub fn commit(&mut self) -> Result<u16, ()> {
        self.last_cmd_sent += 1;
        self.command_queue.push(DriverCommand::Commit(self.last_cmd_sent))
            .map_err(|_|())
            .map(|_| self.last_cmd_sent)
    }

    fn current_status(&mut self) -> AoStatus {
        if let Some(state) = self.shared.take(atomic::Ordering::Acquire) {
            self.cached_state = state;
        }
        self.cached_state
    }
    
    pub fn all_commands_processed(&mut self) -> bool {
        self.current_status().last_command == self.last_cmd_sent
    }

    pub fn timestamp(&mut self) -> f64 {
        self.driver.time() - self.current_status().timestamp
    }

    pub fn start(&mut self) -> Result<(), Box<Error>> {
        self.driver.start()
    }
}

pub fn open() -> Result<DriverFrontend, Box<Error>> {
    let status_chan = Arc::new(AtomicOption::new());
    let (cmd_rd, cmd_wt) = rt::ringbuffer::new(16);
    let backend = DriverBackend::new(status_chan.clone(), cmd_rd);
    let frontend = DriverFrontend::new(status_chan, cmd_wt, try!(pa::Driver::open(backend)));

    // Initialize PortAudio
    return Ok(frontend);
}

mod pa {
    use portaudio;
    use std::error::Error;
    use std::time;
    
    const SAMPLE_RATE: f64 = 48_000.;
    const FRAMES_PER_BUFFER: u32 = 64;
    const CHANNELS: i32 = 2;

    fn override_time(base: time::Instant, _: f64) -> f64 {
        let now = time::Instant::now() - base;
        now.as_secs() as f64 + (now.subsec_nanos() as f64) / 1000_000_000.
    }
    
    pub struct Driver {
        pa: portaudio::PortAudio,
        stream: portaudio::Stream<portaudio::NonBlocking, portaudio::Output<f32>>,
        base: time::Instant,
    }

    impl Driver {
        pub fn time(&self) -> f64 {
            override_time(self.base, self.stream.time())
        }
        pub fn open(mut backend: super::DriverBackend) -> Result<Driver, Box<Error>> {
            let pa = try!(portaudio::PortAudio::new());
            let mut settings = try!(pa.default_output_stream_settings(CHANNELS, SAMPLE_RATE, FRAMES_PER_BUFFER));
            settings.flags = portaudio::stream_flags::CLIP_OFF;
            let base_time = time::Instant::now();

            let callback = move |portaudio::OutputStreamCallbackArgs{buffer, frames, time, ..}| {
                use sample::Signal;
                backend.handle_commands(override_time(base_time, time.buffer_dac));
                //println!("now: {}", time.buffer_dac);
                for (dst, src) in buffer.iter_mut().zip(backend.signal().to_samples()) {
                    *dst = src
                }
                portaudio::Continue
            };
            let stream = try!(pa.open_non_blocking_stream(settings, callback));
            Ok(Driver{pa: pa, stream: stream, base: base_time})
        }

        pub fn start(&mut self) -> Result<(), Box<Error>> {
            try!(self.stream.start());
            Ok(())
        }

        /// Note that this is an absolute minimum; codecs should
        /// probably set their buffer sizes to be at least two video
        /// frames long
        pub fn min_buffer_size(&self) -> u32 {
            FRAMES_PER_BUFFER * 2
        }
    }
}
