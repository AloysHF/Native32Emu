// libretro API implementation.

#![allow(static_mut_refs)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use super::callbacks;
use super::constants::*;
use super::types::*;
use native32emu_core::emulator::Emulator;
use native32emu_core::input_handler::InputHandler;
use std::ffi::{c_void, CStr};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};

/// Global emulator instance
static mut EMULATOR: Option<Emulator> = None;

/// Previous RetroPad Select state, used to trigger the back action once per press.
static RETURN_BUTTON_DOWN: AtomicBool = AtomicBool::new(false);

/// Get a reference to the emulator
unsafe fn get_emulator() -> &'static Emulator {
    EMULATOR.as_ref().expect("Emulator not initialized")
}

/// Get a mutable reference to the emulator
unsafe fn get_emulator_mut() -> &'static mut Emulator {
    EMULATOR.as_mut().expect("Emulator not initialized")
}

// ============================================================
// Startup functions
// ============================================================

/// Set environment callback
#[no_mangle]
pub extern "C" fn retro_set_environment(cb: retro_environment_t) {
    callbacks::set_environment(cb);
    // Declare core options as early as possible so the frontend can show them
    // before any content is loaded.
    set_core_options();
}

/// Set video refresh callback
#[no_mangle]
pub extern "C" fn retro_set_video_refresh(cb: retro_video_refresh_t) {
    callbacks::set_video_refresh(cb);
}

/// Set audio sample callback
#[no_mangle]
pub extern "C" fn retro_set_audio_sample(cb: retro_audio_sample_t) {
    callbacks::set_audio_sample(cb);
}

/// Set batch audio sample callback
#[no_mangle]
pub extern "C" fn retro_set_audio_sample_batch(cb: retro_audio_sample_batch_t) {
    callbacks::set_audio_sample_batch(cb);
}

/// Set input poll callback
#[no_mangle]
pub extern "C" fn retro_set_input_poll(cb: retro_input_poll_t) {
    callbacks::set_input_poll(cb);
}

/// Set input state callback
#[no_mangle]
pub extern "C" fn retro_set_input_state(cb: retro_input_state_t) {
    callbacks::set_input_state(cb);
}

/// Return API version
#[no_mangle]
pub extern "C" fn retro_api_version() -> u32 {
    RETRO_API_VERSION
}

/// Initialize the core
#[no_mangle]
pub extern "C" fn retro_init() {
    // Initialize logging
    callbacks::init_log();
    super::logger::init();
    log::info!("Native32Emu libretro core initialized");
}

/// Deinitialize the core
#[no_mangle]
pub extern "C" fn retro_deinit() {
    unsafe {
        EMULATOR = None;
    }
    RETURN_BUTTON_DOWN.store(false, Ordering::Relaxed);
    log::info!("Native32Emu libretro core deinitialized");
}

/// Get system information
#[no_mangle]
pub extern "C" fn retro_get_system_info(info: *mut retro_system_info) {
    unsafe {
        (*info) = retro_system_info {
            library_name: c"Native32Emu".as_ptr(),
            library_version: c"1.2.0".as_ptr(),
            valid_extensions: c"smf|sgm|ssl|zip".as_ptr(),
            need_fullpath: true,
            // A .zip is a complete Native32 game package; the core extracts it
            // itself and boots its menu entry point, so RetroArch must hand over
            // the whole archive rather than auto-extracting an inner file.
            block_extract: true,
        };
    }
}

/// Set controller port device
#[no_mangle]
pub extern "C" fn retro_set_controller_port_device(_port: u32, _device: u32) {
    // Native32 only supports basic joypad, ignore device type changes
}

// ============================================================
// Running functions
// ============================================================

/// Load a game
#[no_mangle]
pub extern "C" fn retro_load_game(info: *const retro_game_info) -> bool {
    unsafe {
        RETURN_BUTTON_DOWN.store(false, Ordering::Relaxed);
        let game_info = &*info;

        // Check if path is valid
        if game_info.path.is_null() {
            log::error!("Game path is null");
            return false;
        }

        let path = match CStr::from_ptr(game_info.path).to_str() {
            Ok(p) => p,
            Err(e) => {
                log::error!("Invalid game path: {}", e);
                return false;
            }
        };

        // Set pixel format to XRGB8888
        let pixel_format = retro_pixel_format::RETRO_PIXEL_FORMAT_XRGB8888;
        let success = callbacks::environment(
            RETRO_ENVIRONMENT_SET_PIXEL_FORMAT,
            &pixel_format as *const _ as *mut c_void,
        );
        if !success {
            log::error!("Failed to set pixel format");
            return false;
        }

        // Register input descriptors
        register_input_descriptors();

        // Set performance level hint
        let perf_level: u32 = 4;
        callbacks::environment(
            RETRO_ENVIRONMENT_SET_PERFORMANCE_LEVEL,
            &perf_level as *const _ as *mut c_void,
        );

        // Create emulator instance
        match Emulator::from_path(std::path::PathBuf::from(path), 100) {
            Ok(mut emu) => {
                let (width, height) = emu.get_resolution();
                log::info!("Game loaded: {} ({}x{})", path, width, height);
                // Apply the user's current core option selections.
                apply_core_options(&mut emu);
                EMULATOR = Some(emu);
                true
            }
            Err(e) => {
                log::error!("Failed to load game: {}", e);
                false
            }
        }
    }
}

