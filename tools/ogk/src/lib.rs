//#![feature(collections)]
extern crate byteorder;
extern crate lz4;
extern crate cdg as cdg_parser;

pub mod mp3;
pub mod util;
pub mod ogg;
pub mod cdg;


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
