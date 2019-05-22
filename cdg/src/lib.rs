#![warn(missing_docs)]
#![allow(unknown_lints)]
//! A CD+G parser
//!
//! This documentation is probably best read alongside [CD+G Revealed](http://jbum.com/sware/cdg_revealed.txt)
//!
//! The CD+G format is very closely tied to the details of how compact
//! discs store audio; it's intended to be stored in the subchannel
//! data alongside normal Red Book audio. For the purposes of
//! consuming this data from a `.cdg` file using this library, all
//! that it is important to know is that the file is divided into
//! sectors of 96 bytes, each of which represents 1/75th of a second.
//!
//! The CD+G display model is a 300x216-pixel indexed color
//! framebuffer divided into 6x12-pixel tiles, with a 16-color
//! palette. The outermost cell on each side (i.e., the top and bottom
//! 12 rows and the left and right 6 columns) are drawn a solid
//! "border color" rather than drawn from the framebuffer.


use std::io::Read;
use std::fmt;

/// A 6x12 tile, to be blitted to the display
#[derive(Debug)]
pub struct Tile {
    /// X and Y coordinates, in tiles. 0,0 is the top left corner
    pub pos: (u8,u8), // x, then y
    /// The CLUT indices of the background and foreground colors
    pub color: (u8,u8),
    /// A 1bpp representation of the tile. Bytes represent rows; byte
    /// 0 is the top row. Within each byte/row, bit 5 (0x20) is the
    /// leftmost pixel and bit 0 (0x01) is the rightmost.
    pub content: [u8;12], // LSB is rightmost pixel; byte 0 is top
    /// The channel to display this tile on.
    pub channel: u8,
}

impl Tile {
    /// Convert a tile from the subchannel data found in a Tile command
    fn from(data: &[u8]) -> Self {
        if data.len() != 16 {
            panic!("Data is the wrong size");
        }
        let mut content = [0; 12];
        iter_copy(content[..].iter_mut(), data[4..16].iter().map(|x| x & 0x3F));
        Tile{
            pos: (data[3] & 0x3F, data[2] & 0x1F),
            color: (data[0] & 0x0F, data[1] & 0x0F), 
            content: content,
            // This channel interpretation is from CDGFix. I don't
            // have access to the real specs, so I don't know if it's
            // accurate.
            channel: (data[0] & 0x30) >> 2 | (data[1] & 0x30) >> 4,
        }
    }

    /// Return the CLUT index of the pixel at x,y
    pub fn get_pixel(&self, x: u8, y: u8) -> u8 {
        assert!(x < 6);
        assert!(y < 12);
        
        if self.content[y as usize] & (0x20 >> x) == 0 {
            self.color.0
        } else {
            self.color.1
        }
    }
}

/// A scroll command
#[derive(Eq,PartialEq,Debug,Copy,Clone)]
pub enum ScrollCommand {
    /// Don't scroll
    Noop,
    /// Scroll one tile up or to the left
    NW,
    /// Scroll one tile down or to the right
    SE,
}

impl ScrollCommand {
    fn from_u8(x: u8) -> Self {
        // Returns the scroll command from bits 4 and 5 of x
        match x & 0x30 {
            0x10 => ScrollCommand::SE,
            0x20 => ScrollCommand::NW,
            _ => ScrollCommand::Noop, // Invalid or NOOP
        }
    }
}

fn iter_copy<'a, T: Copy + 'a, DI: Iterator<Item=&'a mut T>, SI: Iterator<Item=T>>(dest: DI, src: SI) {
    for (dest,src) in dest.zip(src) {
        *dest = src;
    }
}

// Expand u4 to u8 filling the range.
fn expand4to8(x: u16) -> u8 {
    (x | x << 4) as u8
}

/// A 12-bit RGB color
#[derive(Copy,Eq,PartialEq,Clone)]
pub struct RgbColor(u16);

impl fmt::Debug for RgbColor {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_tuple("RGB")
            .field(&self.r())
            .field(&self.g())
            .field(&self.b())
            .finish()
    }
}

impl RgbColor {
    /// Convert the raw (i.e., pair of sixbits) RGB data from the CDG
    /// subchannel data into something slightly faster to compute
    /// with.
    fn from_subchannel(data0: u8, data1: u8) -> Self {
        // Remove the P and Q channels
        RgbColor((data0 as u16 & 0x3F) << 6 | (data1 as u16 & 0x3F))
    }

    /// Convert from an RGB triplet. The individual channels are each
    /// truncated to four bits.
    pub fn from_rgb(r: u8, g: u8, b: u8) -> RgbColor {
        let r = r as u16;
        let g = g as u16;
        let b = b as u16;

        RgbColor((r & 0xF0 << 4) | (g & 0xF0) | (b >> 4))
    }
    
    // This can be done very quickly via SSE; perhaps I'll implement that later
    /// The red component evenly scaled to 0..255
    pub fn r(&self) -> u8 {expand4to8((self.0 >> 8) & 0xF)}
    /// The green component evenly scaled to 0..255
    pub fn g(&self) -> u8 {expand4to8((self.0 >> 4) & 0xF)}
    /// The blue component evenly scaled to 0..255
    pub fn b(&self) -> u8 {expand4to8((self.0     ) & 0xF)}
}

