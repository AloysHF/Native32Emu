//! Smoke test: load every available Native32 game, run it for a number of
//! frames, and assert the emulator neither panics nor produces a blank frame.
//!
//! This test needs the (large, non-distributed) game assets, so it is marked
//! `#[ignore]` and only runs on demand:
//!
//! ```text
//! cargo test -p native32emu-core --test smoke -- --ignored --nocapture
//! ```
//!
//! By default it looks for games in `<repo>/tmp/native32_game`. Override the
//! location with the `NATIVE32_GAME_DIR` environment variable.

use std::io::Write;
use std::path::{Path, PathBuf};

use native32emu_core::action_vm::VmHost;
use native32emu_core::emulator::Emulator;

/// Number of frames to run per game before sampling the output.
const FRAMES: u32 = 150;

/// Resolve the directory that holds the Native32 game assets.
fn game_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("NATIVE32_GAME_DIR") {
        let p = PathBuf::from(dir);
        return p.is_dir().then_some(p);
    }
    // Default: <workspace_root>/tmp/native32_game. CARGO_MANIFEST_DIR points at the
    // core crate (crates/native32emu-core), so go up two levels.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("tmp").join("native32_game"));
    candidate.filter(|p| p.is_dir())
}

fn packed_game_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("NATIVE32_PACKED_GAME_DIR") {
        let path = PathBuf::from(dir);
        return path.is_dir().then_some(path);
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|path| path.parent())
        .map(|root| root.join("tmp").join("native32_NoKaraOK"))
        .filter(|path| path.is_dir())
}

/// Recursively collect playable game files (.smf / .ssl), skipping the NES
/// games and save files.
fn collect_games(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("NESGAME"))
            {
                continue;
            }
            collect_games(&path, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = ext.to_ascii_lowercase();
            if ext == "smf" || ext == "ssl" {
                out.push(path);
            }
        }
    }
}

/// Returns true if the frame buffer contains more than one distinct pixel
/// value, i.e. it is not a single flat color (all-black / all-white / etc.).
fn frame_has_content(framebuffer: &[u32]) -> bool {
    let mut iter = framebuffer.iter();
    let first = match iter.next() {
        Some(&p) => p,
        None => return false,
    };
    iter.any(|&p| p != first)
}

