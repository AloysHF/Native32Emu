// MPEG-1 video decoder.
//
// Ported to Rust from PL_MPEG (https://github.com/phoboslab/pl_mpeg) by Dominic
// Szablewski, originally MIT licensed; itself inspired by Zoltan Korandi's Java
// MPEG-1 decoder. Decodes an MPEG-1 video elementary stream into YCrCb frames.

use super::buffer::Buffer;

const PICTURE_TYPE_INTRA: i32 = 1;
const PICTURE_TYPE_PREDICTIVE: i32 = 2;
const PICTURE_TYPE_B: i32 = 3;

const START_SEQUENCE: i32 = 0xB3;
const START_SLICE_FIRST: i32 = 0x01;
const START_SLICE_LAST: i32 = 0xAF;
const START_PICTURE: i32 = 0x00;
const START_EXTENSION: i32 = 0xB5;
const START_USER_DATA: i32 = 0xB2;

fn is_slice_start_code(c: i32) -> bool {
    (START_SLICE_FIRST..=START_SLICE_LAST).contains(&c)
}

const PIXEL_ASPECT_RATIO: [f64; 14] = [
    1.0000, 0.6735, 0.7031, 0.7615, 0.8055, 0.8437, 0.8935, 0.9157, 0.9815, 1.0255, 1.0695, 1.0950,
    1.1575, 1.2051,
];

const PICTURE_RATE: [f64; 16] = [
    0.000, 23.976, 24.000, 25.000, 29.970, 30.000, 50.000, 59.940, 60.000, 0.000, 0.000, 0.000,
    0.000, 0.000, 0.000, 0.000,
];

const ZIG_ZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

const INTRA_QUANT_MATRIX: [u8; 64] = [
    8, 16, 19, 22, 26, 27, 29, 34, 16, 16, 22, 24, 27, 29, 34, 37, 19, 22, 26, 27, 29, 34, 34, 38,
    22, 22, 26, 27, 29, 34, 37, 40, 22, 26, 27, 29, 32, 35, 40, 48, 26, 27, 29, 32, 35, 40, 48, 58,
    26, 27, 29, 34, 38, 46, 56, 69, 27, 29, 35, 38, 46, 56, 69, 83,
];

const NON_INTRA_QUANT_MATRIX: [u8; 64] = [16; 64];

const PREMULTIPLIER_MATRIX: [i32; 64] = [
    32, 44, 42, 38, 32, 25, 17, 9, 44, 62, 58, 52, 44, 35, 24, 12, 42, 58, 55, 49, 42, 33, 23, 12,
    38, 52, 49, 44, 38, 30, 20, 10, 32, 44, 42, 38, 32, 25, 17, 9, 25, 35, 33, 30, 25, 20, 14, 7,
    17, 24, 23, 20, 17, 14, 9, 5, 9, 12, 12, 10, 9, 7, 5, 2,
];

// VLC tables. Each entry is (index, value): index > 0 points to the next node
// (`index + bit`), index <= 0 terminates with `value`.

const MACROBLOCK_ADDRESS_INCREMENT: &[(i16, i16)] = &[
    (2, 0),
    (0, 1),
    (4, 0),
    (6, 0),
    (8, 0),
    (10, 0),
    (0, 3),
    (0, 2),
    (12, 0),
    (14, 0),
    (0, 5),
    (0, 4),
    (16, 0),
    (18, 0),
    (0, 7),
    (0, 6),
    (20, 0),
    (22, 0),
    (24, 0),
    (26, 0),
    (28, 0),
    (30, 0),
    (32, 0),
    (34, 0),
    (36, 0),
    (38, 0),
    (0, 9),
    (0, 8),
    (-1, 0),
    (40, 0),
    (-1, 0),
    (42, 0),
    (44, 0),
    (46, 0),
    (0, 15),
    (0, 14),
    (0, 13),
    (0, 12),
    (0, 11),
    (0, 10),
    (48, 0),
    (50, 0),
    (52, 0),
    (54, 0),
    (56, 0),
    (58, 0),
    (60, 0),
    (62, 0),
    (64, 0),
    (-1, 0),
    (-1, 0),
    (66, 0),
    (68, 0),
    (70, 0),
    (72, 0),
    (74, 0),
    (76, 0),
    (78, 0),
    (0, 21),
    (0, 20),
    (0, 19),
    (0, 18),
    (0, 17),
    (0, 16),
    (0, 35),
    (-1, 0),
    (-1, 0),
    (0, 34),
    (0, 33),
    (0, 32),
    (0, 31),
    (0, 30),
    (0, 29),
    (0, 28),
    (0, 27),
    (0, 26),
    (0, 25),
    (0, 24),
    (0, 23),
    (0, 22),
];

const MACROBLOCK_TYPE_INTRA: &[(i16, i16)] = &[(2, 0), (0, 0x01), (-1, 0), (0, 0x11)];

const MACROBLOCK_TYPE_PREDICTIVE: &[(i16, i16)] = &[
    (2, 0),
    (0, 0x0a),
    (4, 0),
    (0, 0x02),
    (6, 0),
    (0, 0x08),
    (8, 0),
    (10, 0),
    (12, 0),
    (0, 0x12),
    (0, 0x1a),
    (0, 0x01),
    (-1, 0),
    (0, 0x11),
];

const MACROBLOCK_TYPE_B: &[(i16, i16)] = &[
    (2, 0),
    (4, 0),
    (6, 0),
    (8, 0),
    (0, 0x0c),
    (0, 0x0e),
    (10, 0),
    (12, 0),
    (0, 0x04),
    (0, 0x06),
    (14, 0),
    (16, 0),
    (0, 0x08),
    (0, 0x0a),
    (18, 0),
    (20, 0),
    (0, 0x1e),
    (0, 0x01),
    (-1, 0),
    (0, 0x11),
    (0, 0x16),
    (0, 0x1a),
];

