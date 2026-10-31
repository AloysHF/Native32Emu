// libretro type definitions.

#![allow(non_camel_case_types)]

use std::ffi::c_char;

/// API version constant
pub const RETRO_API_VERSION: u32 = 1;

/// System information (statically allocated)
#[repr(C)]
pub struct retro_system_info {
    pub library_name: *const c_char,
    pub library_version: *const c_char,
    pub valid_extensions: *const c_char,
    pub need_fullpath: bool,
    pub block_extract: bool,
}

/// Game information
#[repr(C)]
pub struct retro_game_info {
    pub path: *const c_char,
    pub data: *const std::ffi::c_void,
    pub size: usize,
    pub meta: *const c_char,
}

/// Audio-video information
#[repr(C)]
pub struct retro_system_av_info {
    pub geometry: retro_game_geometry,
    pub timing: retro_system_timing,
}

/// Game geometry
#[repr(C)]
pub struct retro_game_geometry {
    pub base_width: u32,
    pub base_height: u32,
    pub max_width: u32,
    pub max_height: u32,
    pub aspect_ratio: f32,
}

/// System timing
#[repr(C)]
pub struct retro_system_timing {
    pub fps: f64,
    pub sample_rate: f64,
}

/// Pixel format enum
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum retro_pixel_format {
    /// 0RGB1555, native endian (deprecated)
    RETRO_PIXEL_FORMAT_0RGB1555 = 0,
    /// XRGB8888, native endian (recommended)
    RETRO_PIXEL_FORMAT_XRGB8888 = 1,
    /// RGB565, native endian
    RETRO_PIXEL_FORMAT_RGB565 = 2,
}

/// Core option variable, used by RETRO_ENVIRONMENT_SET_VARIABLES and
/// RETRO_ENVIRONMENT_GET_VARIABLE.
///
/// For SET_VARIABLES, `value` holds the human-readable description followed by
/// "; " and a pipe-separated list of allowed values, the first of which is the
/// default (e.g. "Audio Volume; 100|90|80"). For GET_VARIABLE, the core sets
/// `key` and the frontend fills in `value` with the user's current selection.
#[repr(C)]
pub struct retro_variable {
    pub key: *const c_char,
    pub value: *const c_char,
}

/// Input descriptor
#[repr(C)]
pub struct retro_input_descriptor {
    pub port: u32,
    pub device: u32,
    pub index: u32,
    pub id: u32,
    pub description: *const c_char,
}

/// Log callback type (simplified - no variadic args in Rust)
pub type retro_log_printf_t = unsafe extern "C" fn(level: retro_log_level, fmt: *const c_char);

/// Log levels
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum retro_log_level {
    RETRO_LOG_DEBUG = 0,
    RETRO_LOG_INFO = 1,
    RETRO_LOG_WARN = 2,
    RETRO_LOG_ERROR = 3,
}

/// Log callback structure
#[repr(C)]
pub struct retro_log_callback {
    pub log: retro_log_printf_t,
}

/// On-screen message shown by the frontend (RETRO_ENVIRONMENT_SET_MESSAGE).
#[repr(C)]
pub struct retro_message {
    pub msg: *const c_char,
    /// Duration in frames.
    pub frames: u32,
}

// Callback function types

/// Environment callback
pub type retro_environment_t = unsafe extern "C" fn(cmd: u32, data: *mut std::ffi::c_void) -> bool;

/// Video refresh callback
pub type retro_video_refresh_t =
    unsafe extern "C" fn(data: *const std::ffi::c_void, width: u32, height: u32, pitch: usize);

/// Single audio sample callback
pub type retro_audio_sample_t = unsafe extern "C" fn(left: i16, right: i16);

/// Batch audio sample callback
pub type retro_audio_sample_batch_t =
    unsafe extern "C" fn(data: *const i16, frames: usize) -> usize;

/// Input poll callback
pub type retro_input_poll_t = unsafe extern "C" fn();

/// Input state callback
pub type retro_input_state_t =
    unsafe extern "C" fn(port: u32, device: u32, index: u32, id: u32) -> i16;