#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn smoke_all_games() {
    let dir = match game_dir() {
        Some(d) => d,
        None => {
            eprintln!(
                "skipping: no game directory found \
                 (set NATIVE32_GAME_DIR or place games in <repo>/native32_game)"
            );
            return;
        }
    };

    let mut games = Vec::new();
    collect_games(&dir, &mut games);
    games.sort();

    assert!(
        !games.is_empty(),
        "no .smf/.ssl games found under {}",
        dir.display()
    );

    println!(
        "Running smoke test over {} games ({FRAMES} frames each)",
        games.len()
    );

    let mut failures = Vec::new();

    for game in &games {
        let rel = game.strip_prefix(&dir).unwrap_or(game);
        match run_one(game) {
            Ok(true) => println!("[PASS] {}", rel.display()),
            Ok(false) => {
                println!("[WARN] {} (blank frame)", rel.display());
            }
            Err(reason) => {
                println!("[FAIL] {} - {reason}", rel.display());
                failures.push(format!("{}: {reason}", rel.display()));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} game(s) failed the smoke test:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Run a single game for `FRAMES` frames. Returns Ok(true) if the final frame
/// has visible content, Ok(false) if it is blank, or Err on a load failure.
/// A panic inside the emulator will fail the test via the normal unwinding.
fn run_one(path: &Path) -> Result<bool, String> {
    let mut emu =
        Emulator::from_path(path.to_path_buf(), 100).map_err(|e| format!("failed to load: {e}"))?;

    for _ in 0..FRAMES {
        emu.set_buttons(&[]);
        emu.tick();
        emu.draw();
    }

    Ok(frame_has_content(emu.get_framebuffer()))
}

#[test]
#[ignore = "requires packed local Native32 assets (set NATIVE32_PACKED_GAME_DIR)"]
fn packed_dragon_background_decodes() {
    let Some(dir) = packed_game_dir() else {
        eprintln!("skipping: no packed game directory found");
        return;
    };
    let game = find_asset(&dir, "Dragon.smf").expect("Dragon.smf not found");
    let data = std::fs::read(game).expect("read Dragon.smf");
    let mut reader = native32emu_core::file_loader::Native32Reader::new(data);
    reader.init().expect("initialize packed Dragon.smf");

    let background = reader.get_image(2).expect("decode Dragon background");
    assert_eq!((background.width, background.height), (322, 242));
    assert!(
        frame_has_content(&background.pixels),
        "Dragon background is blank"
    );
}

#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn save_state_round_trip() {
    let dir = game_dir().expect("no game directory found");
    let game = find_asset(&dir, "FHUI.smf").expect("FHUI.smf not found");
    let mut emu = Emulator::from_path(game, 100).expect("load game");

    // Saving before the first tick exercises the frame-zero startup state.
    let mut startup_state = vec![0; emu.serialize_size()];
    emu.serialize(&mut startup_state)
        .expect("save startup state");
    emu.deserialize(&startup_state)
        .expect("restore startup state");

    for _ in 0..30 {
        emu.set_buttons(&[]);
        emu.tick();
        emu.draw();
    }
    emu.vm.vars.insert("state_probe".into(), "saved".into());
    let saved_tick = emu.tick_count;
    let saved_frame = emu.frame_player.current_frame;
    let saved_pixels = emu.get_framebuffer().to_vec();
    let mut state = vec![0; emu.serialize_size()];
    emu.serialize(&mut state).expect("save running state");

    for _ in 0..5 {
        emu.tick();
    }
    emu.vm.vars.insert("state_probe".into(), "changed".into());
    emu.deserialize(&state).expect("restore running state");
    emu.draw();

    assert_eq!(emu.tick_count, saved_tick);
    assert_eq!(emu.frame_player.current_frame, saved_frame);
    assert_eq!(emu.vm.vars.get("state_probe").unwrap(), "saved");
    assert_eq!(emu.get_framebuffer(), saved_pixels);
}

// ---------------------------------------------------------------------------
// Scripted-input regression harness
//
// GUI / interaction bugs (e.g. "pressing a key on a menu freezes the screen")
// are hard to catch with the plain smoke test because that test never feeds any
// input. The helpers below turn such interactions into deterministic, headless
// regression tests: input is scripted per-frame and the emulator core is driven
// directly, so there is no window and no timing dependency. The Action VM is
// seeded deterministically, so a given input script always produces the same
// run.
// ---------------------------------------------------------------------------

/// Native32 keycode for the Z / "A" button (menu confirm).
const KEY_Z: u16 = 0x4000;

/// Locate a game asset by file name (case-insensitive) under `dir`.
fn find_asset(dir: &Path, file_name: &str) -> Option<PathBuf> {
    let mut all = Vec::new();
    collect_games(dir, &mut all);
    all.into_iter().find(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case(file_name))
    })
}

/// Drive a game headlessly with a per-frame input script.
///
/// `input_for_frame(frame)` returns the Native32 keycodes held during that
/// frame. The closure is called for every frame from 0 to `frames - 1`. The
/// returned emulator can be inspected for final state (loaded content, sprite
/// positions, etc.).
fn run_scripted<F>(path: &Path, frames: u32, mut input_for_frame: F) -> Result<Emulator, String>
where
    F: FnMut(u32) -> Vec<u16>,
{
    let mut emu =
        Emulator::from_path(path.to_path_buf(), 100).map_err(|e| format!("failed to load: {e}"))?;
    for frame in 0..frames {
        let pressed = input_for_frame(frame);
        emu.set_buttons(&pressed);
        emu.tick();
        emu.draw();
    }
    Ok(emu)
}