fn macroblock_type_table(picture_type: i32) -> &'static [(i16, i16)] {
    match picture_type {
        PICTURE_TYPE_INTRA => MACROBLOCK_TYPE_INTRA,
        PICTURE_TYPE_PREDICTIVE => MACROBLOCK_TYPE_PREDICTIVE,
        _ => MACROBLOCK_TYPE_B,
    }
}

const CODE_BLOCK_PATTERN: &[(i16, i16)] = &[
    (2, 0),
    (4, 0),
    (6, 0),
    (8, 0),
    (10, 0),
    (12, 0),
    (14, 0),
    (16, 0),
    (18, 0),
    (20, 0),
    (22, 0),
    (24, 0),
    (26, 0),
    (0, 60),
    (28, 0),
    (30, 0),
    (32, 0),
    (34, 0),
    (36, 0),
    (38, 0),
    (40, 0),
    (42, 0),
    (44, 0),
    (46, 0),
    (0, 32),
    (0, 16),
    (0, 8),
    (0, 4),
    (48, 0),
    (50, 0),
    (52, 0),
    (54, 0),
    (56, 0),
    (58, 0),
    (60, 0),
    (62, 0),
    (0, 62),
    (0, 2),
    (0, 61),
    (0, 1),
    (0, 56),
    (0, 52),
    (0, 44),
    (0, 28),
    (0, 40),
    (0, 20),
    (0, 48),
    (0, 12),
    (64, 0),
    (66, 0),
    (68, 0),
    (70, 0),
    (72, 0),
    (74, 0),
    (76, 0),
    (78, 0),
    (80, 0),
    (82, 0),
    (84, 0),
    (86, 0),
    (0, 63),
    (0, 3),
    (0, 36),
    (0, 24),
    (88, 0),
    (90, 0),
    (92, 0),
    (94, 0),
    (96, 0),
    (98, 0),
    (100, 0),
    (102, 0),
    (104, 0),
    (106, 0),
    (108, 0),
    (110, 0),
    (112, 0),
    (114, 0),
    (116, 0),
    (118, 0),
    (0, 34),
    (0, 18),
    (0, 10),
    (0, 6),
    (0, 33),
    (0, 17),
    (0, 9),
    (0, 5),
    (-1, 0),
    (120, 0),
    (122, 0),
    (124, 0),
    (0, 58),
    (0, 54),
    (0, 46),
    (0, 30),
    (0, 57),
    (0, 53),
    (0, 45),
    (0, 29),
    (0, 38),
    (0, 26),
    (0, 37),
    (0, 25),
    (0, 43),
    (0, 23),
    (0, 51),
    (0, 15),
    (0, 42),
    (0, 22),
    (0, 50),
    (0, 14),
    (0, 41),
    (0, 21),
    (0, 49),
    (0, 13),
    (0, 35),
    (0, 19),
    (0, 11),
    (0, 7),
    (0, 39),
    (0, 27),
    (0, 59),
    (0, 55),
    (0, 47),
    (0, 31),
];

const MOTION: &[(i16, i16)] = &[
    (2, 0),
    (0, 0),
    (4, 0),
    (6, 0),
    (8, 0),
    (10, 0),
    (0, 1),
    (0, -1),
    (12, 0),
    (14, 0),
    (0, 2),
    (0, -2),
    (16, 0),
    (18, 0),
    (0, 3),
    (0, -3),
    (20, 0),
    (22, 0),
    (24, 0),
    (26, 0),
    (-1, 0),
    (28, 0),
    (30, 0),
    (32, 0),
    (34, 0),
    (36, 0),
    (0, 4),
    (0, -4),
    (-1, 0),
    (38, 0),
    (40, 0),
    (42, 0),
    (0, 7),
    (0, -7),
    (0, 6),
    (0, -6),
    (0, 5),
    (0, -5),
    (44, 0),
    (46, 0),
    (48, 0),
    (50, 0),
    (52, 0),
    (54, 0),
    (56, 0),
    (58, 0),
    (60, 0),
    (62, 0),
    (64, 0),
    (66, 0),
    (0, 10),
    (0, -10),
    (0, 9),
    (0, -9),
    (0, 8),
    (0, -8),
    (0, 16),
    (0, -16),
    (0, 15),
    (0, -15),
    (0, 14),
    (0, -14),
    (0, 13),
    (0, -13),
    (0, 12),
    (0, -12),
    (0, 11),
    (0, -11),
];

const DCT_SIZE_LUMINANCE: &[(i16, i16)] = &[
    (2, 0),
    (4, 0),
    (0, 1),
    (0, 2),
    (6, 0),
    (8, 0),
    (0, 0),
    (0, 3),
    (0, 4),
    (10, 0),
    (0, 5),
    (12, 0),
    (0, 6),
    (14, 0),
    (0, 7),
    (16, 0),
    (0, 8),
    (-1, 0),
];

const DCT_SIZE_CHROMINANCE: &[(i16, i16)] = &[
    (2, 0),
    (4, 0),
    (0, 0),
    (0, 1),
    (0, 2),
    (6, 0),
    (0, 3),
    (8, 0),
    (0, 4),
    (10, 0),
    (0, 5),
    (12, 0),
    (0, 6),
    (14, 0),
    (0, 7),
    (16, 0),
    (0, 8),
    (-1, 0),
];

fn dct_size_table(plane_index: usize) -> &'static [(i16, i16)] {
    if plane_index == 0 {
        DCT_SIZE_LUMINANCE
    } else {
        DCT_SIZE_CHROMINANCE
    }
}

