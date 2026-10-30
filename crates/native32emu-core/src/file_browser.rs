// File browser for the FHUI front-end menu.
//
// The menu shell (FHUI.smf) builds its game list at runtime by asking the host
// to enumerate the game files and category directories on disk. It uses these
// GetUrl2 host calls:
//
//   GetFileNum+<dir>   -> number of games in <dir>
//   GetFirstFile+<dir> -> reset iterator, return the first game name
//   GetNextFile+<dir>  -> return the next game name
//   GetDirNum+<dir>    -> number of category directories in <dir>
//   GetFirstDir+<dir>  -> reset iterator, return the first directory name
//   GetNextDir+<dir>   -> return the next directory name
//
// `<dir>` is a Native32 menu path: root-relative, '/'-separated, with directory
// names padded with trailing spaces (e.g. "/EPOP    /"). The returned names are
// the file stems without extension (e.g. "EBBLADE"); the menu later appends the
// ".dat" (thumbnail) or ".smf" (game) extension itself.
//
// File and directory iterators are independent: the Chinese menu enumerates
// both lists in the same frame.

use std::path::{Path, PathBuf};

use crate::content_loader::ContentLoader;

/// Stateful directory browser used by the front-end menu.
pub struct FileBrowser {
    /// Sorted game file stems (no extension) for the last listed directory.
    entries: Vec<String>,
    /// Iterator cursor for GetFirstFile / GetNextFile.
    cursor: usize,
    /// Sorted category directory names for the last listed directory.
    dirs: Vec<String>,
    /// Iterator cursor for GetFirstDir / GetNextDir.
    dir_cursor: usize,
}

impl Default for FileBrowser {
    fn default() -> Self {
        Self::new()
    }
}

impl FileBrowser {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            cursor: 0,
            dirs: Vec::new(),
            dir_cursor: 0,
        }
    }

    /// Rebuild the file entry list for `menu_path`, resolved relative to
    /// `base_file`. The running menu file itself is skipped so it does not
    /// appear as a game.
    fn refresh(&mut self, base_file: &Path, menu_path: &str) {
        self.entries = list_game_files(base_file, menu_path);
        self.cursor = 0;
    }

    /// Number of game files in `menu_path`.
    pub fn file_count(&mut self, base_file: &Path, menu_path: &str) -> usize {
        self.refresh(base_file, menu_path);
        self.entries.len()
    }

    /// Reset the iterator for `menu_path` and return the first game name
    /// (or an empty string if the directory has no games).
    pub fn first_file(&mut self, base_file: &Path, menu_path: &str) -> String {
        self.refresh(base_file, menu_path);
        self.current_file()
    }

    /// Advance the iterator and return the next game name (or an empty string
    /// once the directory is exhausted).
    pub fn next_file(&mut self) -> String {
        self.cursor += 1;
        self.current_file()
    }

    /// Number of category directories in `menu_path`.
    pub fn dir_count(&mut self, base_file: &Path, menu_path: &str) -> usize {
        self.refresh_dirs(base_file, menu_path);
        self.dirs.len()
    }

    /// Reset the directory iterator for `menu_path` and return the first name.
    pub fn first_dir(&mut self, base_file: &Path, menu_path: &str) -> String {
        self.refresh_dirs(base_file, menu_path);
        self.current_dir()
    }

    /// Advance the directory iterator and return the next name.
    pub fn next_dir(&mut self) -> String {
        self.dir_cursor += 1;
        self.current_dir()
    }

    fn refresh_dirs(&mut self, base_file: &Path, menu_path: &str) {
        self.dirs = list_game_dirs(base_file, menu_path);
        self.dir_cursor = 0;
    }

    fn current_file(&self) -> String {
        self.entries.get(self.cursor).cloned().unwrap_or_default()
    }

    fn current_dir(&self) -> String {
        self.dirs.get(self.dir_cursor).cloned().unwrap_or_default()
    }
}

/// Resolve a Native32 menu path to a real directory on disk.
///
/// An empty path means the menu root (the directory containing `base_file`).
pub fn resolve_menu_dir(base_file: &Path, menu_path: &str) -> Option<PathBuf> {
    let components: Vec<&str> = menu_path
        .split('/')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if components.is_empty() {
        return base_file.parent().map(|p| p.to_path_buf());
    }

    let relative = components.join("/");
    ContentLoader::find_content_file(base_file, &relative).filter(|p| p.is_dir())
}

/// List the game file stems (".smf", no extension) inside `menu_path`,
/// sorted case-insensitively for stable navigation order.
///
/// The running menu file is listed like any other game: the Chinese FHUI
/// menu's root view shows it as the single root entry and requests its
/// thumbnail, so excluding it would empty the root list and clear the
/// thumbnail pane ("No File!" / "No Thumbnail" on screen).
fn list_game_files(base_file: &Path, menu_path: &str) -> Vec<String> {
    let dir = match resolve_menu_dir(base_file, menu_path) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let is_smf = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("smf"));
            if !is_smf {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.push(stem.to_string());
            }
        }
    }

    names.sort_by_key(|a| a.to_lowercase());
    names
}