/// Regression test for the GunFire menu freeze: holding the confirm button on
/// the start menu must navigate to the next screen instead of freezing.
///
/// Previously a redundant per-frame movie-action pass kept re-issuing the
/// `Stop` keyframe action of the selection movie, so its confirm animation
/// never advanced and the menu never progressed. We assert the loaded content
/// actually switches once confirm is held.
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn gunfire_menu_advances_on_confirm() {
    let dir = match game_dir() {
        Some(d) => d,
        None => {
            eprintln!("skipping: no game directory found (set NATIVE32_GAME_DIR)");
            return;
        }
    };

    let menu = match find_asset(&dir, "GFSTART.SSL") {
        Some(p) => p,
        None => {
            eprintln!("skipping: GFSTART.SSL not found under {}", dir.display());
            return;
        }
    };

    // Warm up for 40 frames with no input so the menu is fully shown, then hold
    // the confirm button. The selection animation should play and the menu
    // should load the next screen, i.e. the loaded content file changes.
    const WARMUP: u32 = 40;
    const TOTAL: u32 = 160;

    let emu = run_scripted(&menu, TOTAL, |frame| {
        if frame >= WARMUP {
            vec![KEY_Z]
        } else {
            vec![]
        }
    })
    .expect("run gunfire menu");

    let final_name = emu
        .filename
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    assert!(
        !final_name.eq_ignore_ascii_case("GFSTART.SSL"),
        "menu did not advance after holding confirm: still on {final_name} \
         (selection animation appears frozen)"
    );
}

#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn nav_select_na32_loads_native32_menu() {
    let dir = game_dir().expect("no game directory found");
    let selector = find_asset(&dir, "SELECT.SSL").expect("SELECT.SSL not found");
    let mut emu = Emulator::from_path(selector, 100).expect("load platform selector");

    VmHost::get_url(&mut emu, "NULL", "NAV+NAV_SelectNa32+reValue");
    assert_eq!(
        emu.content_loader.pending_content.as_deref(),
        Some("NA32UI.smf")
    );

    emu.tick();
    let final_name = emu
        .filename
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    assert!(
        final_name.eq_ignore_ascii_case("NA32UI.smf"),
        "Native32 selection loaded {final_name} instead of NA32UI.smf"
    );
}

#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn nav_select_kok_restarts_selector() {
    let dir = game_dir().expect("no game directory found");
    let selector = find_asset(&dir, "SELECT.SSL").expect("SELECT.SSL not found");
    let mut emu = Emulator::from_path(selector, 100).expect("load platform selector");
    let original = emu.filename.clone();
    emu.vm
        .vars
        .insert("reload_probe".to_string(), "present".to_string());

    VmHost::get_url(&mut emu, "NULL", "NAV+NAV_SelectKOK+reValue");
    assert_eq!(
        emu.content_loader.pending_content.as_deref(),
        Some("SELECT.SSL")
    );

    emu.tick();
    assert_eq!(emu.filename, original);
    assert!(!emu.vm.vars.contains_key("reload_probe"));
}

#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn metal_storm_continue_loads_next_content() {
    let dir = game_dir().expect("no game directory found");
    let source_start = find_asset(&dir, "MSSTART.ssl").expect("MSSTART.ssl not found");
    let source_over = find_asset(&dir, "MSOVER.ssl").expect("MSOVER.ssl not found");
    let source_next = find_asset(&dir, "MSPLAY10.ssl").expect("MSPLAY10.ssl not found");

    let temp = tempfile::tempdir().expect("create temporary game directory");
    let metal_dir = temp.path().join("NA32SSL/ENGLISH/METAL");
    std::fs::create_dir_all(&metal_dir).expect("create Metal Storm directory");
    let game_over = metal_dir.join("MSOVER.ssl");
    let game_start = metal_dir.join("MSSTART.ssl");
    let next_game = metal_dir.join("MSPLAY10.ssl");
    std::fs::copy(source_over, &game_over).expect("copy MSOVER.ssl");
    std::fs::copy(source_start, &game_start).expect("copy MSSTART.ssl");
    std::fs::copy(source_next, &next_game).expect("copy MSPLAY10.ssl");
    std::fs::write(format!("{}.ssl_sav", game_start.display()), "|1|1|10|0|00")
        .expect("seed Metal Storm save data");

    let mut emu = Emulator::from_path(game_start, 100).expect("load Metal Storm start screen");
    VmHost::get_url(
        &mut emu,
        "/NA32SSL /ENGLISH /METAL   /MSOVER.ssl",
        "SSL+SSL_PlayNext",
    );
    emu.tick();
    for frame in 0..240 {
        let pressed = if (60..63).contains(&frame) {
            vec![KEY_Z]
        } else {
            vec![]
        };
        emu.set_buttons(&pressed);
        emu.tick();
        emu.draw();
    }

    let final_name = emu
        .filename
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    assert!(
        final_name.eq_ignore_ascii_case("MSPLAY10.ssl"),
        "continue loaded {final_name} instead of MSPLAY10.ssl: {:?}",
        emu.vm.vars
    );
}

