---
title: OggMP3 Specification
author: TQ Hirsch <thequux@thequux.com>
---

# DRAFT

OggMP3 is a direct mapping of MP3 (or really, any of `MPEG{1,2,2.5}
Layer {I,II,III}`) frames into Ogg packets. One frame appears per
packet; the granule position of a packet is the sample number of the
last sample in the packet. Thus, the first packet will have a granule
position equal to the number of samples per frame.

Muxers SHOULD strip CRC protection from encoded frames; Ogg provides
it for you.

All multi-byte values are encoded big-endian to align them with
network byte order.

## Header

| Offset | Length | Contents                         |
|--------|--------|----------------------------------|
|      0 |      8 | `OggMP3\0\0` (stream identifier)   |
|      8 |      1 | Format major version (0)         |
|      9 |      1 | Format minor version (0)         |
|     10 |      1 | Flags                            |
|     11 |      1 | Number of auxiliary Ogg headers  |
|      8 |      4 | Representative frame header      |

The major version is incremented upon incompatible changes. The minor
version is incrememnted upon compatible changes.

TODO: Define compatibility guarantees

The representative frame header is copied from the first frame, but
the first sync byte is inverted (i.e., zero). This prevents it from
being interpreted as a real MP3 frame.

## Flags

|     Bit | Meaning                                                |
|---------|--------------------------------------------------------|
| 0 (LSB) | Contains tag header packet (implies that field 3 >= 1) |
|       1 | 2-channel audio                                        |
|       2 | Shortened frame headers                                |


## Shortened frame headers

The 4-byte frame header on each encoded frame is replaced by a single
byte:

| Bits | Source bits | Meaning     |
|------|-------------|-------------|
|  7-4 |         7-4 | Stereo mode |
|  3-0 |       15-12 | Bit rate    |

When decoding, these values should be recombined with the
representative frame header from the stream header before passing the
frames to the underlying codec.

## Tag header

TODO: Define tag header

The tag header must be an ID3v1, ID3v2, or APE tag. A separate metadata
stream should be preferred to built-in tags.

## Notes

If shortened frame headers are not used and frames don't span pages,
the multiplexed file should be fully compatible(!) with legacy MP3
players.
