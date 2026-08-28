// Content loader: handles typed content loading and switching.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ContentLoadKind {
    #[default]
    Replace,
    LaunchChild,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContentLoader {
    pub pending_content: Option<String>,
    #[serde(default)]
    pub pending_kind: ContentLoadKind,
}

impl Default for ContentLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentLoader {
    pub fn new() -> Self {
        Self {
            pending_content: None,
            pending_kind: ContentLoadKind::Replace,
        }
    }

    /// Mark content for loading at the end of the current tick.
    pub fn queue_load(&mut self, filename: &str) {
        self.queue(filename, ContentLoadKind::Replace);
    }

    /// Mark a child application for loading at the end of the current tick.
    pub fn queue_child(&mut self, filename: &str) {
        self.queue(filename, ContentLoadKind::LaunchChild);
    }

    fn queue(&mut self, filename: &str, kind: ContentLoadKind) {
        // Parse the filename: split by '/', trim each component, remove empty segments.
        // Native32 games often pad directory names with trailing spaces (e.g. "BBLADE  ").
        let parts: Vec<&str> = filename
            .split('/')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if parts.is_empty() {
            log::warn!("Empty content filename");
            return;
        }
        self.pending_content = Some(parts.join("/"));
        self.pending_kind = kind;
    }

    /// Check if there's pending content to load.
    pub fn has_pending(&self) -> bool {
        self.pending_content.is_some()
    }

    /// Take the pending content filename.
    pub fn take_pending(&mut self) -> Option<(String, ContentLoadKind)> {
        let content = self.pending_content.take()?;
        let kind = std::mem::take(&mut self.pending_kind);
        Some((content, kind))
    }

    /// Find the content file by searching up the directory tree.
    /// Handles case-insensitive matching and trailing spaces in path components.
    pub fn find_content_file(current_game_path: &Path, filename: &str) -> Option<PathBuf> {
        let relative_path = Path::new(filename);

        // Search from the current game file's directory upward
        let mut search_dir = current_game_path.parent();

        while let Some(dir) = search_dir {
            // Try exact match first
            let pattern = dir.join(relative_path);
            if pattern.exists() {
                return Some(pattern);
            }

            // Try case-insensitive match for each path component
            if let Some(resolved) = Self::fuzzy_resolve(dir, relative_path) {
                return Some(resolved);
            }

            search_dir = dir.parent();
        }

        log::warn!("Failed to find content file: {}", filename);
        None
    }