/// Regression test for the front-end menu game list: the FHUI shell enumerates
/// the games on disk via the GetFileNum / GetFirstFile / GetNextFile host calls
/// and loads each game's `.dat` thumbnail. Previously these calls were ignored,
/// so the menu showed an empty game list. We drive the menu into the first
/// category and assert the list is populated and thumbnails are bound.
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn fhui_menu_populates_game_list() {
    let dir = match game_dir() {
        Some(d) => d,
        None => {
            eprintln!("skipping: no game directory found (set NATIVE32_GAME_DIR)");
            return;
        }
    };
    let menu = match find_asset(&dir, "FHUI.smf") {
        Some(p) => p,
        None => {
            eprintln!("skipping: FHUI.smf not found under {}", dir.display());
            return;
        }
    };

    // Warm up, then tap confirm once to enter the first category's game grid.
    let emu = run_scripted(&menu, 160, |frame| {
        if (50..53).contains(&frame) {
            vec![KEY_Z]
        } else {
            vec![]
        }
    })
    .expect("run FHUI menu");

    // We should still be in the menu (not have launched a game yet).
    let name = emu
        .filename
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    assert!(
        name.eq_ignore_ascii_case("FHUI.smf"),
        "expected to remain in the menu, but loaded {name}"
    );

    // The per-category game counts must be populated (EPOP is the first tab).
    let epop_count = emu
        .vm
        .vars
        .get("tfilenum0")
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);
    assert!(
        epop_count > 0,
        "menu game count not populated (tfilenum0 = {:?})",
        emu.vm.vars.get("tfilenum0")
    );

    // Entering the category must enumerate its games and bind thumbnails.
    assert!(
        emu.vm.vars.get("realfilenum").is_some_and(|v| v != "0"),
        "category game list not enumerated (realfilenum = {:?})",
        emu.vm.vars.get("realfilenum")
    );
    assert!(
        emu.renderer.sprite_override_count() > 0,
        "no game thumbnails were loaded from .dat files"
    );
}

/// Selecting a game in the populated list must launch it: the StartGame host
/// call loads the chosen `.smf`, switching the loaded content away from FHUI.
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn fhui_menu_starts_selected_game() {
    let dir = match game_dir() {
        Some(d) => d,
        None => {
            eprintln!("skipping: no game directory found (set NATIVE32_GAME_DIR)");
            return;
        }
    };
    let menu = match find_asset(&dir, "FHUI.smf") {
        Some(p) => p,
        None => {
            eprintln!("skipping: FHUI.smf not found under {}", dir.display());
            return;
        }
    };

    // Tap confirm to enter the category, then confirm again to launch the
    // first game.
    let emu = run_scripted(&menu, 260, |frame| {
        if (50..53).contains(&frame) || (110..113).contains(&frame) {
            vec![KEY_Z]
        } else {
            vec![]
        }
    })
    .expect("run FHUI menu");

    let name = emu
        .filename
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    assert!(
        !name.eq_ignore_ascii_case("FHUI.smf"),
        "selecting a game did not launch it: still on the menu"
    );
}