// dct_coeff bitmap: 0xff00 run, 0x00ff level. Values are unsigned; the sign bit
// follows in the stream. 0xffff = escape.
const DCT_COEFF: &[(i16, u16)] = &[
    (2, 0),
    (0, 0x0001),
    (4, 0),
    (6, 0),
    (8, 0),
    (10, 0),
    (12, 0),
    (0, 0x0101),
    (14, 0),
    (16, 0),
    (18, 0),
    (20, 0),
    (0, 0x0002),
    (0, 0x0201),
    (22, 0),
    (24, 0),
    (26, 0),
    (28, 0),
    (30, 0),
    (0, 0x0003),
    (0, 0x0401),
    (0, 0x0301),
    (32, 0),
    (0, 0xffff),
    (34, 0),
    (36, 0),
    (0, 0x0701),
    (0, 0x0601),
    (0, 0x0102),
    (0, 0x0501),
    (38, 0),
    (40, 0),
    (42, 0),
    (44, 0),
    (0, 0x0202),
    (0, 0x0901),
    (0, 0x0004),
    (0, 0x0801),
    (46, 0),
    (48, 0),
    (50, 0),
    (52, 0),
    (54, 0),
    (56, 0),
    (58, 0),
    (60, 0),
    (0, 0x0d01),
    (0, 0x0006),
    (0, 0x0c01),
    (0, 0x0b01),
    (0, 0x0302),
    (0, 0x0103),
    (0, 0x0005),
    (0, 0x0a01),
    (62, 0),
    (64, 0),
    (66, 0),
    (68, 0),
    (70, 0),
    (72, 0),
    (74, 0),
    (76, 0),
    (78, 0),
    (80, 0),
    (82, 0),
    (84, 0),
    (86, 0),
    (88, 0),
    (90, 0),
    (92, 0),
    (0, 0x1001),
    (0, 0x0502),
    (0, 0x0007),
    (0, 0x0203),
    (0, 0x0104),
    (0, 0x0f01),
    (0, 0x0e01),
    (0, 0x0402),
    (94, 0),
    (96, 0),
    (98, 0),
    (100, 0),
    (102, 0),
    (104, 0),
    (106, 0),
    (108, 0),
    (110, 0),
    (112, 0),
    (114, 0),
    (116, 0),
    (118, 0),
    (120, 0),
    (122, 0),
    (124, 0),
    (-1, 0),
    (126, 0),
    (128, 0),
    (130, 0),
    (132, 0),
    (134, 0),
    (136, 0),
    (138, 0),
    (140, 0),
    (142, 0),
    (144, 0),
    (146, 0),
    (148, 0),
    (150, 0),
    (152, 0),
    (154, 0),
    (0, 0x000b),
    (0, 0x0802),
    (0, 0x0403),
    (0, 0x000a),
    (0, 0x0204),
    (0, 0x0702),
    (0, 0x1501),
    (0, 0x1401),
    (0, 0x0009),
    (0, 0x1301),
    (0, 0x1201),
    (0, 0x0105),
    (0, 0x0303),
    (0, 0x0008),
    (0, 0x0602),
    (0, 0x1101),
    (156, 0),
    (158, 0),
    (160, 0),
    (162, 0),
    (164, 0),
    (166, 0),
    (168, 0),
    (170, 0),
    (172, 0),
    (174, 0),
    (176, 0),
    (178, 0),
    (180, 0),
    (182, 0),
    (0, 0x0a02),
    (0, 0x0902),
    (0, 0x0503),
    (0, 0x0304),
    (0, 0x0205),
    (0, 0x0107),
    (0, 0x0106),
    (0, 0x000f),
    (0, 0x000e),
    (0, 0x000d),
    (0, 0x000c),
    (0, 0x1a01),
    (0, 0x1901),
    (0, 0x1801),
    (0, 0x1701),
    (0, 0x1601),
    (184, 0),
    (186, 0),
    (188, 0),
    (190, 0),
    (192, 0),
    (194, 0),
    (196, 0),
    (198, 0),
    (200, 0),
    (202, 0),
    (204, 0),
    (206, 0),
    (0, 0x001f),
    (0, 0x001e),
    (0, 0x001d),
    (0, 0x001c),
    (0, 0x001b),
    (0, 0x001a),
    (0, 0x0019),
    (0, 0x0018),
    (0, 0x0017),
    (0, 0x0016),
    (0, 0x0015),
    (0, 0x0014),
    (0, 0x0013),
    (0, 0x0012),
    (0, 0x0011),
    (0, 0x0010),
    (208, 0),
    (210, 0),
    (212, 0),
    (214, 0),
    (216, 0),
    (218, 0),
    (220, 0),
    (222, 0),
    (0, 0x0028),
    (0, 0x0027),
    (0, 0x0026),
    (0, 0x0025),
    (0, 0x0024),
    (0, 0x0023),
    (0, 0x0022),
    (0, 0x0021),
    (0, 0x0020),
    (0, 0x010e),
    (0, 0x010d),
    (0, 0x010c),
    (0, 0x010b),
    (0, 0x010a),
    (0, 0x0109),
    (0, 0x0108),
    (0, 0x0112),
    (0, 0x0111),
    (0, 0x0110),
    (0, 0x010f),
    (0, 0x0603),
    (0, 0x1002),
    (0, 0x0f02),
    (0, 0x0e02),
    (0, 0x0d02),
    (0, 0x0c02),
    (0, 0x0b02),
    (0, 0x1f01),
    (0, 0x1e01),
    (0, 0x1d01),
    (0, 0x1c01),
    (0, 0x1b01),
];

/// A decoded image plane (Y, Cr or Cb). Size is rounded up to whole macroblocks.
#[derive(Clone, Default)]
pub struct Plane {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

/// A decoded YCrCb frame. `width`/`height` are the display size; the planes may
/// be larger (rounded up to the nearest macroblock).
#[derive(Clone, Default)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub y: Plane,
    pub cr: Plane,
    pub cb: Plane,
}

#[derive(Clone, Copy, Default)]
struct Motion {
    full_px: bool,
    is_set: bool,
    r_size: i32,
    h: i32,
    v: i32,
}

fn clamp_u8(n: i32) -> u8 {
    n.clamp(0, 255) as u8
}

/// MPEG-1 video decoder over an in-memory elementary stream.
pub struct Video {
    buffer: Buffer,
    framerate: f64,
    pixel_aspect_ratio: f64,
    time: f64,
    last_frame_time: f64,
    frames_decoded: i64,

