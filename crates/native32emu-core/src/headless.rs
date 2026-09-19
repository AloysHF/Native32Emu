// Headless input-movie replay: dump frames + optional audio for video encode.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::emulator::Emulator;
use crate::movie::InputMovie;

#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub movie: InputMovie,
    /// PNG directory (`%06d.png`). None skips video frames.
    pub dump_frames_dir: Option<PathBuf>,
    /// Dump every N emulated frames (1 = every frame).
    pub dump_every: u32,
    /// WAV path for mixed emulator audio.
    pub record_audio: Option<PathBuf>,
    pub max_frames: u64,
    pub volume: u32,
}

#[derive(Debug, Clone)]
pub struct ReplayResult {
    pub frames: u64,
    pub dumped_frames: u64,
    pub final_content: String,
    pub audio_path: Option<PathBuf>,
}

/// Replay an input movie headlessly and optionally dump frames/audio.
pub fn run_replay(game_path: &Path, config: &ReplayConfig) -> Result<ReplayResult> {
    let capture_audio = config.record_audio.is_some();
    let volume = if capture_audio { 100 } else { config.volume };
    let mut emu = Emulator::from_path(game_path.to_path_buf(), volume)
        .with_context(|| format!("Failed to load {}", game_path.display()))?;
    if config.movie.auto_skip_cutscenes {
        emu.set_auto_skip_cutscenes(true);
    }
    emu.set_audio_capture(capture_audio);
    for code in &config.movie.cheats {
        emu.cheats
            .add_code(code)
            .map_err(|e| anyhow::anyhow!("Invalid cheat '{code}': {e}"))?;
    }

    if let Some(dir) = &config.dump_frames_dir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create {}", dir.display()))?;
        // Remove stale PNGs so FFmpeg %06d cannot pick up a previous run.
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                if entry
                    .path()
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("png"))
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    let dump_every = config.dump_every.max(1);
    let movie_end = config
        .movie
        .events
        .iter()
        .map(|e| e.frame + u64::from(e.hold.max(1)))
        .max()
        .unwrap_or(0);
    let run_frames = if movie_end == 0 {
        config.max_frames.max(1)
    } else {
        // Replay the movie plus a short tail for post-input animation.
        (movie_end + 30).min(config.max_frames.max(1))
    };
    let mut dumped_frames = 0u64;
    let mut audio_pcm: Vec<i16> = Vec::new();
    let mut sample_rate = emu.get_audio_sample_rate() as u32;

    for frame in 0..run_frames {
        let keys = config.movie.keys_at(frame);
        emu.set_buttons(&keys);
        if emu.is_cutscene_active()
            && (config.movie.auto_skip_cutscenes
                || keys.contains(&0x4000)
                || keys.contains(&0x8800))
        {
            emu.skip_cutscene();
        }
        emu.tick();
        emu.draw();
        if capture_audio {
            sample_rate = emu.get_audio_sample_rate() as u32;
            audio_pcm.extend_from_slice(&emu.get_pending_audio_samples());
        }
        if let Some(dir) = &config.dump_frames_dir {
            if frame.is_multiple_of(u64::from(dump_every)) {
                let path = dir.join(format!("{dumped_frames:06}.png"));
                emu.renderer
                    .save_screenshot(&path)
                    .with_context(|| format!("Failed to dump {}", path.display()))?;
                dumped_frames += 1;
            }
        }
    }

    let mut audio_path = None;
    if let Some(path) = &config.record_audio {
        write_wav_pcm16_stereo(path, &audio_pcm, sample_rate)?;
        audio_path = Some(path.clone());
    }

    let final_content = emu
        .filename
        .file_name()
        .map(|n| n.to_string_lossy().to_uppercase())
        .unwrap_or_default();

    Ok(ReplayResult {
        frames: run_frames,
        dumped_frames,
        final_content,
        audio_path,
    })
}

fn write_wav_pcm16_stereo(path: &Path, samples: &[i16], sample_rate: u32) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
    }
    let data_len = (samples.len() * 2) as u32;
    let mut buf = Vec::with_capacity(44 + samples.len() * 2);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_len).to_le_bytes());
    buf.extend_from_slice(b"WAVEfmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    buf.extend_from_slice(&4u16.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, buf)?;
    Ok(())
}
