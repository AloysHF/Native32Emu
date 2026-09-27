//! Real-asset regression and reproducible captures for Bloody Blade.
//! Run with: cargo test -p native32emu-core --test yuv_color -- --ignored --nocapture

use std::path::{Path, PathBuf};

use native32emu_core::emulator::Emulator;
use native32emu_core::file_loader::Native32Reader;
use native32emu_core::input_handler::KEYCODE_A;

fn asset_root() -> PathBuf {
    std::env::var_os("NATIVE32_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp/native32_game"))
}

fn capture(path: &Path, width: u32, height: u32, pixels: &[u32]) {
    let rgba: Vec<u8> = pixels
        .iter()
        .flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8, (p >> 24) as u8])
        .collect();
    image::save_buffer(path, &rgba, width, height, image::ColorType::Rgba8).unwrap();
}

#[test]
#[ignore = "requires local Bloody Blade assets (set NATIVE32_GAME_DIR)"]
fn bloody_blade_shadow_and_gameplay() {
    let root = asset_root();
    let output = std::env::var_os("NATIVE32_COLOR_CAPTURE_DIR").map(PathBuf::from);
    if let Some(dir) = &output {
        std::fs::create_dir_all(dir).unwrap();
    }

    // Isolate saves so captures are deterministic and never change user progress.
    let sandbox = tempfile::tempdir().unwrap();
    let launcher_dir = sandbox.path().join("EPOP");
    let scene_dir = sandbox.path().join("NA32SSL/ENGLISH/BBLADE");
    std::fs::create_dir_all(&launcher_dir).unwrap();
    std::fs::create_dir_all(&scene_dir).unwrap();
    std::fs::copy(
        root.join("EPOP/EBBLADE.smf"),
        launcher_dir.join("EBBLADE.smf"),
    )
    .unwrap();
    for entry in std::fs::read_dir(root.join("NA32SSL/ENGLISH/BBLADE")).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().is_some_and(|ext| ext == "SSL") {
            std::fs::copy(entry.path(), scene_dir.join(entry.file_name())).unwrap();
        }
    }
    let mut emu = Emulator::from_path(launcher_dir.join("EBBLADE.smf"), 0).unwrap();
    emu.set_auto_skip_cutscenes(true);
    for frame in 0..1200 {
        emu.set_buttons(
            if [200, 260, 320, 380, 600, 660, 720, 780].contains(&frame) {
                &[KEYCODE_A]
            } else if (800..900).contains(&frame) {
                &[0x0400]
            } else {
                &[]
            },
        );
        emu.tick();
        emu.draw();
        // Drain audio to keep headless captures bounded.
        emu.get_pending_audio_samples();
        if [199, 259, 319, 379, 599, 999, 1199].contains(&frame) {
            println!("frame {}: {}", frame + 1, emu.filename.display());
            if let Some(dir) = &output {
                let (w, h) = emu.get_resolution();
                capture(
                    &dir.join(format!("frame-{}.png", frame + 1)),
                    w,
                    h,
                    emu.get_framebuffer(),
                );
            }
        }
    }
    assert_eq!(emu.filename.file_name().unwrap(), "BBPLAY10.SSL");

    let mut reader = Native32Reader::new(
        std::fs::read(root.join("NA32SSL/ENGLISH/BBLADE/BBPLAY10.SSL")).unwrap(),
    );
    reader.init().unwrap();
    let sprite = reader.get_image(636).unwrap();
    assert_eq!((sprite.width, sprite.height), (46, 74));
    if let Some(dir) = &output {
        capture(
            &dir.join("sprite-636.png"),
            sprite.width,
            sprite.height,
            &sprite.pixels,
        );
    }

    // Decode the raw quads independently to locate black/transparent shadow stippling.
    let table = reader.base + reader.image_idx as usize + 4 * (636 - 1);
    let offset = u32::from_le_bytes(reader.data[table..table + 4].try_into().unwrap()) as usize;
    let start = reader.base + offset;
    let data = &reader.data[start..];
    let mut i = 8;
    let mut quads = Vec::new();
    while quads.len() < 23 * 37 {
        let op = u16::from_le_bytes(data[i..i + 2].try_into().unwrap());
        i += 2;
        assert_ne!(op, 0);
        let count = (op & 0x7fff) as usize;
        for _ in 0..count {
            quads.push(<[u8; 6]>::try_from(&data[i..i + 6]).unwrap());
            if op & 0x8000 != 0 {
                i += 6;
            }
        }
        if op & 0x8000 == 0 {
            i += 6;
        }
    }
    let mut black = 0;
    let mut green = 0;
    for (q, quad) in quads.iter().enumerate() {
        if quad == &[0, 16, 16, 0, 0, 0] {
            for (dx, dy) in [(0, 1), (1, 0)] {
                let x = (q % 23) * 2 + dx;
                let y = (q / 23) * 2 + dy;
                let pixel = sprite.pixels[y * 46 + x];
                black += usize::from(pixel == 0xff000000);
                green += usize::from(pixel == 0xff009a00);
            }
            for (dx, dy) in [(0, 0), (1, 1)] {
                let x = (q % 23) * 2 + dx;
                let y = (q / 23) * 2 + dy;
                assert_eq!(sprite.pixels[y * 46 + x], 0, "transparent shadow pixel");
            }
        }
    }
    println!("sprite #636 shadow stipple: {black} black, {green} erroneous green");
    assert_eq!(black, 113, "black shadow stippling was not preserved");
    assert_eq!(green, 0, "zero-chroma shadows decoded as green");
}
