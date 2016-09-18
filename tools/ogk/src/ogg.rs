use std::error;
pub trait BitstreamFrame {
    fn content(&self) -> &[u8];
    fn timestamp(&self) -> u64;
}

pub trait BitstreamCoder {
    type Frame : BitstreamFrame;
    type Error : error::Error;
    
    fn headers(&self) -> Vec<Vec<u8>>;
    fn next_frame(&mut self) -> Result<Option<Self::Frame>, Self::Error>;
}