    width: usize,
    height: usize,
    mb_width: usize,
    mb_height: usize,
    mb_size: usize,
    luma_width: usize,
    luma_height: usize,
    chroma_width: usize,
    chroma_height: usize,

    start_code: i32,
    picture_type: i32,

    motion_forward: Motion,
    motion_backward: Motion,

    has_sequence_header: bool,

    quantizer_scale: i32,
    slice_begin: bool,
    macroblock_address: i32,
    mb_row: usize,
    mb_col: usize,

    macroblock_type: i32,
    macroblock_intra: bool,

    dc_predictor: [i32; 3],

    frames: Vec<Frame>,
    cur: usize,
    fwd: usize,
    bwd: usize,

    block_data: [i32; 64],
    intra_quant_matrix: [u8; 64],
    non_intra_quant_matrix: [u8; 64],

    has_reference_frame: bool,
    assume_no_b_frames: bool,
}

impl Video {
    /// Create a decoder over a video elementary stream and parse the sequence
    /// header if present.
    pub fn new(video_es: Vec<u8>) -> Self {
        let mut s = Self {
            buffer: Buffer::new(video_es),
            framerate: 0.0,
            pixel_aspect_ratio: 0.0,
            time: 0.0,
            last_frame_time: 0.0,
            frames_decoded: 0,
            width: 0,
            height: 0,
            mb_width: 0,
            mb_height: 0,
            mb_size: 0,
            luma_width: 0,
            luma_height: 0,
            chroma_width: 0,
            chroma_height: 0,
            start_code: -1,
            picture_type: 0,
            motion_forward: Motion::default(),
            motion_backward: Motion::default(),
            has_sequence_header: false,
            quantizer_scale: 0,
            slice_begin: false,
            macroblock_address: 0,
            mb_row: 0,
            mb_col: 0,
            macroblock_type: 0,
            macroblock_intra: false,
            dc_predictor: [128; 3],
            frames: Vec::new(),
            cur: 0,
            fwd: 1,
            bwd: 2,
            block_data: [0; 64],
            intra_quant_matrix: [0; 64],
            non_intra_quant_matrix: [0; 64],
            has_reference_frame: false,
            assume_no_b_frames: false,
        };
        s.start_code = s.buffer.find_start_code(START_SEQUENCE);
        if s.start_code != -1 {
            s.decode_sequence_header();
        }
        s
    }

    pub fn has_header(&mut self) -> bool {
        if self.has_sequence_header {
            return true;
        }
        if self.start_code != START_SEQUENCE {
            self.start_code = self.buffer.find_start_code(START_SEQUENCE);
        }
        if self.start_code == -1 {
            return false;
        }
        self.decode_sequence_header()
    }

    pub fn width(&self) -> usize {
        self.width
    }
    pub fn height(&self) -> usize {
        self.height
    }
    pub fn framerate(&self) -> f64 {
        self.framerate
    }
    pub fn pixel_aspect_ratio(&self) -> f64 {
        self.pixel_aspect_ratio
    }
    /// Display time (seconds) of the most recently returned frame.
    pub fn last_frame_time(&self) -> f64 {
        self.last_frame_time
    }
    /// Borrow a decoded frame by index (as returned by [`decode`](Self::decode)).
    pub fn frame(&self, index: usize) -> &Frame {
        &self.frames[index]
    }

    /// Decode the next frame in display order. Returns the index of the frame
    /// to display, or `None` if no more frames are available.
    pub fn decode(&mut self) -> Option<usize> {
        if !self.has_header() {
            return None;
        }
        loop {
            if self.start_code != START_PICTURE {
                self.start_code = self.buffer.find_start_code(START_PICTURE);
                if self.start_code == -1 {
                    // Flush the last reference frame at end of stream.
                    if self.has_reference_frame
                        && !self.assume_no_b_frames
                        && self.buffer.has_ended()
                        && (self.picture_type == PICTURE_TYPE_INTRA
                            || self.picture_type == PICTURE_TYPE_PREDICTIVE)
                    {
                        self.has_reference_frame = false;
                        return Some(self.bwd);
                    }
                    return None;
                }
            }

            // Need a full picture buffered before decoding it.
            if self.buffer.has_start_code(START_PICTURE) == -1 && !self.buffer.has_ended() {
                return None;
            }

            self.decode_picture();

            let frame = if self.assume_no_b_frames {
                Some(self.bwd)
            } else if self.picture_type == PICTURE_TYPE_B {
                Some(self.cur)
            } else if self.has_reference_frame {
                Some(self.fwd)
            } else {
                self.has_reference_frame = true;
                None
            };

            if let Some(idx) = frame {
                self.last_frame_time = self.time;
                self.frames_decoded += 1;
                self.time = self.frames_decoded as f64 / self.framerate;
                return Some(idx);
            }
        }
    }

    fn decode_sequence_header(&mut self) -> bool {
        let max_header_size = 64 + 2 * 64 * 8;
        if !self.buffer.has(max_header_size) {
            return false;
        }

        self.width = self.buffer.read(12) as usize;
        self.height = self.buffer.read(12) as usize;
        if self.width == 0 || self.height == 0 {
            return false;
        }

        let mut par_code = self.buffer.read(4) as i32 - 1;
        if par_code < 0 {
            par_code = 0;
        }
        let par_last = (PIXEL_ASPECT_RATIO.len() - 1) as i32;
        if par_code > par_last {
            par_code = par_last;
        }
        self.pixel_aspect_ratio = PIXEL_ASPECT_RATIO[par_code as usize];

        self.framerate = PICTURE_RATE[self.buffer.read(4) as usize];

        // Skip bit_rate, marker, buffer_size and constrained bit.
        self.buffer.skip(18 + 1 + 10 + 1);

        // Custom intra quant matrix?
        if self.buffer.read(1) != 0 {
            for i in 0..64 {
                let idx = ZIG_ZAG[i];
                self.intra_quant_matrix[idx] = self.buffer.read(8) as u8;
            }
        } else {
            self.intra_quant_matrix = INTRA_QUANT_MATRIX;
        }

        // Custom non-intra quant matrix?
        if self.buffer.read(1) != 0 {
            for i in 0..64 {
                let idx = ZIG_ZAG[i];
                self.non_intra_quant_matrix[idx] = self.buffer.read(8) as u8;
            }
        } else {
            self.non_intra_quant_matrix = NON_INTRA_QUANT_MATRIX;
        }

        self.mb_width = (self.width + 15) >> 4;
        self.mb_height = (self.height + 15) >> 4;
        self.mb_size = self.mb_width * self.mb_height;
        self.luma_width = self.mb_width << 4;
        self.luma_height = self.mb_height << 4;
        self.chroma_width = self.mb_width << 3;
        self.chroma_height = self.mb_height << 3;

        self.frames = vec![self.make_frame(), self.make_frame(), self.make_frame()];
        self.cur = 0;
        self.fwd = 1;
        self.bwd = 2;

        self.has_sequence_header = true;
        true
    }

