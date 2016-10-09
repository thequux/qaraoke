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
   
use std::borrow::Cow;
use std::rc::Rc;


pub mod types {
    use glium;
    use std::rc::Rc;
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

    pub enum StreamDesc<Surface> {
        Audio(Rc<AudioCodec>),
        Video(Rc<VideoCodec<Surface>>),
    }
}

// TODO: Add glium_pib for bare metal Raspberry Pi support

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let filename = args.get(1).expect("Usage: $0 filename");
    let player = unsafe { std::mem::uninitialized() };
    use glium::DisplayBuild;

    let display = glium::glutin::WindowBuilder::new().build_glium().unwrap();
   
    let mut frame_count = 0;
    let mut fps = fps_counter::FPSCounter::new();
    let start_time = std::time::Instant::now();

    loop {
        // Do updates
        let playtime = std::time::Instant::now().duration_since(start_time);
        player.update((playtime.as_secs() * 1000) as u32 + playtime.subsec_nanos() / 1000000);
        let mut target = display.draw();
        player.render_frame(target);
        target.finish().unwrap();
        
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
