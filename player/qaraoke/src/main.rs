extern crate cdg;
extern crate cdg_renderer;
#[macro_use]
extern crate glium;
extern crate image;
extern crate fps_counter;
extern crate ogk;
extern crate mpg123;
extern crate sample;
extern crate byteorder;

// Import codecs
mod codec;
   
use std::cell::RefCell;
use std::error::Error;
use std::rc::Rc;

use glium::backend::Facade;


pub mod types {
    use glium;
    use std::rc::Rc;
    use std::cell::RefCell;
    pub enum CodecError {
        Underrun,
    }
    
    pub trait AudioCodec {
        /// Ranks quality of various codecs; higer is better. There's
        /// no scale to this number.  As a rough guide, this should be
        /// the number of bits of entropy per second, adjusted by the
        /// quality of the codec, where Opus is the reference.
        ///
        /// As this is not well-defined, wing it as best you can.
        fn quality(&self) -> u32;

        /// Retrieve 48kHz, signed 16 bit samples in interleaved stereo.
        /// If you can't fill the buffer, return underrun.
        fn get_samples(&mut self, buffer: &mut [[i16;2]]) -> Result<(), CodecError>;
    }

    pub trait VideoCodec<Surface: glium::Surface> {
        /// Do pre-playback initialization. Compile shaders, set up
        /// textures, etc.
        fn initialize(&mut self, context: &Rc<glium::backend::Context>);
        /// Render a frame. initialize will be called first.
        /// when is measured in milliseconds since the start of playback.
        fn render_frame(&mut self, context: &Rc<glium::backend::Context>, target: &mut Surface, when: u32);
    }

    #[derive(Clone)]
    pub enum StreamDesc<Surface> {
        Audio(Rc<RefCell<AudioCodec>>),
        Video(Rc<RefCell<VideoCodec<Surface>>>),
    }
}

struct KaraokeSource<R, S> {
    demux: ogk::ogg::OggDemux<R, types::StreamDesc<S>>,
    audio: Option<Rc<RefCell<types::AudioCodec>>>,
    video: Option<Rc<RefCell<types::VideoCodec<S>>>>,
}

impl <R: std::io::Read, S: glium::Surface + 'static> KaraokeSource<R, S> {
    pub fn from_stream(reader: R) -> Result<Self, Box<Error>> {
        use types::StreamDesc;
        let mut source = KaraokeSource{
            demux: try!(ogk::ogg::OggDemux::new(reader, codec::identify_header)),
            audio: None,
            video: None,
        };
        for (_stream_id, stream) in source.demux.streams() {
            match stream {
                &StreamDesc::Audio(ref stream) => {
                    source.audio = Some(source.audio.map_or_else(
                        || stream.clone(),
                        |old_stream| if (*old_stream).borrow().quality() < (**stream).borrow().quality() {
                            stream.clone()
                        } else {
                            old_stream
                        }
                    ));
                },
                &StreamDesc::Video(ref stream) => {
                    // TODO: Provide some way to decide between codecs
                    source.video = Some(stream.clone());
                }
            }
        }

        Ok(source)
    }
    
}

// TODO: Add glium_pib for bare metal Raspberry Pi support

fn main() {
    use std::fs;
    let args: Vec<String> = std::env::args().collect();
    let filename = args.get(1).expect("Usage: $0 filename");
    let mut player = KaraokeSource::from_stream(fs::File::open(filename).unwrap()).unwrap();
    use glium::DisplayBuild;

    let display = glium::glutin::WindowBuilder::new().build_glium().unwrap();
   
    let mut frame_count = 0;
    let mut fps = fps_counter::FPSCounter::new();
    let start_time = std::time::Instant::now();

    if let Some(ref vcodec) = player.video {
        vcodec.borrow_mut().initialize(display.get_context())
    }
    
    loop {
        // Do updates
        let playtime = std::time::Instant::now().duration_since(start_time);
        let time_ms = (playtime.as_secs() * 1000) as u32 + playtime.subsec_nanos() / 1000_000;
        if let Some(ref vcodec) = player.video {
            let mut target = display.draw();
            vcodec.borrow_mut().render_frame(display.get_context(), &mut target, time_ms);
            target.finish().unwrap();
        }

        player.demux.pump_until(time_ms as u64 * 1000 + 1000_000);
        // Handle events
        for ev in display.poll_events() {
            match ev {
                glium::glutin::Event::Closed => return,
                _ => (),
            }
        }
        frame_count += 1;
        let fps_c = fps.tick();
        if frame_count % 100 == 0 {
            display.get_window().map(|win| win.set_title(&format!("{} fps", fps_c)));
        }
    }
}
