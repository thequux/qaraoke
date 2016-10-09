use ogk::ogg;
use mpg123;
use std::collections::VecDeque;
use std::cell::RefCell;
use std::rc::Rc;
use sample;
use sample::signal::Signal;
use types;
use glium;

type SampleQueue = Rc<RefCell<VecDeque<[i16;2]>>>;

struct Mp3Decoder {
    sample_queue: SampleQueue,
    decoder: mpg123::Handle<i16>,
    sample_frequency: u64,
    aux_headers: usize,
}

struct Mp3DecoderFrontend {
    sample_queue: SampleQueue,
}

impl Mp3Decoder {
    fn handle_samples<S: Signal<Item=[i16;2]>>(&mut self, signal: S) {
        self.sample_queue.borrow_mut().extend(signal)
    }
}

impl ogg::BitstreamDecoder for Mp3Decoder {
    fn map_granule(&self, timestamp: u64) -> u64 { 1000_000 * timestamp / self.sample_frequency }
    fn num_headers(&self) -> usize { self.aux_headers + 1 }
    fn process_header(&mut self, _: &[u8]) { }
    fn process_packet(&mut self, packet: &[u8], last_granule: u64) -> u64 {
        if let Err(e) = self.decoder.feed(packet) {
            println!("Encountered mp3 error at granule {}: {:?}", last_granule, e);
        }
        // Move what data we can out
        let mut buf : [i16;2304] = [0;2304];
        let res = self.decoder.shit(&mut buf);
        match res {
            Err(e) => {
                println!("Encountered mp3 decode error at granule {}: {:?}", last_granule, e);
                return last_granule+1;
            },
            Ok((rate, channels, nsamples)) => {
                use sample::signal::Signal;
                if nsamples > 0 {
                    if channels == 2 {
                         self.handle_samples(sample::signal::from_interleaved_samples(buf.iter().cloned())
                                             .from_hz_to_hz(rate as f64, 48000.))
                     } else {
                         self.handle_samples(
                             buf.iter().cloned().map(|x| [x;2]).from_hz_to_hz(rate as f64, 48000.)
                         )
                     }
                }
            },
        };
        last_granule + 1
    }

    fn notice_gap(&mut self) {}
    fn finish(&mut self) {}
}

impl types::AudioCodec for Mp3DecoderFrontend {
    fn quality(&self) -> u32 { 32768 }
    fn get_samples(&mut self, buffer: &mut [[i16;2]]) -> Result<(), types::CodecError> {
        let mut queue = self.sample_queue.borrow_mut();
        for pos in buffer.iter_mut() {
            if let Some(s) = queue.pop_front() {
                *pos = s;
            } else {
                return Err(types::CodecError::Underrun);
            }
        }
        Ok(())
    }
}

pub fn try_start_stream<S: glium::Surface>(raw_header: &[u8]) -> Option<(Box<ogg::BitstreamDecoder>, types::StreamDesc<S>)> {
    use byteorder::{ByteOrder, LittleEndian};
    use std::cell::RefCell;
    if &raw_header[0..9] != b"OggMP3\0\0\0" {
        return None;
    }
    let aux_headers = raw_header[11] as usize;
    let sample_freq = LittleEndian::read_u32(&raw_header[16..20]) as u64;
    let sample_queue : SampleQueue = Default::default();

    let decoder = Box::new(Mp3Decoder{
        sample_queue: sample_queue.clone(),
        decoder: {
            let mut handle = mpg123::Handle::new().unwrap();
            handle.open_feed().unwrap();
            handle
        },
        sample_frequency: sample_freq,
        aux_headers: aux_headers,
    }) as Box<ogg::BitstreamDecoder>;

    let frontend = types::StreamDesc::Audio(
        Rc::new(RefCell::new(Mp3DecoderFrontend{
            sample_queue: sample_queue
        }))
    );

    Some((decoder, frontend))
}