/// Unload the current game
#[no_mangle]
pub extern "C" fn retro_unload_game() {
    unsafe {
        EMULATOR = None;
    }
    RETURN_BUTTON_DOWN.store(false, Ordering::Relaxed);
    log::info!("Game unloaded");
}

/// Get system audio-video information
#[no_mangle]
pub extern "C" fn retro_get_system_av_info(info: *mut retro_system_av_info) {
    unsafe {
        let emu = get_emulator();
        let (width, height) = emu.get_resolution();
        let sample_rate = emu.get_audio_sample_rate();

        (*info) = retro_system_av_info {
            geometry: retro_game_geometry {
                base_width: width,
                base_height: height,
                max_width: width,
                max_height: height,
                aspect_ratio: width as f32 / height as f32,
            },
            timing: retro_system_timing {
                fps: 30.0,
                sample_rate,
            },
        };
    }
}

/// Run one frame of the emulator
#[no_mangle]
pub extern "C" fn retro_run() {
    unsafe {
        let emu = get_emulator_mut();

        // 0. Re-apply core options if the user changed them in the frontend menu.
        if core_options_changed() {
            apply_core_options(emu);
        }

        // 1. Poll input
        callbacks::input_poll();

        // 2. Handle the frontend-level back action on the Select press edge.
        if return_button_just_pressed(0) {
            match emu.try_return_to_menu() {
                Ok(true) => log::info!("Returned to the ZIP game menu"),
                Ok(false) => {
                    log::info!("Requesting frontend shutdown");
                    callbacks::environment(RETRO_ENVIRONMENT_SHUTDOWN, ptr::null_mut());
                    return;
                }
                Err(e) => {
                    log::error!("Failed to reload the ZIP game menu: {}", e);
                    callbacks::environment(RETRO_ENVIRONMENT_SHUTDOWN, ptr::null_mut());
                    return;
                }
            }
        }

        // 3. Query joypad button state and convert to Native32 keycodes
        let buttons = query_joypad_buttons(0);
        emu.set_buttons(&buttons);

        // 4. During a cutscene, suppress game input and
        // allow the A or B button to skip the logo/cutscene videos instead.
        // When auto-skip is enabled, skip as soon as the cutscene starts.
        if emu.is_cutscene_active()
            && (emu.auto_skip_cutscenes
                || buttons.contains(&NATIVE32_KEY_A)
                || buttons.contains(&NATIVE32_KEY_B))
        {
            emu.skip_cutscene();
        }

        // 5. Execute one tick of emulation
        emu.tick();

        // 6. Render the frame
        emu.draw();

        // 7. Output video frame
        let framebuffer = emu.get_framebuffer();
        let (width, height) = emu.get_resolution();
        let pitch = width as usize * 4; // XRGB8888 = 4 bytes per pixel
        callbacks::video_refresh(framebuffer.as_ptr() as *const c_void, width, height, pitch);

        // 8. Output audio samples
        let audio_samples = emu.get_pending_audio_samples();
        if !audio_samples.is_empty() {
            callbacks::audio_sample_batch(
                audio_samples.as_ptr(),
                audio_samples.len() / 2, // Stereo: 2 samples per frame
            );
        }
    }
}

/// Reset the game
#[no_mangle]
pub extern "C" fn retro_reset() {
    unsafe {
        if let Some(emu) = EMULATOR.as_mut() {
            emu.reset();
            log::info!("Game reset");
        }
    }
}

/// Get the region (always NTSC)
#[no_mangle]
pub extern "C" fn retro_get_region() -> u32 {
    RETRO_REGION_NTSC
}

// ============================================================
// Serialization functions (save states)
// ============================================================

/// Get the size needed for serialization
#[no_mangle]
pub extern "C" fn retro_serialize_size() -> usize {
    unsafe {
        match EMULATOR.as_ref() {
            Some(emu) => emu.serialize_size(),
            None => 0,
        }
    }
}

