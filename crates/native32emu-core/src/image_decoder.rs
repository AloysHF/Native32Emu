// Image decoder for Native32 YUV and ARGB image formats.
// Decodes compressed image data into ARGB pixel buffers.

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u32>, // ARGB format, row-major
}

fn read_u16_le_slice(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32_le_slice(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn clip(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

/// Interpolate U/V vertically (2x upsampling).
fn interpolate_y(data: &[u8], w: usize, h: usize) -> Vec<u8> {
    let h1 = h * 2;
    let mut result = vec![0u8; w * h1];
    // Bounds-safe read: callers may pass dimensions slightly larger than the
    // source buffer for odd-sized images (e.g. menu thumbnails).
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    for y in 0..h {
        for dy in 0..2 {
            let y1 = y * 2 + dy;
            for x in 0..w {
                let val = if dy == 0 {
                    if y == 0 || get(y * w + x) != 0 {
                        get(y * w + x)
                    } else {
                        get((y - 1) * w + x)
                    }
                } else {
                    if y == h - 1 || get(y * w + x) != 0 {
                        get(y * w + x)
                    } else {
                        get((y + 1) * w + x)
                    }
                };
                result[y1 * w + x] = val;
            }
        }
    }
    result
}

/// Interpolate U/V horizontally (2x upsampling).
fn interpolate_x(data: &[u8], w: usize, h: usize) -> Vec<u8> {
    let w1 = w * 2;
    let mut result = vec![0u8; w1 * h];
    let get = |i: usize| data.get(i).copied().unwrap_or(0);
    for y in 0..h {
        for x in 0..w {
            for dx in 0..2 {
                let x1 = x * 2 + dx;
                let val = if dx == 0 {
                    if x == 0 || get(y * w + x) != 0 {
                        get(y * w + x)
                    } else {
                        get(y * w + (x - 1))
                    }
                } else {
                    if x == w - 1 || get(y * w + x) != 0 {
                        get(y * w + x)
                    } else {
                        get(y * w + (x + 1))
                    }
                };
                result[y * w1 + x1] = val;
            }
        }
    }
    result
}

/// Decode a YUV 4:2:0 image with packbits-like compression.
pub fn decode_image_yuv(data: &[u8]) -> Option<RgbaImage> {
    if data.len() < 8 {
        return None;
    }

    let width = read_u16_le_slice(data, 0) as usize;
    let height = read_u16_le_slice(data, 2) as usize;
    let img_size = read_u32_le_slice(data, 4) as usize;

    if width == 0 || height == 0 {
        return None;
    }

    let mut y_2_2 = vec![0u8; width * height];
    // 4:2:0 packs ceil(dim/2) chroma samples / quads per axis, so that an odd
    // width/height is fully covered by the last (clipped) quad. Using floor
    // here would make the decoder consume one fewer quad per row than the
    // encoder produced, drifting every row sideways and shearing the image
    // diagonally (notably the `.dat` menu name banners and previews).
    let uv_w = width.div_ceil(2);
    let uv_h = height.div_ceil(2);
    let mut u_1_1 = vec![0u8; uv_w * uv_h];
    let mut v_1_1 = vec![0u8; uv_w * uv_h];

    // Helper to put a 2x2 quad of Y values + U/V. Writes are bounds-checked so
    // that images with odd dimensions (e.g. menu thumbnails) cannot panic.
    let putquad =
        |pix: usize, y_buf: &mut [u8], u_buf: &mut [u8], v_buf: &mut [u8], chunk: &[u8]| {
            let y_coord = pix / uv_w;
            let x_coord = pix % uv_w;
            let mut set_y = |x: usize, y: usize, val: u8| {
                if x < width && y < height {
                    y_buf[y * width + x] = val;
                }
            };
            // Y values: x0y0, x0y1, x1y0, x1y1
            set_y(2 * x_coord, 2 * y_coord, chunk[0]);
            set_y(2 * x_coord, 2 * y_coord + 1, chunk[1]);
            set_y(2 * x_coord + 1, 2 * y_coord, chunk[2]);
            set_y(2 * x_coord + 1, 2 * y_coord + 1, chunk[3]);
            // U (Cb) is byte 4, V (Cr) is byte 5
            if pix < v_buf.len() {
                u_buf[pix] = chunk[4];
                v_buf[pix] = chunk[5];
            }
        };

    let mut pixel: usize = 0;
    let mut i: usize = 8;
    let max_pixels = uv_w * uv_h;

    while i + 2 <= data.len() && i < img_size + 8 && pixel < max_pixels {
        let op = read_u16_le_slice(data, i) as usize;
        if op == 0 {
            log::error!("Corrupted YUV image: command value 0 at offset 0x{:x}", i);
            return None;
        }
        i += 2;

        if op & 0x8000 != 0 {
            // N quads of literal data
            let count = op & 0x7FFF;
            for _ in 0..count {
                if i + 6 > data.len() || pixel >= max_pixels {
                    break;
                }
                putquad(pixel, &mut y_2_2, &mut u_1_1, &mut v_1_1, &data[i..i + 6]);
                pixel += 1;
                i += 6;
            }
        } else {
            // Repeat 1 quad N times
            if i + 6 > data.len() {
                break;
            }
            let chunk = [
                data[i],
                data[i + 1],
                data[i + 2],
                data[i + 3],
                data[i + 4],
                data[i + 5],
            ];
            i += 6;
            for _ in 0..op {
                if pixel >= max_pixels {
                    break;
                }
                putquad(pixel, &mut y_2_2, &mut u_1_1, &mut v_1_1, &chunk);
                pixel += 1;
            }
        }
    }

    // Upsample U/V: vertical first, then horizontal
    let u_2_2 = interpolate_x(&interpolate_y(&u_1_1, uv_w, uv_h), uv_w, height);
    let v_2_2 = interpolate_x(&interpolate_y(&v_1_1, uv_w, uv_h), uv_w, height);

    // Convert YUV to ARGB.
    //
    // The upsampled chroma buffers have stride `uv_w * 2`, which equals `width`
    // only when the width is even. For odd widths the chroma stride is
    // `width + 1`, so the chroma must be indexed by its own stride (clamping the
    // last column) instead of the luma width; otherwise the chroma drifts one
    // pixel further per row, tinting the image (typically green).
    let chroma_w = uv_w * 2;
    let mut pixels = vec![0u32; width * height];
    for y in 0..height {
        for x in 0..width {
            let li = y * width + x;
            if y_2_2[li] == 0 {
                // Transparent
                pixels[li] = 0x00000000;
                continue;
            }
            let cx = x.min(chroma_w.saturating_sub(1));
            let ci = y * chroma_w + cx;
            let c = y_2_2[li] as i32 - 16;
            let d = u_2_2.get(ci).copied().unwrap_or(128) as i32 - 128;
            let e = v_2_2.get(ci).copied().unwrap_or(128) as i32 - 128;
            let r = clip((298 * c + 409 * e + 128) >> 8);
            let g = clip((298 * c - 100 * d - 208 * e + 128) >> 8);
            let b = clip((298 * c + 516 * d + 128) >> 8);
            pixels[li] = 0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
        }
    }

    Some(RgbaImage {
        width: width as u32,
        height: height as u32,
        pixels,
    })
}

/// Decode an ARGB1555 image with run-length encoding.
pub fn decode_image_argb(data: &[u8]) -> Option<RgbaImage> {
    if data.len() < 8 {
        return None;
    }

    let width = read_u16_le_slice(data, 0) as usize;
    let height = read_u16_le_slice(data, 2) as usize;
    let img_size = read_u32_le_slice(data, 4) as usize;

    if width == 0 || height == 0 {
        return None;
    }

    let total_pixels = width * height;
    let mut pixels = vec![0u32; total_pixels];
    let mut pixel: usize = 0;
    let mut i: usize = 8;

    while i + 2 <= data.len() && i < img_size + 8 && pixel < total_pixels {
        let op = read_u16_le_slice(data, i);

        if op == 0x0000 {
            // Literal transparent pixel
            pixels[pixel] = 0x00000000;
            pixel += 1;
            i += 2;
        } else if op & 0xC000 == 0xC000 {
            // Repeat pixel N times
            let count = (op & 0x3FFF) as usize;
            if i + 4 > data.len() {
                break;
            }
            let value = read_u16_le_slice(data, i + 2);
            let argb = argb1555_to_argb(value);
            for _ in 0..count {
                if pixel >= total_pixels {
                    break;
                }
                pixels[pixel] = argb;
                pixel += 1;
            }
            i += 4;
        } else {
            log::error!("Unknown ARGB command 0x{:04x} at offset 0x{:06x}", op, i);
            return None;
        }
    }

    Some(RgbaImage {
        width: width as u32,
        height: height as u32,
        pixels,
    })
}

/// Convert an A1R5G5B5 (ARGB1555) pixel to ARGB8888.
///
/// The 16-bit value packs, from most to least significant:
/// alpha (bit 15), red (bits 14-10), green (bits 9-5), blue (bits 4-0).
fn argb1555_to_argb(value: u16) -> u32 {
    if value & 0x8000 == 0 {
        // Transparent
        0x00000000
    } else {
        let r = ((value >> 10) & 0x1F) << 3;
        let g = ((value >> 5) & 0x1F) << 3;
        let b = (value & 0x1F) << 3;
        0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === clip() tests ===

    #[test]
    fn test_clip_normal_values() {
        assert_eq!(clip(0), 0);
        assert_eq!(clip(128), 128);
        assert_eq!(clip(255), 255);
    }

    #[test]
    fn test_clip_clamps_low() {
        assert_eq!(clip(-1), 0);
        assert_eq!(clip(-1000), 0);
    }

    #[test]
    fn test_clip_clamps_high() {
        assert_eq!(clip(256), 255);
        assert_eq!(clip(1000), 255);
    }

    // === argb1555_to_argb() tests ===

    #[test]
    fn test_argb1555_transparent() {
        // bit 15 = 0 → transparent
        assert_eq!(argb1555_to_argb(0x0000), 0x00000000);
        assert_eq!(argb1555_to_argb(0x7FFF), 0x00000000); // bit 15 still 0
    }

    #[test]
    fn test_argb1555_white() {
        // bit 15 = 1, R=31, G=31, B=31
        // 5-bit max 31 → shifted left by 3 → 248 (0xF8), not 255
        assert_eq!(argb1555_to_argb(0xFFFF), 0xFFF8F8F8);
    }

    #[test]
    fn test_argb1555_red() {
        // bit 15 = 1, R=31 (bits 14-10), G=0, B=0
        // value = 0x8000 | (31 << 10) = 0xFC00
        let result = argb1555_to_argb(0xFC00);
        assert_eq!(result & 0xFF000000, 0xFF000000); // alpha = 0xFF
        assert_eq!((result >> 16) & 0xFF, 0xF8); // R = 31 << 3 = 248
        assert_eq!((result >> 8) & 0xFF, 0x00); // G = 0
        assert_eq!(result & 0xFF, 0x00); // B = 0
    }

    #[test]
    fn test_argb1555_green() {
        // bit 15 = 1, R=0, G=31, B=0
        // value = 0x8000 | (31 << 5) = 0x83E0
        let result = argb1555_to_argb(0x83E0);
        assert_eq!((result >> 16) & 0xFF, 0x00); // R = 0
        assert_eq!((result >> 8) & 0xFF, 0xF8); // G = 31 << 3 = 248
        assert_eq!(result & 0xFF, 0x00); // B = 0
    }

    #[test]
    fn test_argb1555_blue() {
        // bit 15 = 1, R=0, G=0, B=31 (bits 4-0)
        // value = 0x8000 | 31 = 0x801F
        let result = argb1555_to_argb(0x801F);
        assert_eq!((result >> 16) & 0xFF, 0x00); // R = 0
        assert_eq!((result >> 8) & 0xFF, 0x00); // G = 0
        assert_eq!(result & 0xFF, 0xF8); // B = 31 << 3 = 248
    }

    #[test]
    fn test_argb1555_exact_channel_layout() {
        // Pin the exact A1R5G5B5 bit layout so a future R/B mix-up is caught.
        assert_eq!(argb1555_to_argb(0xFC00), 0xFFF80000); // pure red
        assert_eq!(argb1555_to_argb(0x83E0), 0xFF00F800); // pure green
        assert_eq!(argb1555_to_argb(0x801F), 0xFF0000F8); // pure blue
    }

    #[test]
    fn test_argb1555_mixed_pixel() {
        // Mixed pixel: R=10, G=20, B=30.
        let value = 0x8000 | (10 << 10) | (20 << 5) | 30;
        assert_eq!(argb1555_to_argb(value), 0xFF50A0F0);
    }

    // === RgbaImage tests ===

    #[test]
    fn test_rgba_image_clone() {
        let img = RgbaImage {
            width: 2,
            height: 1,
            pixels: vec![0xFF000000, 0x00FF0000],
        };
        let img2 = img.clone();
        assert_eq!(img2.width, 2);
        assert_eq!(img2.pixels, vec![0xFF000000, 0x00FF0000]);
    }

    // === decode_image_yuv tests ===

    #[test]
    fn test_decode_yuv_empty_data() {
        assert!(decode_image_yuv(&[]).is_none());
    }

    #[test]
    fn test_decode_yuv_too_short() {
        // Less than 8 bytes header
        assert!(decode_image_yuv(&[0u8; 7]).is_none());
    }

    #[test]
    fn test_decode_yuv_zero_dimensions() {
        let mut data = vec![0u8; 16];
        // width = 0
        data[0..2].copy_from_slice(&0u16.to_le_bytes());
        data[2..4].copy_from_slice(&240u16.to_le_bytes());
        assert!(decode_image_yuv(&data).is_none());
    }

    #[test]
    fn test_decode_yuv_odd_width_chroma_alignment() {
        // Regression test: for odd widths the upsampled chroma stride differs
        // from the luma width. The chroma must be indexed by its own stride so
        // colors do not drift one pixel per row (which previously tinted images
        // green). Build a 5x2 image (uv_w=2) with two horizontally adjacent
        // quads carrying distinct, saturated chroma and verify each column has
        // the same color on both rows.
        let mut data = vec![0u8; 8 + 2 + 12];
        data[0..2].copy_from_slice(&5u16.to_le_bytes()); // width (odd)
        data[2..4].copy_from_slice(&2u16.to_le_bytes()); // height
        data[4..8].copy_from_slice(&14u32.to_le_bytes()); // data size
        data[8..10].copy_from_slice(&0x8002u16.to_le_bytes()); // literal: 2 quads
                                                               // quad 0: opaque luma + chroma (U=133, V=108)
        data[10..16].copy_from_slice(&[200, 200, 200, 200, 133, 108]);
        // quad 1: opaque luma + chroma (U=120, V=160)
        data[16..22].copy_from_slice(&[200, 200, 200, 200, 120, 160]);

        let img = decode_image_yuv(&data).expect("decode");
        assert_eq!(img.width, 5);
        assert_eq!(img.height, 2);

        let px = |x: usize, y: usize| img.pixels[y * 5 + x];
        // Each column's color must match between row 0 and row 1 (no drift).
        assert_eq!(px(0, 0), px(0, 1), "column 0 drifted between rows");
        assert_eq!(px(2, 0), px(2, 1), "column 2 drifted between rows");
        // The two quads carry different chroma, so their colors must differ.
        assert_ne!(px(0, 0), px(2, 0), "distinct chroma collapsed to one color");
    }

    #[test]
    fn test_decode_yuv_odd_dimensions_no_panic() {
        // Odd width and height must not panic during interpolation/conversion.
        let mut data = vec![0u8; 8 + 2 + 6];
        data[0..2].copy_from_slice(&3u16.to_le_bytes());
        data[2..4].copy_from_slice(&3u16.to_le_bytes());
        data[4..8].copy_from_slice(&8u32.to_le_bytes());
        data[8..10].copy_from_slice(&0x8001u16.to_le_bytes());
        data[10..16].copy_from_slice(&[200, 200, 200, 200, 130, 120]);
        let img = decode_image_yuv(&data).expect("decode");
        assert_eq!((img.width, img.height), (3, 3));
    }

    #[test]
    fn test_decode_yuv_minimal_image() {
        // 2x2 YUV image (minimum valid: uv_w=1, uv_h=1)
        let mut data = vec![0u8; 64];
        data[0..2].copy_from_slice(&2u16.to_le_bytes()); // width=2
        data[2..4].copy_from_slice(&2u16.to_le_bytes()); // height=2
                                                         // img_size = enough for 1 pixel quad (6 bytes) + header offset
        data[4..8].copy_from_slice(&12u32.to_le_bytes());

        // Command: literal 1 quad (0x8000 | 1 = 0x8001)
        data[8..10].copy_from_slice(&0x8001u16.to_le_bytes());
        // 6 bytes of YUV data: Y0, Y1, Y2, Y3, U, V
        data[10] = 128; // Y0
        data[11] = 128; // Y1
        data[12] = 128; // Y2
        data[13] = 128; // Y3
        data[14] = 128; // U
        data[15] = 128; // V

        let result = decode_image_yuv(&data);
        assert!(result.is_some());
        let img = result.unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
        assert_eq!(img.pixels.len(), 4);
    }

    // === decode_image_argb tests ===

    #[test]
    fn test_decode_argb_empty_data() {
        assert!(decode_image_argb(&[]).is_none());
    }

    #[test]
    fn test_decode_argb_too_short() {
        assert!(decode_image_argb(&[0u8; 7]).is_none());
    }

    #[test]
    fn test_decode_argb_zero_dimensions() {
        let mut data = vec![0u8; 16];
        data[0..2].copy_from_slice(&0u16.to_le_bytes()); // width=0
        data[2..4].copy_from_slice(&2u16.to_le_bytes()); // height=2
        assert!(decode_image_argb(&data).is_none());
    }

    #[test]
    fn test_decode_argb_transparent_pixel() {
        let mut data = vec![0u8; 32];
        data[0..2].copy_from_slice(&1u16.to_le_bytes()); // width=1
        data[2..4].copy_from_slice(&1u16.to_le_bytes()); // height=1
        data[4..8].copy_from_slice(&2u32.to_le_bytes()); // img_size=2 (1 command)

        // Command: 0x0000 = literal transparent pixel
        data[8..10].copy_from_slice(&0x0000u16.to_le_bytes());

        let result = decode_image_argb(&data);
        assert!(result.is_some());
        let img = result.unwrap();
        assert_eq!(img.pixels[0], 0x00000000); // transparent
    }

    #[test]
    fn test_decode_argb_repeated_pixel() {
        let mut data = vec![0u8; 32];
        data[0..2].copy_from_slice(&3u16.to_le_bytes()); // width=3
        data[2..4].copy_from_slice(&1u16.to_le_bytes()); // height=1
        data[4..8].copy_from_slice(&4u32.to_le_bytes()); // img_size=4 (1 command + 1 pixel)

        // Command: 0xC000 | 3 = repeat 3 times
        data[8..10].copy_from_slice(&(0xC000u16 | 3).to_le_bytes());
        // ARGB1555 value: white (0xFFFF) → ARGB8888 = 0xFFF8F8F8 (5-bit max 31→248)
        data[10..12].copy_from_slice(&0xFFFFu16.to_le_bytes());

        let result = decode_image_argb(&data);
        assert!(result.is_some());
        let img = result.unwrap();
        assert_eq!(img.pixels.len(), 3);
        for pixel in &img.pixels {
            assert_eq!(*pixel, 0xFFF8F8F8); // white: 5-bit 31<<3=248 per channel
        }
    }

    #[test]
    fn test_decode_argb_red_pixel_layout() {
        // Regression test: a 1x1 image holding an A1R5G5B5 red pixel must
        // decode to red, not blue (the channels used to be swapped).
        let mut data = vec![0u8; 32];
        data[0..2].copy_from_slice(&1u16.to_le_bytes()); // width=1
        data[2..4].copy_from_slice(&1u16.to_le_bytes()); // height=1
        data[4..8].copy_from_slice(&4u32.to_le_bytes()); // img_size=4

        // Command: 0xC000 | 1 = repeat 1 time
        data[8..10].copy_from_slice(&(0xC000u16 | 1).to_le_bytes());
        data[10..12].copy_from_slice(&0xFC00u16.to_le_bytes()); // red

        let img = decode_image_argb(&data).expect("decode");
        assert_eq!(img.pixels[0], 0xFFF80000);
    }

    #[test]
    fn test_decode_argb_unknown_command_returns_none() {
        let mut data = vec![0u8; 32];
        data[0..2].copy_from_slice(&1u16.to_le_bytes()); // width=1
        data[2..4].copy_from_slice(&1u16.to_le_bytes()); // height=1
        data[4..8].copy_from_slice(&2u32.to_le_bytes()); // img_size=2

        // Unknown command: 0x4000 (not 0x0000 and not 0xC000 prefix)
        data[8..10].copy_from_slice(&0x4000u16.to_le_bytes());

        let result = decode_image_argb(&data);
        assert!(result.is_none());
    }

    // === interpolate_y tests ===

    #[test]
    fn test_interpolate_y_identity_for_uniform() {
        // Uniform non-zero data should pass through
        let data = vec![128u8; 4]; // 2x2
        let result = interpolate_y(&data, 2, 2);
        assert_eq!(result.len(), 8); // 2x4
                                     // All values should be 128
        for v in &result {
            assert_eq!(*v, 128);
        }
    }

    #[test]
    fn test_interpolate_y_doubles_height() {
        let data = vec![10u8; 6]; // 3x2
        let result = interpolate_y(&data, 3, 2);
        assert_eq!(result.len(), 12); // 3x4
    }

    // === interpolate_x tests ===

    #[test]
    fn test_interpolate_x_identity_for_uniform() {
        let data = vec![128u8; 4]; // 2x2
        let result = interpolate_x(&data, 2, 2);
        assert_eq!(result.len(), 8); // 4x2
        for v in &result {
            assert_eq!(*v, 128);
        }
    }

    #[test]
    fn test_interpolate_x_doubles_width() {
        let data = vec![10u8; 6]; // 3x2
        let result = interpolate_x(&data, 3, 2);
        assert_eq!(result.len(), 12); // 6x2
    }

    // === read_u16_le_slice tests ===

    #[test]
    fn test_read_u16_le_slice() {
        assert_eq!(read_u16_le_slice(&[0x34, 0x12], 0), 0x1234);
        assert_eq!(read_u16_le_slice(&[0x00, 0x34, 0x12, 0x00], 1), 0x1234);
    }

    #[test]
    fn test_read_u32_le_slice() {
        assert_eq!(read_u32_le_slice(&[0x78, 0x56, 0x34, 0x12], 0), 0x12345678);
    }
}
