use ogk::ogg;
use glium;

use types;

pub mod cdg;
pub mod mp3;

pub fn identify_header<S: glium::Surface>(header: &[u8]) -> Option<(Box<ogg::BitstreamDecoder>, types::StreamDesc<S>)> {
    None.or_else(|| cdg::try_start_stream(header))
        .or_else(|| mp3::try_start_stream(header))
}
