extern crate cdg;
extern crate image;

use cdg::RgbColor;
use std::ops::{Index,IndexMut,Fn,Add};

pub trait One {
    fn one() -> Self;
}

impl One for u16 {
    fn one() -> Self { 1 }
}
impl One for u8 {
    fn one() -> Self { 1 }
}

#[derive(Clone,Copy,Debug)]
pub struct Position<T> {
    x: T,
    y: T,
}

impl <T> Position<T> {
    pub fn new(x: T, y: T) -> Self {
        Position{x: x, y: y}
    }
}

#[derive(Copy,Clone,Debug)]
pub struct Rectangle<T> {
    pub nw: Position<T>,
    pub se: Position<T>,
}

impl <T: Ord + Copy + Add<Output=T> + One> Rectangle<T> {
    pub fn new(p0: Position<T>, p1: Position<T>) -> Self {
        use std::cmp::{min,max};
        Rectangle{nw: Position{ x: min(p0.x, p1.x), y: min(p0.y, p1.y)},
                  se: Position{ x: max(p0.x, p1.x), y: max(p0.y, p1.y)}}
    }
    pub fn expand(&self, p: Position<T>) -> Self {
        use std::cmp::{min,max};
        Rectangle{nw: Position{ x: min(self.nw.x, p.x), y: min(self.nw.y, p.y)},
                  se: Position{ x: max(self.se.x, p.x+T::one()), y: max(self.se.y, p.y+T::one())}}
    }
}

const TILE_ROWS: usize = 18;
const TILE_COLS: usize = 50;

pub struct CdgInterpreter {
    tile_shift: Position<u16>,
    pixel_shift: Position<u16>,
    clut: [cdg::RgbColor; 16],
    dirty: Option<Rectangle<u16>>, // in tiles
    content: [[u8;300];216],
    border: u8,
    transparent: u8, // is 0..15 if a color is transparent, 0xff if not
}

struct TileView<'a> {
    interp: &'a mut CdgInterpreter,
    x: usize, // both x and y are in pixels
    y: usize,
}

impl <'a> TileView<'a> {
    // takes x,y, old_val
    fn map_pixels<F>(&mut self, func: F)
        where F : Fn(u8,u8, &mut u8) {
        for y in 0..12 {
            for x in 0..6 {
                func(x,y, &mut self[(x,y)])
            }
        }
    }
    
    fn draw_normal(&mut self, tile: &cdg::Tile) {
        self.map_pixels(|x,y, px| *px = tile.get_pixel(x,y))
    }

    fn draw_xor(&mut self, tile: &cdg::Tile) {
        self.map_pixels(|x,y, px| *px ^= tile.get_pixel(x,y))
    }
}

impl <'b> Index<(u8,u8)> for TileView<'b> {
    type Output = u8;
    fn index(&self, pos: (u8,u8)) -> &u8 {
        assert!(pos.0 < 6 && pos.1 < 12);
        unsafe {
            self.interp.content.get_unchecked(self.y + pos.1 as usize).get_unchecked(self.x + pos.0 as usize)
        }
    }
}

impl <'b> IndexMut<(u8,u8)> for TileView<'b> {
    fn index_mut(&mut self, pos: (u8,u8)) -> &mut u8 {
        assert!(pos.0 < 6 && pos.1 < 12);
        unsafe {
            self.interp.content.get_unchecked_mut(self.y + pos.1 as usize).get_unchecked_mut(self.x + pos.0 as usize)
        }
    }
}

// Default to CGA-ish palette
fn default_colors() -> [RgbColor; 16] {
    [
        RgbColor::from_rgb(0, 0, 0),
        RgbColor::from_rgb(0, 0, 170),
        RgbColor::from_rgb(0, 170, 0),
        RgbColor::from_rgb(0, 170, 170),
        RgbColor::from_rgb(170, 0, 0),
        RgbColor::from_rgb(170, 0, 170),
        RgbColor::from_rgb(170, 170, 0),
        RgbColor::from_rgb(170, 170, 170),
        RgbColor::from_rgb(85, 85, 85),
        RgbColor::from_rgb(85, 85, 255),
        RgbColor::from_rgb(85, 255, 85),
        RgbColor::from_rgb(85, 255, 255),
        RgbColor::from_rgb(255, 85, 85),
        RgbColor::from_rgb(255, 85, 255),
        RgbColor::from_rgb(255, 255, 85),
        RgbColor::from_rgb(255, 255, 255),
    ]
}

/// Basic accessors, constructors
impl CdgInterpreter {
    pub fn new() -> Self {
        CdgInterpreter {
            tile_shift: Position::new(0,0),
            pixel_shift: Position::new(0,0),
            clut: default_colors(),
            dirty: Some(Rectangle::new(Position::new(0,0),
                                       Position::new(50,18))),
            content: [[0;300];216],
            border: 0,
            transparent: 255,
        }
    }
    #[allow(unused)]
    fn map_pxrow(&self, row: usize) -> usize {
        (row + self.tile_shift.y as usize * 12) % 216
    }

    #[allow(unused)]
    fn map_pxcol(&self, row: usize) -> usize {
        (row + self.tile_shift.x as usize * 6) % 300
    }

    fn map_trow(&self, row: usize) -> usize {
        (row + self.tile_shift.y as usize) % TILE_ROWS
    }

    fn map_tcol(&self, col: usize) -> usize {
        (col + self.tile_shift.x as usize) % TILE_COLS
    }

