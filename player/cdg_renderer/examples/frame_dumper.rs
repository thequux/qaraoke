extern crate cdg;
extern crate cdg_renderer;
extern crate image;
use image::{GenericImage};
use std::fs::File;

const SECTORS_PER_FRAME : usize = 3;

fn main() {
    let mut args = std::env::args().skip(1);
    let filename = args.next().expect("Usage: $0 filename destdir");
    let destdir = args.next().expect("Usage: $0 filename destdir");

    let infile = File::open(filename).unwrap();
    let mut scsi = cdg::SubchannelStreamIter::new(std::io::BufReader::with_capacity(16384, infile));

    let mut frame_no = 0;
    let mut sector_no = 0;
    let mut res_image = image::RgbaImage::new(300,216);
    let mut interp = cdg_renderer::CdgInterpreter::new();
    
    while let Some(sector) = scsi.next() {
        if sector_no != 0 && sector_no % SECTORS_PER_FRAME == 0 {
            // render a frame
            res_image.copy_from(&interp, 0, 0);
            // for now, don't dump; just benchmarking

            res_image.save(format!("{}/frame_{:05}.png", destdir, frame_no)).unwrap();
            frame_no += 1;
        }
        for cmd in sector {
            interp.handle_cmd(cmd)
        }
        sector_no += 1;
    }
}