/// One drawing command
#[allow(missing_docs)]
#[derive(Debug)]
pub enum Command {
    /// Clear the scren to `color`. This command will usually appear
    /// multiple times in a row with `repeat` incrementing each time,
    /// starting at 0. If you trust your CDG to be without errors, you
    /// can therefore ignore MemoryPreset commands with a nonzero
    /// repeat value
    MemoryPreset{color: u8, repeat: u8}, // 1
    /// Clear the border region to `color`. Documents vary as to
    /// whether the border region is the outside tile or the outside
    /// half-tile.
    BorderPreset{color: u8}, // 2
    /// Draw a tile normally
    TileNormal{tile: Tile},
    /// Draw a tile by XORing the color indices in the tile with the
    /// colors indices already drawn. Note that this does *not*
    /// operate on RGB values.
    TileXOR{tile: Tile},
    /// Scroll the screen by the given horizontal and vertical amount,
    /// respectively. `offset` is given as an absolute position within
    /// the first tile to begin scanout.
    ///
    /// If `color` is none, the tiles scrolled off one side of the
    /// framebuffer should be copied to the other side. Otherwise,
    /// they should be filled in with `color`
    Scroll{color: Option<u8>, cmd: (ScrollCommand, ScrollCommand), offset: (u8, u8)},
    /// Set one element of the CLUT to transparent, to enable background video/images
    SetTransparent{color: u8},
    /// Load one half of the CLUT. `offset` will be either 0 or 8,
    /// depending on whether the bottom or top half of the CLUT is to
    /// be loaded.
    ///
    /// These changes take effect immediately, so this can be used to
    /// implement color cycling.
    LoadPalette{offset: u8, clut: [RgbColor; 8]},
}

fn parse_scroll(data: &[u8], is_copy: bool) -> Command {
    if data.len() != 16 {
        panic!("INvalid data length");
    }
    let color = if is_copy { None } else { Some(data[0] & 0xF) };
    let h_scroll_cmd = ScrollCommand::from_u8(data[1]);
    let h_scroll_off = data[1] & 0x07;
    let v_scroll_cmd = ScrollCommand::from_u8(data[2]);
    let v_scroll_off = data[2] & 0x0F;
        
    Command::Scroll{
        color: color,
        cmd: (h_scroll_cmd, v_scroll_cmd),
        offset: (h_scroll_off, v_scroll_off),
    }
}

fn parse_clut(data: &[u8]) -> [RgbColor; 8] {
    let mut result = [RgbColor::from_subchannel(0,0); 8];
    iter_copy(result.iter_mut(), data.chunks(2).map(|c| RgbColor::from_subchannel(c[0], c[1])));
    result
}

/// Decode a single subchannel command. The input block must be
/// exactly 24 bytes long.  If the command is invalid for any reason,
/// return None. Otherwise, returns the command.
pub fn decode_subchannel_cmd(block: &[u8]) -> Option<Command> {
    if block.len() != 24 {
        return None
    }
    
    if block[0] & 0x3f != 9 {
        // command is not 9; this isn't a CD+G command
        return None;
    }

    let data = &block[4..20];
    // Iterator is now aligned to data[16]
    match block[1] & 0x3f {
        1 => Some(Command::MemoryPreset{color: data[0] & 0xF, repeat: data[1] & 0xF}),
        2 => Some(Command::BorderPreset{color: data[0] & 0xF}),
        6 => Some(Command::TileNormal{tile: Tile::from(data)}),
        38 => Some(Command::TileXOR{tile: Tile::from(data)}),
        20 => Some(parse_scroll(data, false)),
        24 => Some(parse_scroll(data, true)),
        28 => Some(Command::SetTransparent{color: data[0] & 0xF}),
        30 => Some(Command::LoadPalette{offset: 0, clut: parse_clut(data)}),
        31 => Some(Command::LoadPalette{offset: 8, clut: parse_clut(data)}),
        _ => None, // Invalid command
    }
}

/// Iterator over the blocks within a sector. This produces a stream
/// of `Command` objects, skipping over invalid commands and only
/// returning `None` when there are no more valid commands.
pub struct SectorIter<'a> {
    sector_iter: std::slice::Chunks<'a, u8>,
}

impl <'a> SectorIter<'a> {
    /// Create a new SectorIter from a sector buffer. The buffer must be at least 96 bytes long, and it must be a multiple of 24 bytes.
    pub fn new(sector: &'a [u8]) -> Self {
        assert!(sector.len() >= 96);
        assert!(sector.len() % 24 == 0);
        SectorIter{
            sector_iter: sector.chunks(24),
        }
    }
}
impl <'a> Iterator for SectorIter<'a> {
    type Item = Command;
    
    fn next(&mut self) -> Option<Self::Item> {
        match self.sector_iter.next() {
            None => None,
            Some(cmd) => decode_subchannel_cmd(cmd).or_else(|| self.next())
        }
    }
}

/// A streaming iterator over the sectors read in from a reader. The
/// interface is the same as `Iterator`, but the trait isn't
/// implemented because Rust doesn't have higher-kinded types yet.
///
/// # Examples
///
/// ```
/// let file = std::fs::File::open("/dev/null").unwrap();
/// let mut sector_iterator = cdg::SubchannelStreamIter::new(file);
/// while let Some(sector) = sector_iterator.next() {
///     for cmd in sector {
///         // Do something with command
///         println!("{:?}", cmd);
///     }
/// }
/// ```
pub struct SubchannelStreamIter<R: Read> {
    sector_buf: [u8; 96],
    reader: R,
}

impl <R: Read> SubchannelStreamIter<R> {
    /// Create a new subchannel stream iterator from a Reader
    pub fn new(reader: R) -> Self {
        SubchannelStreamIter{
            sector_buf: [0;96],
            reader: reader,
        }
    }
    
    /// Fetch the next sector from the input file.
    /// Returns None at EOF
    #[allow(should_implement_trait)] // We really should, but until Rust gets higher-kinded types, we can't.
    pub fn next(&mut self) -> Option<SectorIter> {
        match self.reader.read_exact(&mut self.sector_buf) {
            Ok(_) => Some(SectorIter::new(&self.sector_buf)),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
