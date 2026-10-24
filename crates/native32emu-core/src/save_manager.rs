// Save manager: handles persistence of save data to .ssl_sav files.

use std::path::{Path, PathBuf};

/// Build a flat, filesystem-safe save name from a content path.
fn save_file_stem(game_path: &Path) -> String {
    game_path
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .map(|part| {
            part.chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                        c
                    } else {
                        '_'
                    }
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("_")
}

pub struct SaveManager {
    save_path: PathBuf,
}

impl SaveManager {
    pub fn new(game_path: &Path) -> Self {
        Self::new_in(game_path, None)
    }

    /// Create a save manager, optionally placing save data in a frontend-owned
    /// directory (e.g. RetroArch/EmulatorJS `/data/saves`, which is persisted in
    /// the browser). The file name is derived from the full content path so
    /// scenes with identical names stay distinct.
    pub fn new_in(game_path: &Path, save_dir: Option<&Path>) -> Self {
        let save_path = match save_dir {
            Some(dir) => dir.join(format!("{}.ssl_sav", save_file_stem(game_path))),
            None => PathBuf::from(format!("{}.ssl_sav", game_path.display())),
        };
        Self { save_path }
    }

    /// Save data to the .ssl_sav file.
    pub fn save(&self, data: &str) -> bool {
        match std::fs::write(&self.save_path, data) {
            Ok(()) => {
                log::info!("Saved data to {}", self.save_path.display());
                true
            }
            Err(e) => {
                log::error!("Failed to save data: {}", e);
                false
            }
        }
    }

    /// Load data from the .ssl_sav file.
    /// Returns Some(data) if the file exists, None otherwise.
    pub fn load(&self) -> Option<String> {
        if !self.save_path.exists() {
            log::info!("Save file not found: {}", self.save_path.display());
            return None;
        }
        match std::fs::read_to_string(&self.save_path) {
            Ok(data) => {
                log::info!("Loaded save data from {}", self.save_path.display());
                Some(data)
            }
            Err(e) => {
                log::error!("Failed to load save data: {}", e);
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_path_has_ssl_sav_suffix() {
        let game_path = Path::new("/some/path/game.ssl");
        let sm = SaveManager::new(game_path);
        assert_eq!(sm.save_path, PathBuf::from("/some/path/game.ssl.ssl_sav"));
    }

    #[test]
    fn test_save_path_in_directory_is_flattened() {
        let game_path = Path::new("/NA32SSL/ENGLISH/BBLADE/BBPLAY10.SSL");
        let sm = SaveManager::new_in(game_path, Some(Path::new("/data/saves")));
        assert_eq!(
            sm.save_path,
            PathBuf::from("/data/saves/NA32SSL_ENGLISH_BBLADE_BBPLAY10.SSL.ssl_sav")
        );
    }

    #[test]
    fn test_save_roundtrip_in_directory() {
        let dir = tempfile::tempdir().unwrap();
        let saves = dir.path().join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        let game_path = dir.path().join("game.smf");
        std::fs::write(&game_path, b"fake").unwrap();

        let sm = SaveManager::new_in(&game_path, Some(&saves));
        assert!(sm.save("progress"));
        assert_eq!(sm.load(), Some("progress".to_string()));
        assert!(saves.read_dir().unwrap().next().is_some());
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("test_game.ssl");
        std::fs::write(&game_path, b"fake game data").unwrap();

        let sm = SaveManager::new(&game_path);
        let test_data = "highscore=9999;level=5";

        assert!(sm.save(test_data));
        let loaded = sm.load();
        assert_eq!(loaded, Some(test_data.to_string()));
    }

    #[test]
    fn test_load_nonexistent_returns_none() {
        let sm = SaveManager::new(Path::new("/nonexistent/path/game.ssl"));
        assert!(sm.load().is_none());
    }

    #[test]
    fn test_save_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("test_game.ssl");
        std::fs::write(&game_path, b"fake").unwrap();

        let sm = SaveManager::new(&game_path);
        assert!(sm.save("first"));
        assert!(sm.save("second"));
        assert_eq!(sm.load(), Some("second".to_string()));
    }

    #[test]
    fn test_save_returns_true_on_success() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("test_game.ssl");
        std::fs::write(&game_path, b"fake").unwrap();

        let sm = SaveManager::new(&game_path);
        assert!(sm.save("data"));
    }

    #[test]
    fn test_load_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("test_game.ssl");
        std::fs::write(&game_path, b"fake").unwrap();

        let sm = SaveManager::new(&game_path);
        sm.save("");
        assert_eq!(sm.load(), Some("".to_string()));
    }

    #[test]
    fn test_save_unicode_data() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("test_game.ssl");
        std::fs::write(&game_path, b"fake").unwrap();

        let sm = SaveManager::new(&game_path);
        let data = "name=测试玩家;score=100";
        assert!(sm.save(data));
        assert_eq!(sm.load(), Some(data.to_string()));
    }
}
