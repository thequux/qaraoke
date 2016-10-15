use ogk::ogg;
use mpg123;
use sample;
use sample::signal::Signal;
use types;
use glium;
use rt::ringbuffer;
use std::thread;
use std::sync::mpsc;
use libsoxr;

struct Mp3Decoder {
    queue_sender: mpsc::Sender<Vec<types::Sample>>,
    decoder: mpg123::Handle<f32>,
    sample_frequency: u64,
    aux_headers: usize,
    soxr: libsoxr::Soxr,
}

struct Mp3DecoderFrontend {
    receiver: mpsc::Receiver<Vec<types::Sample>>,
    ringbuffer: Option<ringbuffer::Writer<types::Sample>>,
}

fn as_interlaced<T>(buf: &mut [[T; 2]]) -> &mut [T] {
    use std::slice;
    unsafe {
        slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut T, buf.len() * 2)
    }
}

impl Mp3Decoder {
    fn handle_samples(&mut self, rate: f64, buf: &[f32]) {
        let mut obuf : Vec<[f32;2]> = vec![[0.0;2]; (buf.len() as f64 / 2. * 48000. / rate + 0.5) as usize];
        self.soxr.set_io_ratio(rate / 48000., 0).unwrap();
        // This function is deceptively unsafe; regardless of type
        // parameters, it will always use the types given in the initializer (f32/f32)
        if let Ok((idone, odone)) = self.soxr.process::<f32,f32>(Some(buf), as_interlaced(&mut obuf[..])) {
            if idone != buf.len() {
                println!("soxr didn't consume entire input buffer");
            }

            obuf.truncate(odone);
            self.queue_sender.send(obuf);
        } else {
            panic!("Soxr somehow failed");
        }
    }

    fn handle_finish(&mut self) {
        loop {
            let mut obuf = vec![[0.;2]; 512];
            if let Ok((idone, odone)) = self.soxr.process::<f32,f32>(None, as_interlaced(&mut obuf[..])) {
                if odone == 0 {
                    break;
                }
                obuf.truncate(odone);
                self.queue_sender.send(obuf);
            } else {
                panic!("Soxr somehow failed");
            }
        }
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
        let mut buf : [f32;2304] = [0.0;2304];
        let res = self.decoder.shit(&mut buf);
        match res {
            Err(e) => {
                println!("Encountered mp3 decode error at granule {}: {:?}", last_granule, e);
                return last_granule+1;
            },
            Ok((rate, channels, nsamples)) => {
                use sample::signal::Signal;
                if nsamples > 0 {
                    if channels != 2 {
                        panic!("Output must be in stereo! Got {} channels", channels);
                    }
                    
                    //let mut obuf : Vec<f32> = vec![0.0; (2304. * 48000. / rate as f64 + 0.5) as usize];
                    self.handle_samples(rate as f64, &buf[..])
                }
            },
        };
        last_granule + 1
    }

    fn notice_gap(&mut self) {}
    fn finish(&mut self) { self.handle_finish(); }
}

impl types::AudioCodec for Mp3DecoderFrontend {
    fn quality(&self) -> u32 { 240_000 }

    fn set_ringbuffer(&mut self, buffer: ringbuffer::Writer<types::Sample>) {
        self.ringbuffer = Some(buffer);
    }

    fn min_buffer_size(&self) -> u32 { 1152 }

    fn do_needful(&mut self) {
        
    }
}

pub fn try_start_stream<S: glium::Surface>(raw_header: &[u8]) -> Option<(Box<ogg::BitstreamDecoder>, types::StreamDesc<S>)> {
    use byteorder::{ByteOrder, LittleEndian};
    use libsoxr::datatype::Datatype as DT;
    use libsoxr::spec;
    if &raw_header[0..9] != b"OggMP3\0\0\0" {
        return None;
    }
    let aux_headers = raw_header[11] as usize;
    let sample_freq = LittleEndian::read_u32(&raw_header[16..20]) as u64;
    let (sq_sender, sq_receiver) = mpsc::channel();

    let decoder = Box::new(Mp3Decoder{
        soxr: libsoxr::Soxr::create(sample_freq as f64, 48000., 2,
                                    Some(spec::IOSpec::new(DT::Float32I, DT::Float32I)),
                                    Some(spec::QualitySpec::new(spec::QualityRecipe::VeryHigh, spec::QualityFlags::empty())),
                                    None).unwrap(),
        
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
            receiver: sq_receiver,
            ringbuffer: None,
        }))
    );

    Some((decoder, frontend))
}
