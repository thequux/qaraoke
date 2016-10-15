#![feature(conservative_impl_trait)]
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
extern crate libsoxr;

// Import codecs
pub mod rt;
mod codec;
mod ao;

use std::error::Error;

use glium::backend::Facade;


pub mod types {
    use glium;
    use std::rc::Rc;
    use std::sync::mpsc;
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
        fn render_frame(&mut self, context: &Rc<glium::backend::Context>, target: &mut Surface, when: u32);
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

    let mut ao_driver = ao::open().unwrap();
    let mut driver_key = 0;

    ao_driver.start();
    {
        // Set up a stream
        if let Some(ref mut vcodec) = player.video {
            vcodec.initialize(display.get_context())
        }
        if let Some(ref mut acodec) = player.audio {
            // We cheat here and always initialize ring buffers to half a
            // second.
            let (rd, wr) = rt::ringbuffer::new(24000);
            acodec.set_ringbuffer(wr);
            ao_driver.change_stream(Some(rd));
        } else {
            ao_driver.change_stream(None);
        }

        ao_driver.zero_time().unwrap();
        driver_key = ao_driver.commit().unwrap();
    }

    // Wait for the driver to synchronize
    while !ao_driver.all_commands_processed() {
        // Do nothing
    }
    
    loop {
        // Do updates
        let playtime = std::time::Instant::now().duration_since(start_time);
        let time_ms = (playtime.as_secs() * 1000) as u32 + playtime.subsec_nanos() / 1000_000;
        if let Some(ref mut vcodec) = player.video {
            let mut target = display.draw();
            vcodec.render_frame(display.get_context(), &mut target, time_ms);
            target.finish().unwrap();
        }
        player.demux.pump_until(time_ms as u64 * 1000 + 1000_000).unwrap();

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
