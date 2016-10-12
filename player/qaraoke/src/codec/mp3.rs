use ogk::ogg;
use mpg123;
use sample;
use sample::signal::Signal;
use types;
use glium;
use std::sync::mpsc;

struct Mp3Decoder {
    queue_sender: mpsc::Sender<types::AudioBlock>,
    decoder: mpg123::Handle<i16>,
    sample_frequency: u64,
    aux_headers: usize,
}

struct Mp3DecoderFrontend {
    receiver: Option<mpsc::Receiver<types::AudioBlock>>,
}

impl Mp3Decoder {
    fn handle_samples<S: Signal<Item=[i16;2]>>(&mut self, signal: S) {
        use std::iter::FromIterator;
        // I really give no shits whether the samples actually arrive,
        // because the only reason they wouldn't is that the playback
        // stream has moved on, in which case this file isn't going to
        // get decoded much longer.
        self.queue_sender.send(types::AudioBlock{
            block: Vec::from_iter(signal),
        }).ok();
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
    fn quality(&self) -> u32 { 240_000 }
    fn take_sample_queue(&mut self) -> Option<mpsc::Receiver<types::AudioBlock>> {
        return self.receiver.take()
    }
}

pub fn try_start_stream<S: glium::Surface>(raw_header: &[u8]) -> Option<(Box<ogg::BitstreamDecoder>, types::StreamDesc<S>)> {
    use byteorder::{ByteOrder, LittleEndian};
    if &raw_header[0..9] != b"OggMP3\0\0\0" {
        return None;
    }
    let aux_headers = raw_header[11] as usize;
    let sample_freq = LittleEndian::read_u32(&raw_header[16..20]) as u64;
    let (sq_sender, sq_receiver) = mpsc::channel();

    let decoder = Box::new(Mp3Decoder{
        queue_sender: sq_sender,
        decoder: {
            let mut handle = mpg123::Handle::new().unwrap();
            handle.open_feed().unwrap();
            handle
        },
        sample_frequency: sample_freq,
        aux_headers: aux_headers,
    }) as Box<ogg::BitstreamDecoder>;

    let frontend = types::StreamDesc::Audio(
        Some(Box::new(Mp3DecoderFrontend{
            receiver: Some(sq_receiver),
        }))
    );

    Some((decoder, frontend))
}