/// Serialize the emulator state
#[no_mangle]
pub extern "C" fn retro_serialize(data: *mut c_void, size: usize) -> bool {
    unsafe {
        match EMULATOR.as_mut() {
            Some(emu) => {
                if size < emu.serialize_size() {
                    return false;
                }
                let buffer = std::slice::from_raw_parts_mut(data as *mut u8, size);
                emu.serialize(buffer).is_ok()
            }
            None => false,
        }
    }
}

/// Deserialize the emulator state
#[no_mangle]
pub extern "C" fn retro_unserialize(data: *const c_void, size: usize) -> bool {
    unsafe {
        match EMULATOR.as_mut() {
            Some(emu) => {
                let buffer = std::slice::from_raw_parts(data as *const u8, size);
                emu.deserialize(buffer).is_ok()
            }
            None => false,
        }
    }
}

// ============================================================
// Memory access functions (cheat codes)
// ============================================================

/// Get memory data pointer
#[no_mangle]
pub extern "C" fn retro_get_memory_data(_id: u32) -> *mut c_void {
    // Not implemented yet
    ptr::null_mut()
}

/// Get memory size
#[no_mangle]
pub extern "C" fn retro_get_memory_size(_id: u32) -> usize {
    // Not implemented yet
    0
}

// ============================================================
// Cheat functions
// ============================================================

/// Reset cheat codes
#[no_mangle]
pub extern "C" fn retro_cheat_reset() {
    unsafe {
        if let Some(emu) = EMULATOR.as_mut() {
            emu.cheats.clear();
        }
    }
}

/// Set a cheat code
#[no_mangle]
pub extern "C" fn retro_cheat_set(index: u32, enabled: bool, code: *const std::ffi::c_char) {
    unsafe {
        let Some(emu) = EMULATOR.as_mut() else {
            return;
        };

        let code = if code.is_null() {
            ""
        } else {
            match CStr::from_ptr(code).to_str() {
                Ok(code) => code,
                Err(e) => {
                    log::warn!("Ignoring invalid UTF-8 cheat at slot {}: {}", index, e);
                    return;
                }
            }
        };

        if let Err(e) = emu.cheats.set_slot(index, enabled, code) {
            log::warn!(
                "Ignoring invalid cheat at slot {} ('{}'): {}",
                index,
                code,
                e
            );
        }
    }
}

/// Load a special game (not used)
#[no_mangle]
pub extern "C" fn retro_load_game_special(
    _game_type: u32,
    _info: *const retro_game_info,
    _num_info: usize,
) -> bool {
    false
}

// ============================================================
// Helper functions
// ============================================================

/// Register the core's configurable options with the frontend.
///
/// Uses the legacy `RETRO_ENVIRONMENT_SET_VARIABLES` interface, which every
/// libretro frontend supports (modern RetroArch transparently upgrades it to
/// the categorized core-options UI). Each value string is "Description; " plus
/// a pipe-separated list of choices whose first entry is the default.
fn set_core_options() {
    let variables = [
        retro_variable {
            key: c"native32emu_volume".as_ptr(),
            value: c"Audio Volume (%); 100|90|80|70|60|50|40|30|20|10|0".as_ptr(),
        },
        retro_variable {
            key: c"native32emu_repeat_delay".as_ptr(),
            value: c"Key auto-repeat delay (frames); 12|2|4|6|8|10|14|16|18|20|24|30".as_ptr(),
        },
        retro_variable {
            key: c"native32emu_repeat_period".as_ptr(),
            value: c"Key auto-repeat period (frames); 3|1|2|4|5|6|8|10".as_ptr(),
        },
        retro_variable {
            key: c"native32emu_swap_ab".as_ptr(),
            value: c"Swap A/B buttons; disabled|enabled".as_ptr(),
        },
        retro_variable {
            key: c"native32emu_auto_skip_cutscenes".as_ptr(),
            value: c"Auto-skip cutscene videos; disabled|enabled".as_ptr(),
        },
        // Terminator
        retro_variable {
            key: ptr::null(),
            value: ptr::null(),
        },
    ];

    // The frontend copies the data during the call, so a stack array is fine.
    callbacks::environment(
        RETRO_ENVIRONMENT_SET_VARIABLES,
        variables.as_ptr() as *mut c_void,
    );
}

/// Read a single core option value from the frontend by key.
///
/// Returns `None` if the option is unset or the frontend does not support
/// variables.
fn get_core_option(key: &CStr) -> Option<String> {
    let mut var = retro_variable {
        key: key.as_ptr(),
        value: ptr::null(),
    };
    let ok = callbacks::environment(
        RETRO_ENVIRONMENT_GET_VARIABLE,
        &mut var as *mut _ as *mut c_void,
    );
    if ok && !var.value.is_null() {
        unsafe { CStr::from_ptr(var.value).to_str().ok().map(str::to_owned) }
    } else {
        None
    }
}