/// List the category directories inside `menu_path` that contain at least one
/// game (`.smf`) somewhere below them. Directories holding only non-game
/// payload (e.g. the platform's `NA32SSL` asset tree) are hidden.
fn list_game_dirs(base_file: &Path, menu_path: &str) -> Vec<String> {
    let dir = match resolve_menu_dir(base_file, menu_path) {
        Some(d) => d,
        None => return Vec::new(),
    };

    let mut names = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            // Do not follow symlinks: a link loop would make the recursive
            // game scan unbounded.
            if !file_type.is_dir() {
                continue;
            }
            if !contains_game(&entry.path()) {
                continue;
            }
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
    }

    names.sort_by_key(|a| a.to_lowercase());
    names
}

/// Whether `dir` contains a `.smf` game directly or in a nested directory.
fn contains_game(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if contains_game(&entry.path()) {
                return true;
            }
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("smf"))
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_games() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        // Menu file at the root.
        std::fs::write(dir.path().join("FHUI.smf"), b"fake").unwrap();
        // A category directory with two games (+ companion .dat files).
        let cat = dir.path().join("EACT");
        std::fs::create_dir(&cat).unwrap();
        std::fs::write(cat.join("EBBLADE.smf"), b"fake").unwrap();
        std::fs::write(cat.join("EBBLADE.dat"), b"fake").unwrap();
        std::fs::write(cat.join("AGUNFIRE.smf"), b"fake").unwrap();
        std::fs::write(cat.join("notes.txt"), b"fake").unwrap();
        // An asset tree with no games must not be listed as a category.
        let assets = dir.path().join("NA32SSL").join("ENGLISH");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("SELECT.SSL"), b"fake").unwrap();
        // A category whose games live one level down.
        let nested = dir.path().join("FUN").join("EDU");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("ADD.smf"), b"fake").unwrap();
        dir
    }

    #[test]
    fn counts_only_smf_files() {
        let dir = setup_games();
        let base = dir.path().join("FHUI.smf");
        let mut fb = FileBrowser::new();
        assert_eq!(fb.file_count(&base, "/EACT    /"), 2);
    }

    #[test]
    fn iterates_files_sorted() {
        let dir = setup_games();
        let base = dir.path().join("FHUI.smf");
        let mut fb = FileBrowser::new();
        // Sorted case-insensitively: AGUNFIRE before EBBLADE.
        assert_eq!(fb.first_file(&base, "/EACT    /"), "AGUNFIRE");
        assert_eq!(fb.next_file(), "EBBLADE");
        // Exhausted -> empty string.
        assert_eq!(fb.next_file(), "");
    }

    #[test]
    fn lists_the_running_menu_file() {
        let dir = setup_games();
        let base = dir.path().join("FHUI.smf");
        let mut fb = FileBrowser::new();
        // The menu file is the root's single entry: the Chinese FHUI menu
        // builds its root selection ("1. fhui" + thumbnail) from it.
        assert_eq!(fb.file_count(&base, "/"), 1);
        assert_eq!(fb.first_file(&base, "/"), "FHUI");
    }

    #[test]
    fn lists_only_directories_containing_games() {
        let dir = setup_games();
        let base = dir.path().join("FHUI.smf");
        let mut fb = FileBrowser::new();
        // EACT and FUN contain games; NA32SSL holds only SSL assets.
        assert_eq!(fb.dir_count(&base, "/"), 2);
        assert_eq!(fb.first_dir(&base, "/"), "EACT");
        assert_eq!(fb.next_dir(), "FUN");
        assert_eq!(fb.next_dir(), "");
    }

    #[test]
    fn file_and_dir_iterators_are_independent() {
        let dir = setup_games();
        let base = dir.path().join("FHUI.smf");
        let mut fb = FileBrowser::new();
        assert_eq!(fb.first_dir(&base, "/"), "EACT");
        // Re-reading the file list must not rewind the directory iterator.
        assert_eq!(fb.file_count(&base, "/EACT"), 2);
        assert_eq!(fb.next_dir(), "FUN");
        assert_eq!(fb.next_file(), "EBBLADE");
    }

    #[test]
    fn missing_directory_yields_nothing() {
        let dir = setup_games();
        let base = dir.path().join("FHUI.smf");
        let mut fb = FileBrowser::new();
        assert_eq!(fb.file_count(&base, "/NOPE    /"), 0);
        assert_eq!(fb.first_file(&base, "/NOPE    /"), "");
        assert_eq!(fb.dir_count(&base, "/NOPE    /"), 0);
        assert_eq!(fb.first_dir(&base, "/NOPE    /"), "");
    }

    #[test]
    fn resolve_empty_path_is_root() {
        let dir = setup_games();
        let base = dir.path().join("FHUI.smf");
        let resolved = resolve_menu_dir(&base, "/").unwrap();
        assert_eq!(resolved, dir.path());
    }
}
