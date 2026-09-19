// Input movies: record live play to a plain-text file and replay it later.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// One compressed input sample: keys held starting at `frame` for `hold` frames.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputEvent {
    pub frame: u64,
    #[serde(default)]
    pub keys: Vec<u16>,
    #[serde(default = "default_hold")]
    pub hold: u32,
}

fn default_hold() -> u32 {
    1
}

fn default_fps() -> u32 {
    30
}

/// Replayable input movie (plain text `.nmov` or JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputMovie {
    #[serde(default)]
    pub game: String,
    #[serde(default = "default_fps")]
    pub fps: u32,
    #[serde(default)]
    pub auto_skip_cutscenes: bool,
    #[serde(default)]
    pub cheats: Vec<String>,
    #[serde(default)]
    pub events: Vec<InputEvent>,
}

impl Default for InputMovie {
    fn default() -> Self {
        Self {
            game: String::new(),
            fps: default_fps(),
            auto_skip_cutscenes: false,
            cheats: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl InputMovie {
    pub fn new(game: impl Into<String>) -> Self {
        Self {
            game: game.into(),
            ..Self::default()
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read input movie: {}", path.display()))?;
        Self::parse(&text)
            .with_context(|| format!("Failed to parse input movie: {}", path.display()))
    }

    /// JSON when the body starts with `{`; otherwise plain-text `.nmov`.
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.trim_start_matches('\u{feff}');
        if text.trim_start().starts_with('{') {
            return serde_json::from_str(text).context("Failed to parse input movie JSON");
        }
        Self::parse_text(text)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create {}", parent.display()))?;
            }
        }
        let text = if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            serde_json::to_string_pretty(self).context("Failed to serialize movie JSON")?
        } else {
            self.to_text()
        };
        std::fs::write(path, text)
            .with_context(|| format!("Failed to write input movie: {}", path.display()))?;
        Ok(())
    }

    /// Plain-text body:
    /// ```text
    /// # Native32 input movie v1
    /// game: path/to/game.smf
    /// fps: 30
    /// 0 R hold 12
    /// 30 A hold 5
    /// ```
    /// Keys: `L R U D A B` or `0xNNNN`.
    pub fn to_text(&self) -> String {
        let mut out = String::from("# Native32 input movie v1\n");
        out.push_str(&format!("game: {}\n", self.game));
        out.push_str(&format!("fps: {}\n", self.fps));
        out.push_str(&format!(
            "auto_skip_cutscenes: {}\n",
            self.auto_skip_cutscenes
        ));
        for cheat in &self.cheats {
            out.push_str(&format!("cheat: {cheat}\n"));
        }
        out.push_str("# frame keys... [hold N]\n");
        for e in &self.events {
            let keys = e
                .keys
                .iter()
                .map(|k| key_to_name(*k))
                .collect::<Vec<_>>()
                .join(" ");
            if keys.is_empty() {
                out.push_str(&format!("{} hold {}\n", e.frame, e.hold));
            } else {
                out.push_str(&format!("{} {} hold {}\n", e.frame, keys, e.hold));
            }
        }
        out
    }

    fn parse_text(text: &str) -> Result<Self> {
        let mut movie = InputMovie::default();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("game:") {
                movie.game = rest.trim().to_string();
                continue;
            }
            if let Some(rest) = line.strip_prefix("fps:") {
                movie.fps = rest.trim().parse().unwrap_or(default_fps());
                continue;
            }
            if let Some(rest) = line.strip_prefix("auto_skip_cutscenes:") {
                movie.auto_skip_cutscenes = matches!(
                    rest.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
                continue;
            }
            if let Some(rest) = line.strip_prefix("cheat:") {
                let code = rest.trim();
                if !code.is_empty() {
                    movie.cheats.push(code.to_string());
                }
                continue;
            }
            let mut tokens = line.split_whitespace();
            let frame: u64 = tokens
                .next()
                .and_then(|t| t.parse().ok())
                .ok_or_else(|| anyhow::anyhow!("movie line {}: missing frame", i + 1))?;
            let mut keys = Vec::new();
            let mut hold = 1u32;
            let mut expect_hold = false;
            for tok in tokens {
                if expect_hold {
                    hold = tok.parse().unwrap_or(1).max(1);
                    expect_hold = false;
                } else if tok.eq_ignore_ascii_case("hold") {
                    expect_hold = true;
                } else if let Some(code) = parse_key_token(tok) {
                    keys.push(code);
                } else {
                    return Err(anyhow::anyhow!("movie line {}: bad key '{tok}'", i + 1));
                }
            }
            movie.events.push(InputEvent { frame, keys, hold });
        }
        movie.events.sort_by_key(|e| e.frame);
        Ok(movie)
    }

    /// Keys held on `frame` (last covering event wins).
    pub fn keys_at(&self, frame: u64) -> Vec<u16> {
        let mut active = None;
        for e in &self.events {
            let hold = u64::from(e.hold.max(1));
            if e.frame <= frame && frame < e.frame + hold {
                active = Some(e);
            }
        }
        active.map(|e| e.keys.clone()).unwrap_or_default()
    }
}

