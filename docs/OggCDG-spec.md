---
title: OggCDG Specification
author: TQ Hirsch <thequux@thequux.com>
---

# DRAFT

## Header

| Offset | Length | Contents                         |
|--------|--------|----------------------------------|
|      0 |      8 | `OggCDG\0\0` (stream identifier) |
|      8 |      1 | Format major version (0)         |
|      9 |      1 | Format minor version (0)         |
|     10 |      1 | Compression method               |
|     11 |      1 | Sectors per packet - 1           |

## Compression methods

| Number | Name           |
|--------|----------------|
|      0 | No compression |
|      1 | LZ4            |

## Packet format

Each packet (except the header) begins with a type byte.

### Type 0 (CDG commands)

Type 0 contains possibly compressed CDG commands. For this packet
type, the second byte contains the number of sectors that will result
from decompressing the packet contents. The rest of the packet is
compressed data. Each packet is individually compressed and must not
depend on previous packets to decompress.

### Type 1 (Keyframe)

Type 1 contains a keyframe, compressed using the same method indicated
in the header.

The decompressed keyframe is as follows:

| Offset | Length | Content                 |
|--------|--------|-------------------------|
|      0 |     32 | Palette                 |
|     32 |  32400 | Current bitmap          |
|  32432 |      1 | Transparent color index |

Each palette color is represented as it would be in the Load Palette
command in CDG (i.e, the first 16 bytes is the data section of a
Command 28, and the following 176 bytes are the data section of a
Command 30)

Following the palette is a bitmap stored in row-major order with two
pixels per byte. Within a byte, the high nibble appears to the left of
the low nibble.

Each keyframe MUST represent exactly the state that would result from
loading the previous keyframe and then executing the intervening
commands.

Support for generating and handling keyframes is optional in
implementations; they are strictly a performance optimization to speed
seeking. As even extremely naive implementations should be able to
process commands much faster than realtime, keyframes likely will not
have an effect until you have multi-hour CDG streams.

## Granule format

The high 44 bits of the granule number of a packet is the absolute
sector number (starting with 0) of the last sector in the page. The
low 20 bits are the absolute sector number of the last keyframe. There
is an implicit keyframe at offset 0 that contains a standard CGA
palette and a completely black screen (this is a reasonabke initial
state for CDG, and therefore does not need to appear in the
file).

## Content Type

The content type of an OggCDG stream SHALL be video/x-cdg
