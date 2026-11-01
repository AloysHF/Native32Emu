// The entry check runs before any file is parsed, so these tests need no assets.

use native32emu_core::emulator::Emulator;

fn write_scene(dir: &std::path::Path) -> std::path::PathBuf {
    let scene_dir = dir.join("NA32SSL").join("ENGLISH").join("BBLADE");
    std::fs::create_dir_all(&scene_dir).expect("create scene directory");
    let scene = scene_dir.join("BBSTART.SSL");
    std::fs::write(&scene, b"not a real scene").expect("write scene");
    scene
}

#[test]
fn ssl_scene_cannot_start_a_game() {
    let dir = tempfile::tempdir().expect("create temporary directory");
    let scene = write_scene(dir.path());

    let error = Emulator::from_path(scene, 100)
        .err()
        .expect("an SSL scene must not start a session")
        .to_string();

    assert!(error.contains("BBSTART.SSL"), "{error}");
    assert!(error.contains("game entry point"), "{error}");
    assert!(error.contains(".smf"), "{error}");
}

#[test]
fn application_path_passes_the_entry_check() {
    // A missing `.smf` still fails, but at load time — proving the rejection
    // above is about the extension, not about a missing file.
    let dir = tempfile::tempdir().expect("create temporary directory");
    let application = dir.path().join("EBBLADE.smf");

    let error = Emulator::from_path(application, 100)
        .err()
        .expect("missing content must fail")
        .to_string();

    assert!(!error.contains("game entry point"), "{error}");
}
