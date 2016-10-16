extern crate byteorder;
extern crate cdg;
extern crate cdg_renderer;
extern crate fps_counter;
#[macro_use]
extern crate glium;
extern crate image;
extern crate mpg123;
extern crate ogk;
extern crate portaudio;
extern crate sample;
extern crate crossbeam;
extern crate soxr;

#[cfg(feature="raspberry_pi")]
extern crate glium_pib;

// Import codecs
pub mod rt;
mod codec;
mod ao;

use std::rc::Rc;
use std::error::Error;

use glium::backend::Facade;


pub mod types {
    use glium;
    use std::rc::Rc;
    use rt::ringbuffer;
    
    pub type Sample = [f32; 2];

    pub struct AudioBlock {
        pub block: Vec<Sample>,
    }

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

        /// Sets the output ringbuffer, which expects to receive audio
        /// samples at 48kHz.
        fn set_ringbuffer(&mut self, ringbuffer::Writer<Sample>);

        /// Return the size of chunks that are produced into the buffer.
        fn min_buffer_size(&self) -> u32;

        /// Fill up the output buffer as much as possible.  Must be
        /// called at least once per buffer period.
        fn do_needful(&mut self);
    }

    pub trait VideoCodec<Surface: glium::Surface> {
        /// Do pre-playback initialization. Compile shaders, set up
        /// textures, etc.
        fn initialize(&mut self, context: &Rc<glium::backend::Context>);
        /// Render a frame. initialize will be called first.
        /// when is measured in milliseconds since the start of playback.
        fn render_frame(&mut self, context: &Rc<glium::backend::Context>, target: &mut Surface, when: f64);
    }

    //#[derive(Clone)]
    pub enum StreamDesc<Surface> {
        Audio(Option<Box<AudioCodec>>),
        Video(Option<Box<VideoCodec<Surface>>>),
    }
}

struct KaraokeSource<R, S> {
    demux: ogk::ogg::OggDemux<R, types::StreamDesc<S>>,
    audio: Option<Box<types::AudioCodec>>,
    video: Option<Box<types::VideoCodec<S>>>,
}

impl <R: std::io::Read, S: glium::Surface + 'static> KaraokeSource<R, S> {
    pub fn from_stream(reader: R) -> Result<Self, Box<Error>> {
        use types::StreamDesc;
        let mut source = KaraokeSource{
            demux: try!(ogk::ogg::OggDemux::new(reader, codec::identify_header)),
            audio: None,
            video: None,
        };

        //let mut video = None;
        source.audio = source.demux.streams()
            .filter_map(|(_stream_id, stream)| match stream {
                &mut StreamDesc::Audio(ref mut codec @ Some(_)) => Some(codec),
                _ => None,
            })
            .max_by_key(|x| x.as_ref().unwrap().quality())
            .map_or_else(
                || None,
                |stream| stream.take()
            );
        source.video = source.demux.streams()
            .filter_map(|(_stream_id, stream)| match stream {
                &mut StreamDesc::Video(ref mut codec @ Some(_)) => Some(codec),
                _ => None,
            })
            .next()
            .map_or_else(
                || None,
                |stream| stream.take(),
            );

        // Close off the excess streams...
        let discard_streams : Vec<_> = source.demux.streams()
            .filter_map(|(id, stream)| {
                match stream {
                    &mut StreamDesc::Audio(Some(_)) => Some(id),
                    &mut StreamDesc::Video(Some(_)) => Some(id),
                    _ => None,
                }
            })
            .collect();
        for stream in discard_streams {
            source.demux.ignore_stream(stream)
        }

        // Get the first chunk of packets processed
        try!(source.demux.pump_until(1000));
        Ok(source)
    }
}

// TODO: Add glium_pib for bare metal Raspberry Pi support

#[cfg(feature="raspberry_pi")]
fn pib_open_display() -> Rc<glium::backend::Context> {
    use std::sync::Arc;
    let system = glium_pib::System::new(Default::default());
    let system = match system {
        Ok(s) => s,
        Err(_) => {
            panic!("Failed to use broadcom libraries.");
        }
    };
    let system = Arc::new(system);
    let facade : Result<Rc<glium::backend::Context>,_> = glium_pib::create_window_facade(
        &system,
        &std::default::Default::default()
    );
    match facade {
        Ok(f) => f,
        Err(_) => {
            panic!("Failed to use broadcom libraries.");
        },
    }
}

#[cfg(not(feature="raspberry_pi"))]
fn pib_open_display() -> Rc<glium::backend::Context> {
    panic!("Unable to create window");
}

fn main() {
    use std::fs;
    let args: Vec<String> = std::env::args().collect();
    let filename = args.get(1).expect("Usage: $0 filename");
    let mut player = KaraokeSource::from_stream(fs::File::open(filename).unwrap()).unwrap();
    use glium::DisplayBuild;

    let display = glium::glutin::WindowBuilder::new().build_glium();
    let display: Rc<glium::backend::Context> = match display {
        Ok(f) => f.get_context().clone(),
        Err(_) => pib_open_display(),
    };

    let mut frame_count = 0;
    let mut fps = fps_counter::FPSCounter::new();
    let start_time = std::time::Instant::now();

    let mut ao_driver = ao::open().unwrap();
    ao_driver.start().unwrap();
    {
        // Set up a stream
        if let Some(ref mut vcodec) = player.video {
            vcodec.initialize(display.get_context())
        }
        if let Some(ref mut acodec) = player.audio {
            // We cheat here and always initialize ring buffers to half a
            // second.
            let (rd, wr) = rt::ringbuffer::new(96000);
            acodec.set_ringbuffer(wr);
            acodec.do_needful();
            ao_driver.change_stream(Some(rd)).unwrap();
        } else {
            ao_driver.change_stream(None).unwrap();
        }

        ao_driver.zero_time().unwrap();
        ao_driver.commit().unwrap();
    }

    // Wait for the driver to synchronize
    while !ao_driver.all_commands_processed() {
        // Do nothing
    }
    
    loop {
        // Do updates
        let time = ao_driver.timestamp();
        if let Some(ref mut vcodec) = player.video {
            let mut target = glium::Frame::new(
                display.clone(),
                display.get_framebuffer_dimensions(),
            );
            vcodec.render_frame(display.get_context(), &mut target, time);
            target.finish().unwrap();
        }
        player.demux.pump_until((time * 1000. + 1000.) as u64).unwrap();
        if let Some(ref mut acodec) = player.audio {
            acodec.do_needful()
        }
        // Handle events
        /*
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
        */
    }
}
