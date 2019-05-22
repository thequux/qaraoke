use ogk::ogg;
use mpg123;
use types;
use glium;
use rt::ringbuffer;
use std::sync::mpsc;
use std::vec;
use std::os::raw as ostyp;
use soxr;

struct Mp3Decoder {
    queue_sender: mpsc::Sender<Vec<types::Sample>>,
    decoder: mpg123::Handle<f32>,
    sample_frequency: u32,
    aux_headers: usize,
    soxr: soxr::Soxr<types::Sample, types::Sample>,
}

struct Mp3DecoderFrontend {
    receiver: mpsc::Receiver<Vec<types::Sample>>,
    ringbuffer: Option<ringbuffer::Writer<types::Sample>>,
    queued_samples: Option<vec::IntoIter<types::Sample>>,
}

fn as_interlaced<T>(buf: &mut [[T; 2]]) -> &mut [T] {
    use std::slice;
    unsafe {
        slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut T, buf.len() * 2)
    }
}

fn as_mut_frames<T>(buf: &mut [T]) -> &mut [[T;2]] {
    use std::slice;
    unsafe {
        slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut [T;2], buf.len() / 2)
    }
}

fn as_frames<T>(buf: &[T]) -> &[[T;2]] {
    use std::slice;
    unsafe {
        slice::from_raw_parts(buf.as_ptr() as *const [T;2], buf.len() / 2)
    }
}


impl Mp3Decoder {
    fn handle_samples(&mut self, rate: u32, buf: &[f32]) {
        let mut obuf : Vec<[f32;2]> = vec![[0.0;2]; (buf.len() as f64 / 2. * 48000. / rate as f64 + 0.5) as usize];
        //self.queue_sender.send(obuf);  return;
        if rate != self.sample_frequency {
            self.sample_frequency = rate;
            self.soxr.change_rate(rate as f64, 48000., 0).unwrap();
        }
        // This function is deceptively unsafe; regardless of type
        // parameters, it will always use the types given in the initializer (f32/f32)
        if let Ok(done) = self.soxr.process(Some(as_frames(buf)), &mut obuf[..]) {
            obuf.truncate(done);
            self.queue_sender.send(obuf).ok();
        } else {
            panic!("Soxr somehow failed");
        }
    }

    fn handle_finish(&mut self) {
        loop {
            let mut obuf = vec![[0.;2]; 512];
            if let Ok(odone) = self.soxr.process(None, &mut obuf[..]) {
                if odone == 0 {
                    break;
                }
                obuf.truncate(odone);
                self.queue_sender.send(obuf).ok();
            } else {
                panic!("Soxr somehow failed");
            }
        }
    }
}

impl ogg::BitstreamDecoder for Mp3Decoder {
    fn map_granule(&self, timestamp: u64) -> u64 { 1000_000 * timestamp / self.sample_frequency as u64 }
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
                if nsamples > 0 {
                    if channels != 2 {
                        panic!("Output must be in stereo! Got {} channels", channels);
                    }
                    
                    //let mut obuf : Vec<f32> = vec![0.0; (2304. * 48000. / rate as f64 + 0.5) as usize];
                    self.handle_samples(rate, &buf[..]);
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
        if self.ringbuffer.is_none() {
            return
        }
        let drop_ringbuffer;
        {
            let mut obuf = self.ringbuffer.as_mut().unwrap().extender();
            
            loop {
                if let Some(mut it) = self.queued_samples.take() {
                    obuf.extend(&mut it);
                    if it.len() != 0 {
                        // There's more there
                        self.queued_samples = Some(it);
                        return;
                    }
                }
                match self.receiver.try_recv() {
                    Ok(ibuf) => self.queued_samples = Some(ibuf.into_iter()),
                    Err(mpsc::TryRecvError::Disconnected) => {
                        drop_ringbuffer = true;
                        break;
                    },
                    Err(mpsc::TryRecvError::Empty) => {
                        drop_ringbuffer = false;
                        break;
                    }
                }
            }
        }
        if drop_ringbuffer {
            self.ringbuffer.take();
        }
    }
}

pub fn try_start_stream<S: glium::Surface>(raw_header: &[u8]) -> Option<(Box<ogg::BitstreamDecoder>, types::StreamDesc<S>)> {
    use byteorder::{ByteOrder, LittleEndian};
    if &raw_header[0..9] != b"OggMP3\0\0\0" {
        return None;
    }
    let aux_headers = raw_header[11] as usize;
    let sample_freq = LittleEndian::read_u32(&raw_header[16..20]);
    let (sq_sender, sq_receiver) = mpsc::channel();

    // I would like to pass VR as the only quality flag to neable
    // variable sample rate changing. However, that slows resampling down significantly
    let soxr = soxr::SoxrBuilder::new()
        .set_quality(soxr::sys::SOXR_QQ, soxr::sys::soxr_quality_flags::empty())
        .build()
        .unwrap();
    
    let decoder = Box::new(Mp3Decoder{
        soxr: soxr,
        
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
            queued_samples: None,
        }))
    );

    Some((decoder, frontend))
}