pub fn key_to_name(code: u16) -> String {
    match code {
        0x0200 => "L".into(),
        0x0400 => "R".into(),
        0x1c00 => "U".into(),
        0x1e00 => "D".into(),
        0x4000 => "A".into(),
        0x8800 => "B".into(),
        other => format!("0x{other:04x}"),
    }
}

pub fn parse_key_token(tok: &str) -> Option<u16> {
    match tok.to_ascii_uppercase().as_str() {
        "L" | "LEFT" => Some(0x0200),
        "R" | "RIGHT" => Some(0x0400),
        "U" | "UP" => Some(0x1c00),
        "D" | "DOWN" => Some(0x1e00),
        "A" => Some(0x4000),
        "B" => Some(0x8800),
        other => u16::from_str_radix(other.trim_start_matches("0x"), 16).ok(),
    }
}

/// Compresses a live button stream into key-change events.
#[derive(Debug, Default)]
pub struct MovieRecorder {
    movie: InputMovie,
    last_keys: Vec<u16>,
    last_change_frame: Option<u64>,
}

impl MovieRecorder {
    pub fn new(game: impl Into<String>) -> Self {
        Self {
            movie: InputMovie::new(game),
            last_keys: Vec::new(),
            last_change_frame: None,
        }
    }

    pub fn movie_mut(&mut self) -> &mut InputMovie {
        &mut self.movie
    }

    pub fn record_frame(&mut self, frame: u64, keys: &[u16]) {
        let mut sorted = keys.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted == self.last_keys {
            return;
        }
        if let Some(start) = self.last_change_frame {
            let hold = frame.saturating_sub(start).max(1) as u32;
            self.movie.events.push(InputEvent {
                frame: start,
                keys: self.last_keys.clone(),
                hold,
            });
        }
        self.last_keys = sorted;
        self.last_change_frame = Some(frame);
    }

    /// Snapshot including an open trailing hold through `end_frame_exclusive`.
    pub fn snapshot(&self, end_frame_exclusive: u64) -> InputMovie {
        let mut movie = self.movie.clone();
        if let Some(start) = self.last_change_frame {
            let hold = end_frame_exclusive.saturating_sub(start).max(1) as u32;
            movie.events.retain(|e| e.frame < start);
            movie.events.push(InputEvent {
                frame: start,
                keys: self.last_keys.clone(),
                hold,
            });
            movie.events.sort_by_key(|e| e.frame);
        }
        movie
    }
}

/// Live input log for human play; periodically flushes to disk.
#[derive(Debug)]
pub struct InputLog {
    path: PathBuf,
    recorder: MovieRecorder,
    last_frame: u64,
    frames_since_flush: u32,
}

impl InputLog {
    pub fn create(path: impl Into<PathBuf>, game: impl Into<String>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("Failed to create {}", parent.display()))?;
            }
        }
        Ok(Self {
            path,
            recorder: MovieRecorder::new(game),
            last_frame: 0,
            frames_since_flush: 0,
        })
    }

    pub fn movie_mut(&mut self) -> &mut InputMovie {
        self.recorder.movie_mut()
    }

    pub fn record_frame(&mut self, frame: u64, keys: &[u16]) {
        self.recorder.record_frame(frame, keys);
        self.last_frame = self.last_frame.max(frame);
        self.frames_since_flush += 1;
        if self.frames_since_flush >= 300 {
            let _ = self.flush();
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        self.recorder
            .snapshot(self.last_frame + 1)
            .save(&self.path)?;
        self.frames_since_flush = 0;
        Ok(())
    }

    pub fn finish(&mut self, last_frame_exclusive: u64) -> Result<PathBuf> {
        let end = last_frame_exclusive.max(self.last_frame + 1);
        self.recorder.snapshot(end).save(&self.path)?;
        Ok(self.path.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_hold(m: &mut InputMovie, frame: u64, keys: Vec<u16>, hold: u32) {
        m.events.push(InputEvent { frame, keys, hold });
    }

    #[test]
    fn text_roundtrip() {
        let mut m = InputMovie::new("game.smf");
        m.auto_skip_cutscenes = true;
        push_hold(&mut m, 0, vec![0x0400], 12);
        push_hold(&mut m, 30, vec![0x0400, 0x4000], 3);
        let text = m.to_text();
        let parsed = InputMovie::parse(&text).unwrap();
        assert_eq!(parsed.keys_at(0), vec![0x0400]);
        assert_eq!(parsed.keys_at(30), vec![0x0400, 0x4000]);
        assert!(parsed.keys_at(33).is_empty());
    }

    #[test]
    fn recorder_snapshot() {
        let mut rec = MovieRecorder::new("g");
        for f in 0..20 {
            rec.record_frame(f, &[0x4000]);
        }
        let snap = rec.snapshot(20);
        assert_eq!(snap.events.len(), 1);
        assert_eq!(snap.events[0].hold, 20);
    }
}
