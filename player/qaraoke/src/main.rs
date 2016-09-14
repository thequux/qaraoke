extern crate cdg;
extern crate cdg_renderer;
#[macro_use]
extern crate glium;
extern crate image;
extern crate fps_counter;

// TODO: Add glium_pib for bare metal Raspberry Pi support

#[derive(Copy,Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

fn main() {

    use glium::DisplayBuild;

    let display = glium::glutin::WindowBuilder::new().build_glium().unwrap();
    let billboard_vtx = [
        Vertex{position: [-0.5, -0.5], tex_coords: [0.0, 0.0]},
        Vertex{position: [-0.5,  0.5], tex_coords: [0.0, 1.0]},
        Vertex{position: [ 0.5,  0.5], tex_coords: [1.0, 1.0]},
        Vertex{position: [ 0.5, -0.5], tex_coords: [1.0, 0.0]},
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

    use std::io::Cursor;

    let mut interp = cdg_renderer::CdgInterpreter::new();
    let mut frame_count = 0;
    let mut fps = fps_counter::FPSCounter::new();
    loop {
        use glium::Surface;
        use image::GenericImage;
        let mut target = display.draw();
        target.clear_color(0.0, 0.0, 1.0, 1.0);

        let mut image = image::RgbaImage::new(300,216);
        image.copy_from(&interp, 0,0);
        
        let glimage = glium::texture::RawImage2d::from_raw_rgba_reversed(image.into_raw(), (300,216));
        let texture = glium::texture::Texture2d::new(&display, glimage).unwrap();
        let uniforms = uniform!{
            tex: texture.sampled().magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest),
        };
        target.draw(&vertex_buffer, &indices, &program, &uniforms, &Default::default()).unwrap();
        target.finish().unwrap();
   
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
    println!("Hello, world!");
}