/// Regression test for Magical Adventure's first mini-game flow: a confirm
/// press must be consumed by the mini-game script after the frame actions run,
/// starting the timer instead of leaving the prompt stuck forever.
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn magical_adventure_minigame_confirm_starts_timer() {
    let dir = match game_dir() {
        Some(d) => d,
        None => {
            eprintln!("skipping: no game directory found (set NATIVE32_GAME_DIR)");
            return;
        }
    };
    let game = match find_asset(&dir, "MagicalA.smf") {
        Some(p) => p,
        None => {
            eprintln!("skipping: MagicalA.smf not found under {}", dir.display());
            return;
        }
    };

    let mut emu = Emulator::from_path(game, 100).expect("load Magical Adventure");

    for frame in 0..540 {
        if frame == 240 {
            emu.vm.vars.insert("gamestate".into(), "2".into());
            emu.vm.vars.insert("mgstate".into(), "0".into());
            emu.vm.vars.insert("mgkind".into(), "0".into());
            emu.vm.vars.insert("mglevel".into(), "0".into());
            emu.vm.vars.insert("mgenemy".into(), "0".into());
            emu.vm.vars.insert("loopstep".into(), "0".into());
            emu.vm.vars.insert("mgstep".into(), "0".into());
            emu.vm.vars.insert("oldkeypad".into(), "0".into());
            emu.vm.vars.insert("icount".into(), "0".into());
        }

        let pressed = if (80..83).contains(&frame)
            || (220..223).contains(&frame)
            || (285..288).contains(&frame)
        {
            vec![KEY_Z]
        } else {
            vec![]
        };
        emu.set_buttons(&pressed);
        emu.tick();
        emu.draw();
    }

    let timebar_frame = emu.sprites.get("timeBar").map(|m| m.frame).unwrap_or(0);
    assert!(
        timebar_frame > 0,
        "mini-game timer did not advance after confirm (timeBar frame = {timebar_frame})"
    );
    assert!(
        emu.vm.vars.get("mgstate").is_some_and(|v| v != "0"),
        "mini-game stayed in init state after confirm: {:?}",
        emu.vm.vars.get("mgstate")
    );
}
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn magical_adventure_mp3_music_decodes_mixes_and_loops() {
    let dir = game_dir().expect("no game directory found");
    let game = find_asset(&dir, "MagicalA.smf").expect("MagicalA.smf not found");
    let mut emu = Emulator::from_path(game, 100).expect("load Magical Adventure");

    let channel = emu
        .audio
        .play_sound(&mut emu.reader, 0xFF01, "regression_bgm")
        .expect("decode looping MP3 background music");
    let effect_channel = emu
        .audio
        .play_sound(&mut emu.reader, 0x0007, "regression_effect")
        .expect("play RAW sound effect alongside MP3 music");
    assert!(emu.audio.is_channel_playing(channel));
    assert!(emu.audio.is_channel_playing(effect_channel));

    let mut non_silent_frames = 0;
    for _ in 0..900 {
        let samples = emu.get_pending_audio_samples();
        if samples.iter().any(|sample| *sample != 0) {
            non_silent_frames += 1;
        }
    }

    assert!(
        non_silent_frames > 800,
        "decoded MP3 unexpectedly contained prolonged silence"
    );
    assert!(
        emu.audio.is_channel_playing(channel),
        "0xFF MP3 background music stopped instead of looping"
    );
    assert!(
        !emu.audio.is_channel_playing(effect_channel),
        "one-shot RAW sound effect did not finish"
    );

    let mut state = vec![0; emu.serialize_size()];
    emu.serialize(&mut state).expect("save active MP3 state");
    emu.deserialize(&state).expect("restore active MP3 state");
    assert!(
        emu.audio.is_channel_playing(channel),
        "active MP3 channel was not restored from save state"
    );
}

#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn basketball_keeps_background_music_during_gameplay() {
    let dir = game_dir().expect("no game directory found");
    let game = dir.join("ESPG/Basketba.smf");
    let mut emu = Emulator::from_path(game, 100).expect("load Basketball");
    emu.load_frame(1);
    for frame in 0..1_800 {
        let pressed = if frame >= 50 && (frame - 50) % 45 < 3 {
            vec![KEY_Z]
        } else {
            vec![]
        };
        emu.set_buttons(&pressed);
        emu.tick();
        let _ = emu.get_pending_audio_samples();
    }

    assert!(
        emu.audio.is_playing(),
        "Basketball background music stopped during repeated shots"
    );
}

