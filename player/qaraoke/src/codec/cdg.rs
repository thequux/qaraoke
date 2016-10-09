use cdg;
use cdg_renderer;
use image;
use glium;
use std::borrow::Cow;
use std::collections::VecDeque;
use std::rc::Rc;
use std::cell::RefCell;
use ogk::ogg;
use ogk;
use types;




#[derive(Copy,Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

struct CdgPlayerRsrc {
    program: glium::Program,
    indices: glium::index::NoIndices,
    vtx_buffer: glium::VertexBuffer<Vertex>,
}

impl CdgPlayerRsrc {
    fn new(ctx: &Rc<glium::backend::Context>) -> Self {
        let billboard_vtx = [
            // Note that the texture coordinates are inverted from GL coordinates
            // you'd expect; this puts 0,0 at the top left corner
            Vertex{position: [-0.5, -0.5], tex_coords: [0.0, 1.0]},
            Vertex{position: [-0.5,  0.5], tex_coords: [0.0, 0.0]},
            Vertex{position: [ 0.5,  0.5], tex_coords: [1.0, 0.0]},
            Vertex{position: [ 0.5, -0.5], tex_coords: [1.0, 1.0]},
        ];

        let vertex_buffer = glium::VertexBuffer::new(ctx, &billboard_vtx).unwrap();
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

        let program = glium::Program::from_source(ctx, vertex_shader_src, fragment_shader_src, None).unwrap();
        CdgPlayerRsrc{
            program: program,
            indices: indices,
            vtx_buffer: vertex_buffer,
        }
    }
}

struct DecodeChannel {
    queue: VecDeque<(u32, cdg::Command)>,
    finished: bool,
}

impl Default for DecodeChannel {
    fn default() -> Self {
        DecodeChannel{
            queue: Default::default(),
            finished: false,
        }
    }
}

type CommandQueue = Rc<RefCell<DecodeChannel>>;
pub struct CdgPlayer {
    // The first element of the pair is the 75fps frame in which the
    // command should be executed.
    cdg_stream: CommandQueue,
    interp: cdg_renderer::CdgInterpreter,

    current_sector: u32,
    finished: bool,

    out_buffer: image::RgbaImage,
    render_resources: Option<CdgPlayerRsrc>,
}


impl CdgPlayer {
    fn new(queue: CommandQueue) -> Self {
        CdgPlayer{
            cdg_stream: queue,
            interp: cdg_renderer::CdgInterpreter::new(),

            current_sector: 0,
            finished: false,

            out_buffer: image::RgbaImage::new(300,216),
            render_resources: None,
        }
    }

    /// Update playback to time `time`, measured in milliseconds since
    /// start of playback.
    /// # Returns
    /// 
    fn update(&mut self, time: u32) {
        let target_sector = time * 3 / 40;
        let stream = self.cdg_stream.borrow_mut();
        while self.current_sector < target_sector {
            if let Some((ts, cmd)) = stream.queue.pop_front() {
                if ts > target_sector {
                    stream.queue.push_front((ts, cmd));
                    return;
                }
                self.interp.handle_cmd(cmd);
                self.current_sector = ts;
            }
        }
    }

    fn render(&mut self) -> &image::RgbaImage {
        use image::GenericImage;
        self.out_buffer.copy_from(&self.interp, 0,0);
        &self.out_buffer
    }
}

impl <S: glium::Surface> types::VideoCodec<S> for CdgPlayer {
    fn initialize(&mut self, ctx: &Rc<glium::backend::Context>) {
        self.render_resources = Some(CdgPlayerRsrc::new(ctx));
    }
    
    fn render_frame(&mut self, ctx: &Rc<glium::backend::Context>, target: &mut S, when: u32) {
        self.update(when);
        let rsrc = self.render_resources.as_ref().unwrap();
        let player_image = self.render();

        // Render
        use glium::Surface;
        target.clear_color(0.0, 0.0, 1.0, 1.0);

        let glimage = glium::texture::RawImage2d{
            data: Cow::Borrowed(player_image),
            width: 300,
            height: 216,
            format: glium::texture::ClientFormat::U8U8U8U8,
        };
        //let glimage = glium::texture::RawImage2d::from_raw_rgba_reversed(image.into_raw(), (300,216));
        let texture = glium::texture::Texture2d::new(ctx, glimage).unwrap();
        let uniforms = uniform!{
            tex: texture.sampled().magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest),
        };
        target.draw(&rsrc.vtx_buffer, &rsrc.indices, &rsrc.program, &uniforms, &Default::default()).unwrap();
    }
}

struct CdgDecoder {
    header: ogk::cdg::CdgHeader,
    queue: CommandQueue,
}

impl ogg::BitstreamDecoder for CdgDecoder {
    fn map_granule(&self, granule: u64) -> u64 { (granule >> 20) * 40_000 / 3 }

    fn num_headers(&self) -> usize { 0 }

    fn process_header(&mut self, _: &[u8]) { }
    fn process_packet(&mut self, packet: &[u8], last_granule: u64) -> u64 {
        use ogk::cdg::PacketType;
        let last_sector = last_granule >> 20;
        let last_keyframe = last_granule & 0xFFFFF;
        let mut cur_sector = 0;
        let queue = self.queue.borrow_mut();
        match self.header.decode_packet(packet) {
            Some((PacketType::Command, cmds)) => {
                for sector in cmds.chunks(96) {
                    cur_sector += 1;
                    for cmd in cdg::SectorIter::new(sector) {
                        queue.queue.push_back( ((last_sector + cur_sector) as u32, cmd) );
                    }
                }
                (last_sector + cur_sector) << 20 | (last_keyframe + cur_sector) & 0xFFFF
            },
            Some((PacketType::Keyframe, _)) => {
                // We ignore the keyframe
                last_sector & !0xFFFFF
            },
            _ => last_granule
        }
    }
    fn notice_gap(&mut self) {}
    fn finish(&mut self) {
        self.queue.borrow_mut().finished = true;
    }
}

pub fn try_start_stream<S: glium::Surface>(raw_header: &[u8]) -> Option<(Box<ogg::BitstreamDecoder>, types::StreamDesc<S>)> {
    use ogk::cdg::*;
    use std::default::Default;
    CdgHeader::from_bytes(raw_header).map(|header| {
        let queue = Default::default();
        let decoder = Box::new(CdgDecoder{
            header: header,
            queue: queue,
        }) as Box<ogg::BitstreamDecoder>;
        let sd = types::StreamDesc::Video(Rc::new(CdgPlayer::new(queue)));
        (decoder, sd)
    })
}

