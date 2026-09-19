// Native32 Emulator - standalone front-end (minifb window + CLI).
// This binary reuses the shared emulator core from the `native32emu` library
// crate and only adds the platform layer: window management, command-line
// argument parsing, keyboard/gamepad input and the optional on-screen overlay.
// It is only compiled when the "standalone" feature is enabled.

mod standalone;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use native32emu_core::emulator::Emulator;
use native32emu_core::headless::{run_replay, ReplayConfig};
use native32emu_core::movie::{InputLog, InputMovie};

use crate::standalone::cli::Cli;
use crate::standalone::gamepad::GamepadMapper;
use crate::standalone::gamepad_overlay::GamepadOverlay;
use crate::standalone::scaler::{ScaleFilter, Scaler};

// Platform-specific screen resolution APIs for fullscreen mode

#[cfg(target_os = "windows")]
mod screen {
    extern "system" {
        fn GetSystemMetrics(nIndex: i32) -> i32;
    }
    const SM_CXSCREEN: i32 = 0;
    const SM_CYSCREEN: i32 = 1;

    pub fn get_screen_size() -> (usize, usize) {
        unsafe {
            (
                GetSystemMetrics(SM_CXSCREEN) as usize,
                GetSystemMetrics(SM_CYSCREEN) as usize,
            )
        }
    }
}

#[cfg(target_os = "linux")]
mod screen {
    // X11 FFI for querying display resolution
    type Display = *mut core::ffi::c_void;
    type Window = u64;

    #[link(name = "X11")]
    extern "system" {
        fn XOpenDisplay(display_name: *const u8) -> Display;
        fn XCloseDisplay(display: Display) -> i32;
        fn XDefaultRootWindow(display: Display) -> Window;
        fn XDisplayWidth(display: Display, screen_number: i32) -> i32;
        fn XDisplayHeight(display: Display, screen_number: i32) -> i32;
    }

    pub fn get_screen_size() -> (usize, usize) {
        unsafe {
            let display = XOpenDisplay(std::ptr::null());
            if display.is_null() {
                return (800, 600);
            }
            let w = XDisplayWidth(display, 0) as usize;
            let h = XDisplayHeight(display, 0) as usize;
            let _ = XDefaultRootWindow(display);
            let _ = XCloseDisplay(display);
            (w, h)
        }
    }
}