    fn make_frame(&self) -> Frame {
        let luma = self.luma_width * self.luma_height;
        let chroma = self.chroma_width * self.chroma_height;
        Frame {
            width: self.width,
            height: self.height,
            y: Plane {
                width: self.luma_width,
                height: self.luma_height,
                data: vec![0; luma],
            },
            cr: Plane {
                width: self.chroma_width,
                height: self.chroma_height,
                data: vec![0; chroma],
            },
            cb: Plane {
                width: self.chroma_width,
                height: self.chroma_height,
                data: vec![0; chroma],
            },
        }
    }
}

impl Video {
    fn decode_picture(&mut self) {
        self.buffer.skip(10); // temporal_reference
        self.picture_type = self.buffer.read(3) as i32;
        self.buffer.skip(16); // vbv_delay

        // D frames or unknown coding type.
        if self.picture_type <= 0 || self.picture_type > PICTURE_TYPE_B {
            return;
        }

        if self.picture_type == PICTURE_TYPE_PREDICTIVE || self.picture_type == PICTURE_TYPE_B {
            self.motion_forward.full_px = self.buffer.read(1) != 0;
            let f_code = self.buffer.read(3) as i32;
            if f_code == 0 {
                return;
            }
            self.motion_forward.r_size = f_code - 1;
        }

        if self.picture_type == PICTURE_TYPE_B {
            self.motion_backward.full_px = self.buffer.read(1) != 0;
            let f_code = self.buffer.read(3) as i32;
            if f_code == 0 {
                return;
            }
            self.motion_backward.r_size = f_code - 1;
        }

        let frame_temp = self.fwd;
        if self.picture_type == PICTURE_TYPE_INTRA || self.picture_type == PICTURE_TYPE_PREDICTIVE {
            self.fwd = self.bwd;
        }

        // Find first slice start code; skip extension and user data.
        loop {
            self.start_code = self.buffer.next_start_code();
            if self.start_code != START_EXTENSION && self.start_code != START_USER_DATA {
                break;
            }
        }

        while is_slice_start_code(self.start_code) {
            self.decode_slice(self.start_code & 0xFF);
            if self.macroblock_address >= self.mb_size as i32 - 1 {
                break;
            }
            self.start_code = self.buffer.next_start_code();
        }

        // Rotate prediction frames for reference pictures.
        if self.picture_type == PICTURE_TYPE_INTRA || self.picture_type == PICTURE_TYPE_PREDICTIVE {
            self.bwd = self.cur;
            self.cur = frame_temp;
        }
    }

    fn decode_slice(&mut self, slice: i32) {
        self.slice_begin = true;
        self.macroblock_address = (slice - 1) * self.mb_width as i32 - 1;

        self.motion_forward.h = 0;
        self.motion_forward.v = 0;
        self.motion_backward.h = 0;
        self.motion_backward.v = 0;
        self.dc_predictor = [128; 3];

        self.quantizer_scale = self.buffer.read(5) as i32;

        // Skip extra slice information.
        while self.buffer.read(1) != 0 {
            self.buffer.skip(8);
        }

        loop {
            self.decode_macroblock();
            if self.macroblock_address >= self.mb_size as i32 - 1 || !self.buffer.peek_non_zero(23)
            {
                break;
            }
        }
    }

    fn decode_macroblock(&mut self) {
        // Decode address increment.
        let mut increment = 0i32;
        let mut t = self.buffer.read_vlc(MACROBLOCK_ADDRESS_INCREMENT) as i32;
        while t == 34 {
            // macroblock_stuffing
            t = self.buffer.read_vlc(MACROBLOCK_ADDRESS_INCREMENT) as i32;
        }
        while t == 35 {
            // macroblock_escape
            increment += 33;
            t = self.buffer.read_vlc(MACROBLOCK_ADDRESS_INCREMENT) as i32;
        }
        increment += t;

        if self.slice_begin {
            self.slice_begin = false;
            self.macroblock_address += increment;
        } else {
            if self.macroblock_address + increment >= self.mb_size as i32 {
                return; // invalid
            }
            if increment > 1 {
                self.dc_predictor = [128; 3];
                if self.picture_type == PICTURE_TYPE_PREDICTIVE {
                    self.motion_forward.h = 0;
                    self.motion_forward.v = 0;
                }
            }
            while increment > 1 {
                self.macroblock_address += 1;
                self.mb_row = (self.macroblock_address as usize) / self.mb_width;
                self.mb_col = (self.macroblock_address as usize) % self.mb_width;
                self.predict_macroblock();
                increment -= 1;
            }
            self.macroblock_address += 1;
        }

        if self.macroblock_address < 0 {
            return;
        }
        self.mb_row = (self.macroblock_address as usize) / self.mb_width;
        self.mb_col = (self.macroblock_address as usize) % self.mb_width;
        if self.mb_col >= self.mb_width || self.mb_row >= self.mb_height {
            return; // corrupt stream
        }

        let table = macroblock_type_table(self.picture_type);
        self.macroblock_type = self.buffer.read_vlc(table) as i32;
        self.macroblock_intra = (self.macroblock_type & 0x01) != 0;
        self.motion_forward.is_set = (self.macroblock_type & 0x08) != 0;
        self.motion_backward.is_set = (self.macroblock_type & 0x04) != 0;

        if (self.macroblock_type & 0x10) != 0 {
            self.quantizer_scale = self.buffer.read(5) as i32;
        }

        if self.macroblock_intra {
            self.motion_forward.h = 0;
            self.motion_forward.v = 0;
            self.motion_backward.h = 0;
            self.motion_backward.v = 0;
        } else {
            self.dc_predictor = [128; 3];
            self.decode_motion_vectors();
            self.predict_macroblock();
        }

        let cbp = if (self.macroblock_type & 0x02) != 0 {
            self.buffer.read_vlc(CODE_BLOCK_PATTERN) as i32
        } else if self.macroblock_intra {
            0x3f
        } else {
            0
        };

        let mut mask = 0x20;
        for block in 0..6 {
            if (cbp & mask) != 0 {
                self.decode_block(block);
            }
            mask >>= 1;
        }
    }