    fn clear_col(&mut self, n: usize, color: Option<u8>) {
        match color {
            None => (),
            Some(color) => {
                let col = self.map_tcol(n) * 6;
                for r in 0..216 {
                    for c in col..col+6 {
                        unsafe {*self.content.get_unchecked_mut(r).get_unchecked_mut(c) = color; }
                    }
                }
            }
        }
    }

    fn clear_row(&mut self, n: usize, color: Option<u8>) {
        match color {
            None => (),
            Some(color) => {
                let col = self.map_trow(n) * 12;
                for c in col..col+12 {
                    for r in 0..216 {
                        unsafe {*self.content.get_unchecked_mut(r).get_unchecked_mut(c) = color; }
                    }
                }
            }
        }
    }    
    
    fn get_tile(&mut self, pos: (u8,u8)) -> TileView {
        let x = (pos.0 as u16 + self.tile_shift.x) as usize % TILE_COLS;
        let y = (pos.1 as u16 + self.tile_shift.y) as usize % TILE_ROWS;

        TileView{
            interp: self,
            x: x * 6,
            y: y * 12,
        }
    }

    pub fn dirty(&self) -> Option<Rectangle<u16>> {
        self.dirty
    }

    fn invalidate_tile(&mut self, pos: (u8, u8)) {
        let new_tile = Position::new(pos.0 as u16, pos.1 as u16);
        let new_tilep = Position::new(pos.0 as u16 + 1, pos.1 as u16 + 1);
        self.dirty = self.dirty
            .map(|x| x.expand(new_tile))
            .or_else(|| Some(Rectangle::new(new_tile,new_tilep)));
    }

    fn invalidate_all(&mut self) {
        self.dirty = Some(Rectangle::new(Position::new(0,0),
                                         Position::new(50,18)));
    }

    /// Mark the entire region clean
    pub fn clear_dirty_region(&mut self) {
        self.dirty = None;
    }

    // Can be used by decoder when seeking for example
    pub fn reset(&mut self, reset_color: bool) {
        self.tile_shift = Position::new(0,0);
        self.pixel_shift = Position::new(0,0);
        self.dirty = Some(Rectangle::new(Position::new(0,0),
                                       Position::new(50,18)));
        self.content = [[0;300];216];
        self.border = 0;
        self.transparent = 255;

        if reset_color {
            self.clut = default_colors();
        }
    }

    /*
    // For performance reasons, always produces an RGBA image
    fn scanout<Image>(&self, image: &mut Image, region: Rectangle<u16>) {
        for y in region.nw.y as usize..region.se.y as usize {
            for x in region.nw.x as usize..region.se.x as usize {
                pos = (y * stride + x) * 4;
                buffer[y * stride + x] = 
            }
        }
    }
     */
}

impl image::GenericImageView for CdgInterpreter {
    type Pixel = image::Rgba<u8>;
    type InnerImageView = Self;

    fn dimensions(&self) -> (u32,u32) {
        (300,216)
    }

    fn bounds(&self) -> (u32,u32,u32,u32) {
        (0, 0, 300, 216)
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        use image::Pixel;
        let cindex = self.content[self.map_pxrow(y as usize)][self.map_pxcol(x as usize)];
        let c = self.clut[cindex as usize];
        if self.transparent == cindex {
            image::Rgba::from_channels(c.r(), c.g(), c.b(), 0)
        } else {
            image::Rgba::from_channels(c.r(), c.g(), c.b(), 255)
        }
    }

    fn inner(&self) -> &Self::InnerImageView {
        self
    }
}

impl CdgInterpreter {
    pub fn handle_cmd(&mut self, command: cdg::Command) {
        use cdg::Command::*;
        match command {
            MemoryPreset{color, repeat: 0} => {
                for y in 0..216 {
                    for x in 0..300 {
                        self.content[y][x] = color;
                    }
                }
                self.invalidate_all();
            },
            MemoryPreset{..} => (),
            BorderPreset{color} => {self.border = color; self.invalidate_all(); },
            TileNormal{tile} => { self.get_tile(tile.pos).draw_normal(&tile); self.invalidate_tile(tile.pos); },
            TileXOR{tile} => { self.get_tile(tile.pos).draw_xor(&tile); self.invalidate_tile(tile.pos); },
            Scroll{color, cmd: (xc, yc), offset: (xo,yo)} => {
                use cdg::ScrollCommand::{NW,SE,Noop};
                // Handle horizontal scrolling first
                match xc {
                    NW => {
                        self.clear_col(0, color);
                        self.tile_shift.x = (self.tile_shift.x + 1) % TILE_COLS as u16;
                    }
                    SE => {
                        self.tile_shift.x = (self.tile_shift.x + TILE_COLS as u16 - 1) % TILE_COLS as u16;
                        self.clear_col(0, color);
                    }
                    Noop => (),
                }
                match yc {
                    NW => {
                        self.clear_row(0, color);
                        self.tile_shift.y = (self.tile_shift.y + 1) % TILE_ROWS as u16;
                    }
                    SE => {
                        self.tile_shift.y = (self.tile_shift.y + TILE_ROWS as u16 - 1) % TILE_ROWS as u16;
                        self.clear_row(0, color);
                    }
                    Noop => (),
                }
                self.pixel_shift = Position::new(xo as u16 % 6, yo as u16 % 12);
                self.invalidate_all();
            },
            SetTransparent{color} => {self.transparent = color; self.invalidate_all(); },
            LoadPalette{offset, clut} => {
                let off = offset as usize;
                self.clut[off..off+8].copy_from_slice(&clut);
                self.invalidate_all();
            }
            
        }
    }    
}

impl Default for CdgInterpreter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
