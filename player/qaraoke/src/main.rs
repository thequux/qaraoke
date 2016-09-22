extern crate cdg;
extern crate cdg_renderer;
#[macro_use]
extern crate glium;
extern crate image;
extern crate fps_counter;
extern crate ogk;
use std::borrow::Cow;


// TODO: Add glium_pib for bare metal Raspberry Pi support

#[derive(Copy,Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let filename = args.get(1).expect("Usage: $0 filename");
    let mut player = CdgPlayer::new(filename).unwrap();
    use glium::DisplayBuild;

    let display = glium::glutin::WindowBuilder::new().build_glium().unwrap();
    let billboard_vtx = [
        // Note that the texture coordinates are inverted from GL coordinates
        // you'd expect; this puts 0,0 at the top left corner
        Vertex{position: [-0.5, -0.5], tex_coords: [0.0, 1.0]},
        Vertex{position: [-0.5,  0.5], tex_coords: [0.0, 0.0]},
        Vertex{position: [ 0.5,  0.5], tex_coords: [1.0, 0.0]},
        Vertex{position: [ 0.5, -0.5], tex_coords: [1.0, 1.0]},
    ];

    let vertex_buffer = glium::VertexBuffer::new(&display, &billboard_vtx).unwrap();
    let indices = glium::index::NoIndices(glium::index::PrimitiveType::TriangleFan);

    let vertex_shader_src = r#"
    #version 140

    in vec2 position;
    in vec2 tex_coords;
    out vec2 v_tex_coords;

    void main() {
        gl_Position = vec4(position, 0.0, 1.0);
        v_tex_coords = tex_coords;
    }
"#;

    let fragment_shader_src = r#"
    #version 140

    in vec2 v_tex_coords;
    out vec4 color;

    uniform sampler2D tex;

    void main() {
        color = texture(tex, v_tex_coords);
    }
"#;

    let program = glium::Program::from_source(&display, vertex_shader_src, fragment_shader_src, None).unwrap();
    
    let mut frame_count = 0;
    let mut fps = fps_counter::FPSCounter::new();
    let start_time = std::time::Instant::now();

    loop {
        // Do updates
        let playtime = std::time::Instant::now().duration_since(start_time);
        player.update((playtime.as_secs() * 1000) as u32 + playtime.subsec_nanos() / 1000000);
        let player_image = player.render();

        // Render
        use glium::Surface;
        let mut target = display.draw();
        target.clear_color(0.0, 0.0, 1.0, 1.0);

        let glimage = glium::texture::RawImage2d{
            data: Cow::Borrowed(player_image),
            width: 300,
            height: 216,
            format: glium::texture::ClientFormat::U8U8U8U8,
        };
        //let glimage = glium::texture::RawImage2d::from_raw_rgba_reversed(image.into_raw(), (300,216));
        let texture = glium::texture::Texture2d::new(&display, glimage).unwrap();
        let uniforms = uniform!{
            tex: texture.sampled().magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest),
        };
        target.draw(&vertex_buffer, &indices, &program, &uniforms, &Default::default()).unwrap();
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

struct CdgPlayer {
    cdg_stream: cdg::SubchannelStreamIter<Box<std::io::Read>>,
    interp: cdg_renderer::CdgInterpreter,

    current_sector: u32,
    finished: bool,

    out_buffer: image::RgbaImage,
}


impl CdgPlayer {
    fn new(filename: &str) -> std::io::Result<Self> {
        let file = Box::new(try!(std::fs::File::open(filename)));
        
        Ok(CdgPlayer{
            cdg_stream: cdg::SubchannelStreamIter::new(file),
            interp: cdg_renderer::CdgInterpreter::new(),

            current_sector: 0,
            finished: false,

            out_buffer: image::RgbaImage::new(300,216),
        })
    }

    /// Update playback to time `time`, measured in milliseconds since
    /// start of playback.
    /// # Returns
    /// 
    fn update(&mut self, time: u32) {
        let target_sector = time * 3 / 40;
        while self.current_sector < target_sector && !self.finished {
            if let Some(cmds) = self.cdg_stream.next() {
                for cmd in cmds {
                    self.interp.handle_cmd(cmd)
                }
            } else {
                self.finished = true;
            }
            self.current_sector += 1;
        }
    }

    fn render(&mut self) -> &image::RgbaImage {
        use image::GenericImage;
        self.out_buffer.copy_from(&self.interp, 0,0);
        &self.out_buffer
    }
}