    fn decode_motion_vectors(&mut self) {
        if self.motion_forward.is_set {
            let r_size = self.motion_forward.r_size;
            self.motion_forward.h = self.decode_motion_vector(r_size, self.motion_forward.h);
            self.motion_forward.v = self.decode_motion_vector(r_size, self.motion_forward.v);
        } else if self.picture_type == PICTURE_TYPE_PREDICTIVE {
            self.motion_forward.h = 0;
            self.motion_forward.v = 0;
        }

        if self.motion_backward.is_set {
            let r_size = self.motion_backward.r_size;
            self.motion_backward.h = self.decode_motion_vector(r_size, self.motion_backward.h);
            self.motion_backward.v = self.decode_motion_vector(r_size, self.motion_backward.v);
        }
    }

    fn decode_motion_vector(&mut self, r_size: i32, mut motion: i32) -> i32 {
        let fscale = 1 << r_size;
        let m_code = self.buffer.read_vlc(MOTION) as i32;
        let d = if m_code != 0 && fscale != 1 {
            let r = self.buffer.read(r_size as usize) as i32;
            let mut dd = ((m_code.abs() - 1) << r_size) + r + 1;
            if m_code < 0 {
                dd = -dd;
            }
            dd
        } else {
            m_code
        };

        motion += d;
        if motion > (fscale << 4) - 1 {
            motion -= fscale << 5;
        } else if motion < -(fscale << 4) {
            motion += fscale << 5;
        }
        motion
    }

    fn predict_macroblock(&mut self) {
        let mut fw_h = self.motion_forward.h;
        let mut fw_v = self.motion_forward.v;
        if self.motion_forward.full_px {
            fw_h <<= 1;
            fw_v <<= 1;
        }

        if self.picture_type == PICTURE_TYPE_B {
            let mut bw_h = self.motion_backward.h;
            let mut bw_v = self.motion_backward.v;
            if self.motion_backward.full_px {
                bw_h <<= 1;
                bw_v <<= 1;
            }

            if self.motion_forward.is_set {
                self.copy_or_interpolate_macroblock(self.fwd, fw_h, fw_v, false);
                if self.motion_backward.is_set {
                    self.copy_or_interpolate_macroblock(self.bwd, bw_h, bw_v, true);
                }
            } else {
                self.copy_or_interpolate_macroblock(self.bwd, bw_h, bw_v, false);
            }
        } else {
            self.copy_or_interpolate_macroblock(self.fwd, fw_h, fw_v, false);
        }
    }

    fn copy_or_interpolate_macroblock(
        &mut self,
        src_idx: usize,
        mh: i32,
        mv: i32,
        interpolate: bool,
    ) {
        let (mb_row, mb_col, mbw, mbh) = (self.mb_row, self.mb_col, self.mb_width, self.mb_height);
        let cur = self.cur;
        let (dst, src) = frame_pair(&mut self.frames, cur, src_idx);
        process_macroblock(
            &src.y.data,
            &mut dst.y.data,
            mb_row,
            mb_col,
            mbw,
            mbh,
            mh,
            mv,
            16,
            interpolate,
        );
        process_macroblock(
            &src.cr.data,
            &mut dst.cr.data,
            mb_row,
            mb_col,
            mbw,
            mbh,
            mh / 2,
            mv / 2,
            8,
            interpolate,
        );
        process_macroblock(
            &src.cb.data,
            &mut dst.cb.data,
            mb_row,
            mb_col,
            mbw,
            mbh,
            mh / 2,
            mv / 2,
            8,
            interpolate,
        );
    }