    /// Try to resolve a relative path under a base directory with fuzzy matching
    /// (case-insensitive + trailing space trimming for each component).
    fn fuzzy_resolve(base: &Path, relative: &Path) -> Option<PathBuf> {
        let mut current = base.to_path_buf();

        for component in relative.components() {
            let target = component.as_os_str().to_string_lossy().to_lowercase();
            let target = target.trim();

            let mut found = false;
            if let Ok(entries) = std::fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy().to_lowercase();
                    if name_str.trim() == target {
                        current = entry.path();
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                return None;
            }
        }

        if current.exists() {
            Some(current)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_loader_has_no_pending() {
        let mut loader = ContentLoader::new();
        assert!(!loader.has_pending());
        assert!(loader.take_pending().is_none());
    }

    #[test]
    fn test_default_trait() {
        let loader = ContentLoader::default();
        assert!(!loader.has_pending());
    }

    #[test]
    fn test_queue_load_simple_filename() {
        let mut loader = ContentLoader::new();
        loader.queue_load("level1.ssl");
        assert!(loader.has_pending());
        assert_eq!(
            loader.take_pending(),
            Some(("level1.ssl".to_string(), ContentLoadKind::Replace))
        );
    }

    #[test]
    fn test_queue_load_trims_trailing_spaces() {
        let mut loader = ContentLoader::new();
        loader.queue_load("BBLADE  /level1.ssl");
        assert_eq!(
            loader.take_pending(),
            Some(("BBLADE/level1.ssl".to_string(), ContentLoadKind::Replace))
        );
    }

    #[test]
    fn test_queue_load_trims_leading_spaces() {
        let mut loader = ContentLoader::new();
        loader.queue_load("  BBLADE/level1.ssl");
        assert_eq!(
            loader.take_pending(),
            Some(("BBLADE/level1.ssl".to_string(), ContentLoadKind::Replace))
        );
    }

    #[test]
    fn test_queue_load_removes_empty_segments() {
        let mut loader = ContentLoader::new();
        loader.queue_load("///BBLADE///level1.ssl///");
        assert_eq!(
            loader.take_pending(),
            Some(("BBLADE/level1.ssl".to_string(), ContentLoadKind::Replace))
        );
    }

    #[test]
    fn test_queue_load_empty_string() {
        let mut loader = ContentLoader::new();
        loader.queue_load("");
        assert!(!loader.has_pending(), "empty filename should not be queued");
    }

    #[test]
    fn test_queue_load_only_spaces() {
        let mut loader = ContentLoader::new();
        loader.queue_load("   /   /   ");
        assert!(
            !loader.has_pending(),
            "all-whitespace path should not be queued"
        );
    }

    #[test]
    fn test_queue_load_only_slashes() {
        let mut loader = ContentLoader::new();
        loader.queue_load("///");
        assert!(!loader.has_pending(), "all-slash path should not be queued");
    }

    #[test]
    fn test_queue_load_complex_path() {
        let mut loader = ContentLoader::new();
        loader.queue_load("  Game Dir  /  Sub Dir  /  file.ssl  ");
        assert_eq!(
            loader.take_pending(),
            Some((
                "Game Dir/Sub Dir/file.ssl".to_string(),
                ContentLoadKind::Replace
            ))
        );
    }

    #[test]
    fn test_take_pending_consumes() {
        let mut loader = ContentLoader::new();
        loader.queue_load("test.ssl");
        assert!(loader.take_pending().is_some());
        assert!(!loader.has_pending());
        assert!(loader.take_pending().is_none());
    }

    #[test]
    fn test_queue_load_overwrites_previous() {
        let mut loader = ContentLoader::new();
        loader.queue_load("first.ssl");
        loader.queue_load("second.ssl");
        assert_eq!(
            loader.take_pending(),
            Some(("second.ssl".to_string(), ContentLoadKind::Replace))
        );
    }

    #[test]
    fn test_queue_child_marks_launch_kind() {
        let mut loader = ContentLoader::new();
        loader.queue_child("child.smf");
        assert_eq!(
            loader.take_pending(),
            Some(("child.smf".to_string(), ContentLoadKind::LaunchChild))
        );
    }

    #[test]
    fn test_replacement_overwrites_child_kind() {
        let mut loader = ContentLoader::new();
        loader.queue_child("child.smf");
        loader.queue_load("scene.ssl");
        assert_eq!(
            loader.take_pending(),
            Some(("scene.ssl".to_string(), ContentLoadKind::Replace))
        );
    }

    #[test]
    fn test_old_serialized_loader_defaults_to_replacement() {
        let mut loader: ContentLoader =
            serde_json::from_str(r#"{"pending_content":"scene.ssl"}"#).unwrap();
        assert_eq!(
            loader.take_pending(),
            Some(("scene.ssl".to_string(), ContentLoadKind::Replace))
        );
    }

    #[test]
    fn test_find_content_file_nonexistent() {
        let result = ContentLoader::find_content_file(
            std::path::Path::new("/nonexistent/game.ssl"),
            "missing.ssl",
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_find_content_file_finds_sibling() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("game.ssl");
        let target = dir.path().join("level2.ssl");
        std::fs::write(&game_path, b"fake").unwrap();
        std::fs::write(&target, b"fake").unwrap();

        let result = ContentLoader::find_content_file(&game_path, "level2.ssl");
        assert_eq!(result, Some(target));
    }

    #[test]
    fn test_find_content_file_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("game.ssl");
        let target = dir.path().join("Level2.SSL");
        std::fs::write(&game_path, b"fake").unwrap();
        std::fs::write(&target, b"fake").unwrap();

        let result = ContentLoader::find_content_file(&game_path, "level2.ssl");
        // On Windows, tempfile may normalize case, so just check it was found
        assert!(
            result.is_some(),
            "case-insensitive lookup should find the file"
        );
        assert!(result.unwrap().exists());
    }

    #[test]
    fn test_find_content_file_in_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("game.ssl");
        let sub = dir.path().join("levels");
        std::fs::create_dir(&sub).unwrap();
        let target = sub.join("level2.ssl");
        std::fs::write(&game_path, b"fake").unwrap();
        std::fs::write(&target, b"fake").unwrap();

        let result = ContentLoader::find_content_file(&game_path, "levels/level2.ssl");
        assert_eq!(result, Some(target));
    }

    #[test]
    fn test_find_content_file_fuzzy_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let game_path = dir.path().join("game.ssl");
        let sub = dir.path().join("BBLADE");
        std::fs::create_dir(&sub).unwrap();
        let target = sub.join("level2.ssl");
        std::fs::write(&game_path, b"fake").unwrap();
        std::fs::write(&target, b"fake").unwrap();

        // Query with trailing spaces (common in Native32 games)
        let result = ContentLoader::find_content_file(&game_path, "BBLADE  /level2.ssl");
        assert_eq!(result, Some(target));
    }
}