#[cfg(target_os = "macos")]
mod screen {
    // macOS Core Graphics FFI for querying main display resolution
    type CGDirectDisplayID = u32;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> CGDirectDisplayID;
        fn CGDisplayPixelsWide(display: CGDirectDisplayID) -> usize;
        fn CGDisplayPixelsHigh(display: CGDirectDisplayID) -> usize;
    }

    pub fn get_screen_size() -> (usize, usize) {
        unsafe {
            let display = CGMainDisplayID();
            (CGDisplayPixelsWide(display), CGDisplayPixelsHigh(display))
        }
    }
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let cli = Cli::parse_args();

    // Headless movie replay (no window). Game path may come from the movie header.
    if let Some(movie_path) = cli.replay.clone() {
        let movie = InputMovie::load(&movie_path)?;
        let game_path = match &cli.game_path {
            Some(p) => p.clone(),
            None if !movie.game.is_empty() => std::path::PathBuf::from(&movie.game),
            None => {
                eprintln!("Error: pass <GAME_PATH> or set 'game:' in the movie file.");
                std::process::exit(1);
            }
        };
        if !game_path.exists() {
            // Allow repo-relative paths stored in the movie header.
            let alt = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|p| p.parent())
                .map(|root| root.join(&movie.game))
                .filter(|p| p.exists());
            match alt {
                Some(p) => return run_replay_mode(&cli, &p, movie),
                None => {
                    eprintln!("Error: Game file not found: {}", game_path.display());
                    std::process::exit(1);
                }
            }
        }
        return run_replay_mode(&cli, &game_path, movie);
    }

    // Validate game path
    let game_path = match &cli.game_path {
        Some(p) => p.clone(),
        None => {
            eprintln!("Error: No game file specified.");
            eprintln!("Usage: native32-emu [OPTIONS] <GAME_PATH>");
            eprintln!("Run with --help for more information.");
            std::process::exit(1);
        }
    };

    if !game_path.exists() {
        eprintln!("Error: Game file not found: {}", game_path.display());
        std::process::exit(1);
    }

    log::info!("Loading game: {}", game_path.display());

    // Create the shared emulator core
    let mut emu = Emulator::from_path(game_path.clone(), cli.volume)?;

    // Apply key remappings (standalone-only feature)
    let key_remappings = cli.parse_key_remappings();
    emu.input.remap(&key_remappings);

    for cheat in &cli.cheats {
        if let Err(e) = emu.cheats.add_code(cheat) {
            log::warn!("Ignoring invalid cheat '{}': {}", cheat, e);
        }
    }

    emu.set_cheat_debug_logging(cli.debug_cheats, cli.cheat_debug_interval);
    emu.set_cheat_debug_variable_filter(cli.cheat_debug_filter.as_deref());
    // Apply typematic key-repeat timing (matches the hardware keypad driver).
    emu.input
        .set_repeat_timing(cli.repeat_delay, cli.repeat_period);

    // Apply shared core settings (also exposed as libretro core options).
    emu.input.set_swap_ab(cli.swap_ab);
    emu.set_auto_skip_cutscenes(cli.auto_skip_cutscenes);

    let resolution = emu.reader.resolution;
    let display_width = resolution.0 * cli.scale;
    let display_height = resolution.1 * cli.scale;

    let use_custom_scaling = cli.filter != "nearest";
    let mut scaler = Scaler::new();
    match cli.filter.as_str() {
        "bicubic" => scaler.set_filter(ScaleFilter::Bicubic),
        "xbrz" => scaler.set_filter(ScaleFilter::Xbrz),
        _ => {} // "nearest" and "bilinear" use default
    }

    // For fullscreen, get screen resolution before creating the window
    // so minifb creates the window at the correct size from the start.
    let (window_width, window_height) = if cli.fullscreen {
        screen::get_screen_size()
    } else {
        (display_width as usize, display_height as usize)
    };

    // When using a custom filter we scale the buffer ourselves, so tell minifb
    // to do a plain 1:1 stretch (our buffer already matches the window size).
    // For nearest-neighbor we keep AspectRatioStretch so minifb does the work.
    let window_opts = minifb::WindowOptions {
        resize: !cli.fullscreen,
        borderless: cli.fullscreen,
        scale_mode: if use_custom_scaling {
            minifb::ScaleMode::Stretch
        } else {
            minifb::ScaleMode::AspectRatioStretch
        },
        ..Default::default()
    };

    let mut window = minifb::Window::new(
        "Native32 Emulator",
        window_width,
        window_height,
        window_opts,
    )
    .context("Failed to create window")?;

    // Apply fullscreen settings
    if cli.fullscreen {
        window.topmost(true);
        window.set_position(0, 0);
    }

    // Limit to 30fps
    window.set_target_fps(30);

    let frame_duration = Duration::from_millis(1000 / 30);

    // Physical gamepad backend (keyboard remains available either way).
    let mut gamepad = GamepadMapper::new(!cli.no_gamepad);

    // Optional: record windowed human play to a text/JSON input movie.
    let mut input_log = match &cli.record_input {
        Some(path) => {
            let mut log = InputLog::create(path.clone(), game_path.display().to_string())?;
            log.movie_mut().auto_skip_cutscenes = cli.auto_skip_cutscenes;
            for cheat in &cli.cheats {
                log.movie_mut().cheats.push(cheat.clone());
            }
            log::info!("Recording input movie to {}", path.display());
            Some(log)
        }
        None => None,
    };

    // Main emulation loop
    let mut frame_count: u32 = 0;
    let screenshot_path = cli.screenshot.clone();
    // Debounce counter: after returning to a parent via ESC/Select, suppress
    // further back detections for a few frames so the release is not re-triggered.
    let mut esc_cooldown: u32 = 0;

    while window.is_open() {
        // Poll the gamepad first so Select can act as a host back action
        // alongside ESC (return to a parent SMF or exit).
        let gamepad_keys = gamepad.pressed_keycodes();
        let gamepad_select = gamepad.select_just_pressed();

        // Handle ESC / gamepad Select: return to a parent SMF or exit.
        if esc_cooldown > 0 {
            esc_cooldown -= 1;
        } else if window.is_key_down(minifb::Key::Escape) || gamepad_select {
            match emu.try_return_to_parent() {
                Ok(true) => {
                    esc_cooldown = 15; // ~0.5s at 30fps, enough for key release
                    continue;
                }
                Ok(false) => break,
                Err(e) => {
                    log::error!("Failed to reload parent SMF: {}", e);
                    break;
                }
            }
        }

        let frame_start = Instant::now();

        // Feed keyboard + gamepad state into the shared core; the core applies
        // typematic filtering and --swap-ab. Tick consumes button actions.
        let mut pressed = emu.input.get_pressed_keycodes(&window);
        for keycode in gamepad_keys {
            if !pressed.contains(&keycode) {
                pressed.push(keycode);
            }
        }
        emu.set_buttons(&pressed);
        if let Some(log) = input_log.as_mut() {
            log.record_frame(u64::from(frame_count), &pressed);
        }
        // Allow skipping logo/cutscene videos with the A or B button, or
        // automatically when auto-skip is enabled.
        if emu.is_cutscene_active()
            && (emu.auto_skip_cutscenes || pressed.contains(&0x4000) || pressed.contains(&0x8800))
        {
            emu.skip_cutscene();
        }

        // Tick emulation (handles content switching internally)
        emu.tick();

        // Draw frame
        emu.draw();

        // Draw gamepad overlay if enabled
        if cli.show_gamepad {
            let pressed_set: std::collections::HashSet<u16> = pressed.iter().copied().collect();
            GamepadOverlay::draw(
                &mut emu.renderer.buffer,
                resolution.0,
                resolution.1,
                cli.scale,
                &pressed_set,
            );
        }

        // Update window
        if use_custom_scaling {
            // Scale the native-resolution buffer to the display size using the
            // bilinear scaler before handing it to minifb.
            let scaled = scaler.scale(
                &emu.renderer.buffer,
                resolution.0,
                resolution.1,
                window_width as u32,
                window_height as u32,
            );
            window
                .update_with_buffer(scaled, window_width, window_height)
                .context("Failed to update display")?;
        } else {
            window
                .update_with_buffer(
                    &emu.renderer.buffer,
                    resolution.0 as usize,
                    resolution.1 as usize,
                )
                .context("Failed to update display")?;
        }

        frame_count += 1;

        // Take screenshot if requested
        if let Some(ref path) = screenshot_path {
            if frame_count >= cli.screenshot_frames {
                emu.renderer
                    .save_screenshot(path)
                    .context("Failed to save screenshot")?;
                log::info!("Screenshot saved to: {}", path.display());
                break;
            }
        }

        // Frame timing
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }

    log::info!("Emulator exited normally");
    if let Some(mut log) = input_log.take() {
        match log.finish(u64::from(frame_count)) {
            Ok(path) => log::info!(
                "Input movie saved: {} ({} frames). Replay with --replay.",
                path.display(),
                frame_count
            ),
            Err(e) => log::error!("Failed to save input movie: {e}"),
        }
    }
    Ok(())
}

fn run_replay_mode(cli: &Cli, game_path: &std::path::Path, movie: InputMovie) -> Result<()> {
    let config = ReplayConfig {
        movie,
        dump_frames_dir: cli.dump_frames.clone(),
        dump_every: cli.dump_every.max(1),
        record_audio: cli.record_audio.clone(),
        max_frames: cli.max_frames,
        volume: cli.volume,
    };
    log::info!("Replaying movie on {}", game_path.display());
    let result = run_replay(game_path, &config)?;
    log::info!(
        "Replay done: frames={} dumped={} content={}",
        result.frames,
        result.dumped_frames,
        result.final_content
    );
    if let Some(path) = result.audio_path {
        log::info!("Audio saved: {}", path.display());
    }
    Ok(())
}