    fn decode_block(&mut self, block: i32) {
        let mut n = 0usize;
        let intra = self.macroblock_intra;

        if intra {
            // DC coefficient prediction.
            let plane_index = if block > 3 { (block - 3) as usize } else { 0 };
            let predictor = self.dc_predictor[plane_index];
            let dct_size = self.buffer.read_vlc(dct_size_table(plane_index)) as i32;

            if dct_size > 0 {
                let differential = self.buffer.read(dct_size as usize) as i32;
                if (differential & (1 << (dct_size - 1))) != 0 {
                    self.block_data[0] = predictor + differential;
                } else {
                    self.block_data[0] = predictor + (-(1 << dct_size) | (differential + 1));
                }
            } else {
                self.block_data[0] = predictor;
            }

            self.dc_predictor[plane_index] = self.block_data[0];
            self.block_data[0] <<= 3 + 5;
            n = 1;
        }

        let quant_matrix = if intra {
            self.intra_quant_matrix
        } else {
            self.non_intra_quant_matrix
        };

        // AC coefficients (+DC for non-intra).
        loop {
            let coeff = self.buffer.read_vlc_uint(DCT_COEFF);

            if coeff == 0x0001 && n > 0 && self.buffer.read(1) == 0 {
                break; // end_of_block
            }

            let run: i32;
            let mut level: i32;
            if coeff == 0xffff {
                // escape
                run = self.buffer.read(6) as i32;
                level = self.buffer.read(8) as i32;
                if level == 0 {
                    level = self.buffer.read(8) as i32;
                } else if level == 128 {
                    level = self.buffer.read(8) as i32 - 256;
                } else if level > 128 {
                    level -= 256;
                }
            } else {
                run = (coeff >> 8) as i32;
                level = (coeff & 0xff) as i32;
                if self.buffer.read(1) != 0 {
                    level = -level;
                }
            }

            n += run as usize;
            if n >= 64 {
                return; // invalid
            }

            let de_zig_zagged = ZIG_ZAG[n];
            n += 1;

            // Dequantize, oddify, clip.
            level <<= 1;
            if !intra {
                level += if level < 0 { -1 } else { 1 };
            }
            level = (level * self.quantizer_scale * quant_matrix[de_zig_zagged] as i32) >> 4;
            if (level & 1) == 0 {
                level -= if level > 0 { 1 } else { -1 };
            }
            level = level.clamp(-2048, 2047);

            self.block_data[de_zig_zagged] = level * PREMULTIPLIER_MATRIX[de_zig_zagged];
        }

        // Move the block into its plane.
        let mut s = self.block_data;
        self.block_data = [0; 64];
        let n_is_one = n == 1;
        if !n_is_one {
            idct(&mut s);
        }

        let (dw, di) = if block < 4 {
            let mut di = (self.mb_row * self.luma_width + self.mb_col) << 4;
            if (block & 1) != 0 {
                di += 8;
            }
            if (block & 2) != 0 {
                di += self.luma_width << 3;
            }
            (self.luma_width, di)
        } else {
            let di = ((self.mb_row * self.luma_width) << 2) + (self.mb_col << 3);
            (self.chroma_width, di)
        };

        let cur = self.cur;
        let frame = &mut self.frames[cur];
        let d = if block < 4 {
            &mut frame.y.data
        } else if block == 4 {
            &mut frame.cb.data
        } else {
            &mut frame.cr.data
        };

        if intra {
            if n_is_one {
                let value = clamp_u8((s[0] + 128) >> 8);
                block_set_const(d, di, dw, value);
            } else {
                block_set_overwrite(d, di, dw, &s);
            }
        } else if n_is_one {
            let value = (s[0] + 128) >> 8;
            block_set_add_const(d, di, dw, value);
        } else {
            block_set_add(d, di, dw, &s);
        }
    }
}

/// Split out two distinct frames from the slice: a mutable destination and an
/// immutable source. `dst` must not equal `src`.
fn frame_pair(frames: &mut [Frame], dst: usize, src: usize) -> (&mut Frame, &Frame) {
    debug_assert_ne!(dst, src);
    if dst < src {
        let (a, b) = frames.split_at_mut(src);
        (&mut a[dst], &b[0])
    } else {
        let (a, b) = frames.split_at_mut(dst);
        (&mut b[0], &a[src])
    }
}

// 8x8 block writers (block source width is 8).
fn block_set_const(d: &mut [u8], mut di: usize, dw: usize, val: u8) {
    let dest_scan = dw - 8;
    for _y in 0..8 {
        for _x in 0..8 {
            d[di] = val;
            di += 1;
        }
        di += dest_scan;
    }
}

fn block_set_overwrite(d: &mut [u8], mut di: usize, dw: usize, s: &[i32; 64]) {
    let dest_scan = dw - 8;
    let mut si = 0;
    for _y in 0..8 {
        for _x in 0..8 {
            d[di] = clamp_u8(s[si]);
            si += 1;
            di += 1;
        }
        di += dest_scan;
    }
}

fn block_set_add_const(d: &mut [u8], mut di: usize, dw: usize, value: i32) {
    let dest_scan = dw - 8;
    for _y in 0..8 {
        for _x in 0..8 {
            d[di] = clamp_u8(d[di] as i32 + value);
            di += 1;
        }
        di += dest_scan;
    }
}

fn block_set_add(d: &mut [u8], mut di: usize, dw: usize, s: &[i32; 64]) {
    let dest_scan = dw - 8;
    let mut si = 0;
    for _y in 0..8 {
        for _x in 0..8 {
            d[di] = clamp_u8(d[di] as i32 + s[si]);
            si += 1;
            di += 1;
        }
        di += dest_scan;
    }
}

#[allow(clippy::too_many_arguments)]
fn process_macroblock(
    s: &[u8],
    d: &mut [u8],
    mb_row: usize,
    mb_col: usize,
    mb_width: usize,
    mb_height: usize,
    motion_h: i32,
    motion_v: i32,
    block_size: usize,
    interpolate: bool,
) {
    let dw = mb_width * block_size;
    let hp = motion_h >> 1;
    let vp = motion_v >> 1;
    let odd_h = (motion_h & 1) != 0;
    let odd_v = (motion_v & 1) != 0;

    let si0 = ((mb_row * block_size) as i64 + vp as i64) * dw as i64
        + (mb_col * block_size) as i64
        + hp as i64;
    let di0 = ((mb_row * dw + mb_col) * block_size) as i64;
    let max_address = (dw * (mb_height * block_size - block_size + 1) - block_size) as i64;
    if si0 < 0 || si0 > max_address || di0 < 0 || di0 > max_address {
        return; // corrupt video
    }

    let mut si = si0 as usize;
    let mut di = di0 as usize;
    let scan = dw - block_size;
    for _y in 0..block_size {
        for _x in 0..block_size {
            let val: i32 = match (interpolate, odd_h, odd_v) {
                (false, false, false) => s[si] as i32,
                (false, false, true) => (s[si] as i32 + s[si + dw] as i32 + 1) >> 1,
                (false, true, false) => (s[si] as i32 + s[si + 1] as i32 + 1) >> 1,
                (false, true, true) => {
                    (s[si] as i32
                        + s[si + 1] as i32
                        + s[si + dw] as i32
                        + s[si + dw + 1] as i32
                        + 2)
                        >> 2
                }
                (true, false, false) => (d[di] as i32 + s[si] as i32 + 1) >> 1,
                (true, false, true) => {
                    (d[di] as i32 + ((s[si] as i32 + s[si + dw] as i32 + 1) >> 1) + 1) >> 1
                }
                (true, true, false) => {
                    (d[di] as i32 + ((s[si] as i32 + s[si + 1] as i32 + 1) >> 1) + 1) >> 1
                }
                (true, true, true) => {
                    (d[di] as i32
                        + ((s[si] as i32
                            + s[si + 1] as i32
                            + s[si + dw] as i32
                            + s[si + dw + 1] as i32
                            + 2)
                            >> 2)
                        + 1)
                        >> 1
                }
            };
            d[di] = val as u8;
            si += 1;
            di += 1;
        }
        si += scan;
        di += scan;
    }
}

