// native32emu-core - the platform-independent Native32 emulator engine.
//
// This crate contains the shared emulator (`emulator::Emulator`) plus all the
// format parsing, rendering, audio and input logic. It has no dependency on any
// windowing or audio output device; the front-ends (the standalone binary and
// the libretro core) link against this crate and add the platform layer.

#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::manual_memcpy)]
#![allow(clippy::needless_range_loop)]

pub mod action_vm;
pub mod actions;
pub mod archive_loader;
pub mod audio_engine;
pub mod cheats;
pub mod content_loader;
pub mod dat_loader;
pub mod des_constants;
pub mod emulator;
pub mod error;
pub mod file_browser;
pub mod file_loader;
pub mod frame_player;
pub mod header_decryptor;
pub mod headless;
pub mod image_decoder;
pub mod input_handler;
pub mod movie;
pub mod mpeg;
pub mod renderer;
pub mod save_manager;
mod save_state;
pub mod sprite_system;
