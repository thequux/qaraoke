extern crate ogk;

use std::io::prelude::*;
use std::io;

fn hexdump(buf: &[u8]) -> String {
    buf.iter()
        .map(|x| format!("{:02x}", x))
        .collect::<Vec<String>>()
        .join(" ")
}

struct DumpingBitstream(u32);

impl ogk::ogg::BitstreamDecoder for DumpingBitstream {
    fn map_granule(&self, granule: u64) -> u64 {granule}

    fn num_headers(&self) -> usize { 1 }

    fn process_header(&mut self, header: &[u8]) {
        println!("{}@header: {}", self.0, hexdump(header));
    }

    fn process_packet(&mut self, header: &[u8], last_granule: u64) -> u64 {
        println!("{}@{}: {}", self.0, last_granule, hexdump(header));
        last_granule + 1
    }
    fn notice_gap(&mut self) {
        println!("{}@gap", self.0);
    }

    fn finish(&mut self) {
        println!("{}@end", self.0);
    }
}

fn main() {
    use std::cell::Cell;
    let stream_no = Cell::new(0);
    
    match ogk::ogg::OggDemux::new(io::stdin(), move |header| {
        let s = stream_no.get();
        stream_no.set(s + 1);
        println!("{}@initial: {}", s,  hexdump(header));
        Some((Box::new(DumpingBitstream(s)), s))
    }) {
        Ok(mut oggfile) => {
            while !oggfile.is_eof() {
                oggfile.pump_page().unwrap();
            }
        },
        Err(e) => {
            println!("{:?}", e);
        },
    }

    
}
