use std::io::{self,Write};
use std::collections;
use rand;

use crc::crc32;

pub mod libogg;

// Low-level interface

lazy_static!{
    static ref CCITT_TABLE : [u32;256] = crc32::make_table(0x04c11db7);
}

pub struct Packet {
    pub content: Vec<u8>,
    pub timestamp: u64,
}

bitflags!{
    pub flags PageFlags: u8 {
        const PAGE_CTD = 1,
        const PAGE_BOS = 2,
        const PAGE_EOS = 4,
    }
}

pub struct Page {
    pub flags: PageFlags,
    pub granule_position: u64,
    pub stream_serial: u32,
    pub page_sequence: u32,
    /// The last element is the number of elements used
    pub segment_table: Vec<u8>, 
    pub content: Vec<u8>,
}

impl Page {
    pub fn new(stream_serial: u32, sequence: u32) -> Self {
        Page{
            flags: PageFlags::empty(),
            granule_position: !0,
            stream_serial: stream_serial,
            page_sequence: sequence,
            segment_table: Vec::new(),
            content: Vec::new(),
        }
    }

    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        use byteorder::{LittleEndian,ByteOrder};
        let mut header = [0;27];
        header[0..5].copy_from_slice(b"OggS\0");
        header[5] = self.flags.bits();
        LittleEndian::write_u64(&mut header[6..14], self.granule_position);
        LittleEndian::write_u32(&mut header[14..18], self.stream_serial);
        LittleEndian::write_u32(&mut header[18..22], self.page_sequence);
        // leave checksum in 22..26 blank
        header[26] = self.segment_table.len() as u8;

        let mut crc = crc32::update(0, &CCITT_TABLE, &header);
        crc = crc32::update(crc, &CCITT_TABLE, &self.segment_table);
        crc = crc32::update(crc, &CCITT_TABLE, &self.content);
        LittleEndian::write_u32(&mut header[22..26], crc);

        try!(writer.write_all(&header));
        try!(writer.write_all(&self.segment_table));
        try!(writer.write_all(&self.content));
        Ok(())
    }

    fn add_segment(&mut self, segment: &[u8]) {
        assert!(segment.len() < 256);
        assert!(self.segment_table.len() < 255);
        self.segment_table.push(segment.len() as u8);
        self.content.extend(segment);
    }

    fn has_space(&self) -> bool {
        self.segment_table.len() < 255
    }
    
    /// Add the part of packet that starts at offset.  If there is
    /// already data in this packet, offset MUST be 0.
    ///
    /// # Returns
    ///
    /// None: The packet was completely written and the next packet
    /// may be written immediately.
    ///
    /// Some(n): There was insufficent space to write the packet, and
    /// the same packet should be added to a new frame, passing n as
    /// offset.
    fn add_packet(&mut self, packet: &Packet, offset: usize) -> Option<usize> {
        let mut offset = offset;
        let plen = packet.content.len();
        if offset != 0 {
            assert!(self.segment_table.len() == 0);
            assert!(!self.flags.intersects(PAGE_BOS));
            self.flags |= PAGE_CTD;
        }
        while plen - offset > 254 && self.has_space() {
            // Write a 255-byte segment
            self.add_segment(&packet.content[offset..offset+255]);
            offset += 255;
        }
        if self.has_space() {
            // write final chunk
            assert!(plen - offset < 255);
            self.add_segment(&packet.content[offset..]);
            self.granule_position = packet.timestamp;
            return None;
        } else {
            return Some(offset);
        }
    }
}

// Page packer
pub struct PagePacker {
    page_queue : collections::VecDeque<Page>,

    // This is only none when the stream is closed
    active_page: Option<Page>,

    stream_serial: u32,
    page_sequence: u32,
}

impl PagePacker {
    pub fn new(serial: u32) -> Self {
        let mut start_page = Page::new(serial, 0);
        start_page.flags |= PAGE_BOS;
        PagePacker{
            page_queue: collections::VecDeque::new(),
            active_page: Some(start_page),
            stream_serial: serial,
            page_sequence: 0,
        }
    }

    // Immediately emit the current page and prepare the next one
    pub fn emit(&mut self) {
        // Emit a page and get the next page ready
        if let Some(page) = self.active_page.take() {
            let eos = page.flags.intersects(PAGE_EOS);
            self.page_queue.push_back(page);
            if !eos {
                self.page_sequence += 1;
                self.active_page = Some(Page::new(self.stream_serial, self.page_sequence));
            }
        }
    }

    fn get_active(&mut self) -> &mut Page {
        match self.active_page.as_mut() {
            None => panic!("Attempted to add packets to a closed stream"),
            Some(page) => page,
        }
    }
    
    pub fn add_packet(&mut self, packet: &Packet) {
        let mut offset = Some(0);
        while let Some(off) = offset {
            offset = self.get_active().add_packet(packet, off);
            if offset.is_some() {
                self.emit()
            }
        }
    }

    pub fn close(&mut self) {
        self.get_active().flags |= PAGE_EOS;
        self.emit();
        assert!(self.active_page.is_none());
    }

    pub fn is_closed(&self) -> bool {
        self.active_page.is_none()
    }
        
    pub fn peek_next(&self) -> Option<&Page> {
        self.page_queue.front()
    }

    pub fn take_next(&mut self) -> Option<Page> {
        self.page_queue.pop_front()
    }
}

pub trait BitstreamCoder {
    fn headers(&self) -> Vec<Vec<u8>>;
    fn next_frame(&mut self) -> io::Result<Option<Packet>>;

    /// Map a granule position to an absolute timestamp in µs
    fn map_granule(&self, u64) -> u64;
}

struct MuxStream {
    bitstream: Box<BitstreamCoder>,
    serial: u32,
    finished: bool,
    packer: PagePacker,
    
}

// Muxer
pub struct OgkMux {
    streams: Vec<MuxStream>,
}

impl OgkMux {
    pub fn new(interleave: u64) -> Self {
        OgkMux{
            streams: Vec::new(),
        }
    }

    pub fn add_stream(&mut self, stream: Box<BitstreamCoder>) {
        let serial = rand::random();
        self.streams.push(MuxStream{
            bitstream: stream,
            finished: false,
            serial: serial,
            packer: PagePacker::new(serial),
        })
    }

    pub fn write<W: io::Write>(&mut self, w: W) -> io::Result<()> {
        // Write headers...
        unimplemented!();
    }
}