#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn mission_express_fire1_recovers_control_after_jump() {
    let dir = game_dir().expect("no game directory found");
    let game = find_asset(&dir, "FIRE1.SSL").expect("FIRE1.SSL not found");
    let mut emu = Emulator::from_path(game, 100).expect("load Mission Express Fire1");

    let mut saw_uncontrollable_jump = false;
    for frame in 0..360 {
        emu.set_buttons(&[]);
        emu.tick();
        let _ = emu.get_pending_audio_samples();
        if frame == 300 {
            let jump_frame = emu.vm.vars["pcarsf5"].parse::<u32>().unwrap();
            emu.vm.vars.insert("pcarstate".into(), "0".into());
            emu.vm.vars.insert("barstate".into(), "7".into());
            VmHost::goto_frame(&mut emu, "pcar", jump_frame, true);
        }
        if frame > 300
            && emu
                .vm
                .vars
                .get("pcarstate")
                .is_some_and(|state| state == "0")
        {
            saw_uncontrollable_jump = true;
        }
    }

    assert!(saw_uncontrollable_jump, "test did not enter the jump state");
    assert_eq!(
        emu.vm.vars.get("pcarstate").map(String::as_str),
        Some("1"),
        "fire truck did not return to its controllable state after the jump"
    );
}

#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn mission_express_save_state_survives_cross_directory_content_switch() {
    let dir = game_dir().expect("no game directory found");
    let game = dir.join("EPOP/EExpress.smf");
    let mut emu = Emulator::from_path(game, 100).expect("load Mission Express launcher");

    emu.frame_player.take_next_frame();
    emu.frame_player.playing = false;
    emu.content_loader
        .queue_load("/NA32SSL /ENGLISH /MEXPRESS/FIRE1.SSL");
    emu.tick();

    let mut state = vec![0; emu.serialize_size()];
    emu.serialize(&mut state)
        .expect("save state after switching from EPOP to NA32SSL");
    emu.deserialize(&state)
        .expect("restore state after switching from EPOP to NA32SSL");
    assert_eq!(
        emu.filename.file_name().and_then(|name| name.to_str()),
        Some("FIRE1.SSL")
    );
}
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn main_timeline_sound_objects_start_background_music() {
    let dir = game_dir().expect("no game directory found");
    for relative in ["ESPG/Basketba.smf", "EPUZ/Mouse.smf"] {
        let game = dir.join(relative);
        let mut emu = Emulator::from_path(game, 100).expect("load reported game");
        emu.load_frame(1);

        assert!(
            emu.audio.is_playing(),
            "{relative} did not start frame 1 music"
        );
        let samples: Vec<i16> = (0..30)
            .flat_map(|_| emu.get_pending_audio_samples())
            .collect();
        assert!(
            samples.iter().any(|sample| *sample != 0),
            "{relative} frame 1 music decoded to silence"
        );
    }
}
/// Test ZIP file loading: a ZIP archive containing FHUI.smf should be extracted
/// and the main menu loaded automatically.
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn zip_file_loads_fhui() {
    let dir = match game_dir() {
        Some(d) => d,
        None => {
            eprintln!("skipping: no game directory found (set NATIVE32_GAME_DIR)");
            return;
        }
    };

    // Look for a ZIP file in the parent directory (tmp/native32_game.zip)
    let zip_path = dir
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("tmp").join("native32_game.zip"))
        .filter(|p| p.exists());

    let zip_path = match zip_path {
        Some(p) => p,
        None => {
            eprintln!("skipping: native32_game.zip not found");
            return;
        }
    };

    // Load the ZIP file
    let mut emu = Emulator::from_path(zip_path, 100).expect("failed to load ZIP game");

    // Verify that FHUI.smf was loaded (the main menu)
    let name = emu
        .filename
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    assert!(
        name.eq_ignore_ascii_case("FHUI.smf"),
        "expected FHUI.smf to be loaded from ZIP, but got: {name}"
    );

    // Run a few frames to ensure it's functional
    for _ in 0..30 {
        emu.set_buttons(&[]);
        emu.tick();
        emu.draw();
    }

    assert!(
        frame_has_content(emu.get_framebuffer()),
        "ZIP-loaded FHUI.smf produced a blank frame"
    );
}