fn idct(block: &mut [i32; 64]) {
    // Transform columns.
    for i in 0..8 {
        let b1 = block[4 * 8 + i];
        let b3 = block[2 * 8 + i] + block[6 * 8 + i];
        let b4 = block[5 * 8 + i] - block[3 * 8 + i];
        let tmp1 = block[8 + i] + block[7 * 8 + i];
        let tmp2 = block[3 * 8 + i] + block[5 * 8 + i];
        let b6 = block[8 + i] - block[7 * 8 + i];
        let b7 = tmp1 + tmp2;
        let m0 = block[i];
        let x4 = ((b6 * 473 - b4 * 196 + 128) >> 8) - b7;
        let x0 = x4 - (((tmp1 - tmp2) * 362 + 128) >> 8);
        let x1 = m0 - b1;
        let x2 = (((block[2 * 8 + i] - block[6 * 8 + i]) * 362 + 128) >> 8) - b3;
        let x3 = m0 + b1;
        let y3 = x1 + x2;
        let y4 = x3 + b3;
        let y5 = x1 - x2;
        let y6 = x3 - b3;
        let y7 = -x0 - ((b4 * 473 + b6 * 196 + 128) >> 8);
        block[i] = b7 + y4;
        block[8 + i] = x4 + y3;
        block[2 * 8 + i] = y5 - x0;
        block[3 * 8 + i] = y6 - y7;
        block[4 * 8 + i] = y6 + y7;
        block[5 * 8 + i] = x0 + y5;
        block[6 * 8 + i] = y3 - x4;
        block[7 * 8 + i] = y4 - b7;
    }

    // Transform rows.
    let mut i = 0;
    while i < 64 {
        let b1 = block[4 + i];
        let b3 = block[2 + i] + block[6 + i];
        let b4 = block[5 + i] - block[3 + i];
        let tmp1 = block[1 + i] + block[7 + i];
        let tmp2 = block[3 + i] + block[5 + i];
        let b6 = block[1 + i] - block[7 + i];
        let b7 = tmp1 + tmp2;
        let m0 = block[i];
        let x4 = ((b6 * 473 - b4 * 196 + 128) >> 8) - b7;
        let x0 = x4 - (((tmp1 - tmp2) * 362 + 128) >> 8);
        let x1 = m0 - b1;
        let x2 = (((block[2 + i] - block[6 + i]) * 362 + 128) >> 8) - b3;
        let x3 = m0 + b1;
        let y3 = x1 + x2;
        let y4 = x3 + b3;
        let y5 = x1 - x2;
        let y6 = x3 - b3;
        let y7 = -x0 - ((b4 * 473 + b6 * 196 + 128) >> 8);
        block[i] = (b7 + y4 + 128) >> 8;
        block[1 + i] = (x4 + y3 + 128) >> 8;
        block[2 + i] = (y5 - x0 + 128) >> 8;
        block[3 + i] = (y6 - y7 + 128) >> 8;
        block[4 + i] = (y6 + y7 + 128) >> 8;
        block[5 + i] = (x0 + y5 + 128) >> 8;
        block[6 + i] = (y3 - x4 + 128) >> 8;
        block[7 + i] = (y4 - b7 + 128) >> 8;
        i += 8;
    }
}

impl Frame {
    /// Convert this YCrCb frame to XRGB8888 and write it into `dst`, scaling
    /// (nearest-neighbour) to `dst_w` x `dst_h`. `dst` must hold dst_w*dst_h u32.
    /// Uses the BT.601 conversion (same coefficients as PL_MPEG).
    pub fn write_rgb_scaled(&self, dst: &mut [u32], dst_w: usize, dst_h: usize) {
        if dst_w == 0 || dst_h == 0 {
            return;
        }
        let sw = self.width.max(1);
        let sh = self.height.max(1);
        let yw = self.y.width;
        let cw = self.cr.width;

        for ty in 0..dst_h {
            let sy = (ty * sh / dst_h).min(sh - 1);
            let y_row = sy * yw;
            let c_row = (sy / 2) * cw;
            let d_row = ty * dst_w;
            for tx in 0..dst_w {
                let sx = (tx * sw / dst_w).min(sw - 1);
                let yv = self.y.data[y_row + sx] as i32;
                let ci = c_row + (sx / 2);
                let cr = self.cr.data[ci] as i32 - 128;
                let cb = self.cb.data[ci] as i32 - 128;

                let yy = ((yv - 16) * 76309) >> 16;
                let r = (cr * 104597) >> 16;
                let g = (cb * 25674 + cr * 53278) >> 16;
                let b = (cb * 132201) >> 16;

                let rr = clamp_u8(yy + r) as u32;
                let gg = clamp_u8(yy - g) as u32;
                let bb = clamp_u8(yy + b) as u32;
                dst[d_row + tx] = 0xFF00_0000 | (rr << 16) | (gg << 8) | bb;
            }
        }
    }
}
