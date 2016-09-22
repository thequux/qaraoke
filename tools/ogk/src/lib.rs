//#![feature(collections)]
#![allow(unknown_lints)]
#[macro_use] extern crate bitflags;
#[macro_use] extern crate lazy_static;
extern crate byteorder;
extern crate lz4;
extern crate cdg as cdg_parser;
extern crate rand;

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