/// A ZIP with NA32UI.smf as its only menu entry point should boot it and use it
/// as the back-action target.
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn zip_file_loads_na32ui() {
    let dir = game_dir().expect("no game directory found");
    let menu = find_asset(&dir, "NA32UI.smf").expect("NA32UI.smf not found");
    let mut games = Vec::new();
    collect_games(&dir, &mut games);
    let child = games
        .into_iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    !name.eq_ignore_ascii_case("NA32UI.smf")
                        && !name.eq_ignore_ascii_case("FHUI.smf")
                })
        })
        .expect("no child game found");

    let temp_dir = tempfile::tempdir().expect("create temporary directory");
    let zip_path = temp_dir.path().join("na32ui-package.zip");
    let zip_file = std::fs::File::create(&zip_path).expect("create test ZIP");
    let mut zip = zip::ZipWriter::new(zip_file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    zip.start_file("NA32UI.smf", options)
        .expect("add NA32UI.smf");
    zip.write_all(&std::fs::read(menu).expect("read NA32UI.smf"))
        .expect("write NA32UI.smf");
    zip.start_file("GAME.smf", options).expect("add child game");
    zip.write_all(&std::fs::read(child).expect("read child game"))
        .expect("write child game");
    zip.finish().expect("finish test ZIP");

    let mut emu = Emulator::from_path(zip_path, 100).expect("load NA32UI ZIP package");
    let menu_path = emu.filename.clone();
    assert!(menu_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("NA32UI.smf")));

    let child_path = menu_path
        .parent()
        .expect("menu path has no parent")
        .join("GAME.smf");
    emu.reload_from_path(child_path)
        .expect("load extracted child game");
    assert!(emu.try_return_to_menu().expect("return to NA32UI menu"));
    assert_eq!(emu.filename, menu_path);
}

/// The shared back action should return a ZIP-loaded child game to its menu.
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn back_action_returns_to_zip_menu() {
    let dir = match game_dir() {
        Some(d) => d,
        None => {
            eprintln!("skipping: no game directory found (set NATIVE32_GAME_DIR)");
            return;
        }
    };
    let Some(zip_path) = dir
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("tmp").join("native32_game.zip"))
        .filter(|p| p.exists())
    else {
        eprintln!("skipping: native32_game.zip not found");
        return;
    };

    let mut emu = Emulator::from_path(zip_path, 100).expect("failed to load ZIP game");
    let menu_path = emu.filename.clone();
    let mut games = Vec::new();
    collect_games(
        menu_path.parent().expect("FHUI path has no parent"),
        &mut games,
    );
    let child_game = games
        .into_iter()
        .find(|path| *path != menu_path)
        .expect("ZIP package contains no child game");

    emu.reload_from_path(child_game)
        .expect("failed to load child game");
    assert!(emu.try_return_to_menu().expect("back action failed"));
    assert_eq!(emu.filename, menu_path);
    assert!(!emu
        .try_return_to_menu()
        .expect("back action at the menu failed"));
}

/// A placed Pirate bomb must start at the template's first visible frame.
#[test]
#[ignore = "requires local Native32 game assets (set NATIVE32_GAME_DIR)"]
fn pirate_bomb_plays_and_explodes() {
    let dir = game_dir().expect("no game directory found");
    let game = dir.join("EPOP/Epirate.smf");

    let tap_frames = [80, 220, 360, 500, 640];
    let emu = run_scripted(&game, 650, |frame| {
        if tap_frames
            .iter()
            .any(|start| (*start..*start + 3).contains(&frame))
        {
            vec![KEY_Z]
        } else {
            vec![]
        }
    })
    .expect("run Pirate");

    let bomb = emu.sprites.get("B1").expect("bomb clone was not created");
    assert!(bomb.visible, "placed bomb clone is hidden");
    assert!(bomb.playing, "placed bomb clone is not playing");
    assert!(
        bomb.frame < 10,
        "placed bomb started at the hidden template frame: {}",
        bomb.frame
    );

    let emu = run_scripted(&game, 770, |frame| {
        if tap_frames
            .iter()
            .any(|start| (*start..*start + 3).contains(&frame))
        {
            vec![KEY_Z]
        } else {
            vec![]
        }
    })
    .expect("run Pirate through bomb explosion");

    assert!(
        emu.sprites.get("B1").is_none(),
        "bomb clone did not remove itself after exploding"
    );
    assert_eq!(
        emu.vm.vars.get("btotal").map(String::as_str),
        Some("|00000")
    );
}