/// Ask the frontend whether any core option changed since the last query.
fn core_options_changed() -> bool {
    let mut updated = false;
    let ok = callbacks::environment(
        RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE,
        &mut updated as *mut _ as *mut c_void,
    );
    ok && updated
}

/// Apply the current core option selections to the running emulator.
fn apply_core_options(emu: &mut Emulator) {
    if let Some(volume) = get_core_option(c"native32emu_volume").and_then(|v| v.parse::<u32>().ok())
    {
        emu.audio.set_volume(volume);
    }

    let delay = get_core_option(c"native32emu_repeat_delay").and_then(|v| v.parse::<u32>().ok());
    let period = get_core_option(c"native32emu_repeat_period").and_then(|v| v.parse::<u32>().ok());
    if delay.is_some() || period.is_some() {
        emu.input.set_repeat_timing(
            delay.unwrap_or(InputHandler::DEFAULT_REPEAT_DELAY),
            period.unwrap_or(InputHandler::DEFAULT_REPEAT_PERIOD),
        );
    }

    if let Some(swap) = get_core_option(c"native32emu_swap_ab") {
        emu.input.set_swap_ab(swap == "enabled");
    }

    if let Some(skip) = get_core_option(c"native32emu_auto_skip_cutscenes") {
        emu.set_auto_skip_cutscenes(skip == "enabled");
    }
}

/// Register input descriptors with the frontend
fn register_input_descriptors() {
    let descriptors = [
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_LEFT,
            description: c"Left".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_RIGHT,
            description: c"Right".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_UP,
            description: c"Up".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_DOWN,
            description: c"Down".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_A,
            description: c"A Button".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_B,
            description: c"B Button".as_ptr(),
        },
        retro_input_descriptor {
            port: 0,
            device: RETRO_DEVICE_JOYPAD,
            index: 0,
            id: RETRO_DEVICE_ID_JOYPAD_SELECT,
            description: c"Return / Exit".as_ptr(),
        },
        // Terminator
        retro_input_descriptor {
            port: 0,
            device: 0,
            index: 0,
            id: 0,
            description: ptr::null(),
        },
    ];

    callbacks::environment(
        RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS,
        descriptors.as_ptr() as *mut c_void,
    );
}

/// Return whether the RetroPad Select button was newly pressed this frame.
fn return_button_just_pressed(port: u32) -> bool {
    let is_down =
        callbacks::input_state(port, RETRO_DEVICE_JOYPAD, 0, RETRO_DEVICE_ID_JOYPAD_SELECT) != 0;
    let was_down = RETURN_BUTTON_DOWN.swap(is_down, Ordering::Relaxed);
    is_rising_edge(is_down, was_down)
}

fn is_rising_edge(is_down: bool, was_down: bool) -> bool {
    is_down && !was_down
}

/// Query joypad button state and convert to Native32 keycodes.
///
/// Returns the list of discrete Native32 keycodes that are currently pressed.
/// The keycodes are intentionally NOT OR-ed into a single bitmask, because the
/// Native32 directional keycodes overlap (e.g. DOWN 0x1e00 contains all bits of
/// UP 0x1c00 plus LEFT 0x0200), which would make a packed mask ambiguous.
fn query_joypad_buttons(port: u32) -> Vec<u16> {
    let mut buttons = Vec::new();

    let state = |id: u32| -> bool { callbacks::input_state(port, RETRO_DEVICE_JOYPAD, 0, id) != 0 };

    if state(RETRO_DEVICE_ID_JOYPAD_LEFT) {
        buttons.push(NATIVE32_KEY_LEFT);
    }
    if state(RETRO_DEVICE_ID_JOYPAD_RIGHT) {
        buttons.push(NATIVE32_KEY_RIGHT);
    }
    if state(RETRO_DEVICE_ID_JOYPAD_UP) {
        buttons.push(NATIVE32_KEY_UP);
    }
    if state(RETRO_DEVICE_ID_JOYPAD_DOWN) {
        buttons.push(NATIVE32_KEY_DOWN);
    }
    if state(RETRO_DEVICE_ID_JOYPAD_A) {
        buttons.push(NATIVE32_KEY_B); // SNES A -> Native32 B
    }
    if state(RETRO_DEVICE_ID_JOYPAD_B) {
        buttons.push(NATIVE32_KEY_A); // SNES B -> Native32 A
    }

    buttons
}

#[cfg(test)]
mod tests {
    use super::is_rising_edge;

    #[test]
    fn rising_edge_only_fires_on_initial_press() {
        assert!(!is_rising_edge(false, false));
        assert!(is_rising_edge(true, false));
        assert!(!is_rising_edge(true, true));
        assert!(!is_rising_edge(false, true));
    }
}
