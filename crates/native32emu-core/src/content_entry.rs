// Which content a frontend may start: a game's `.smf`/`.sgm` application, or a
// `.zip` package. `.ssl` files are the scenes inside a game.

use std::path::Path;

/// Extensions a frontend may hand the core as the content to start.
pub const LAUNCHABLE_EXTENSIONS: [&str; 3] = ["smf", "sgm", "zip"];

/// `LAUNCHABLE_EXTENSIONS` in the libretro `valid_extensions` form.
pub fn launchable_extensions_field() -> String {
    LAUNCHABLE_EXTENSIONS.join("|")
}

/// Whether `path` is an SSL scene.
pub fn is_scene(path: &Path) -> bool {
    has_extension(path, "ssl")
}

/// The error shown when an SSL scene is passed as launch content.
pub fn scene_rejection(path: &Path) -> String {
    format!(
        "{} is an SSL scene inside a game, not a game entry point. Start the game from \
         its .smf/.sgm application or from its .zip package.",
        path.display()
    )
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_files_are_not_launchable() {
        assert!(is_scene(Path::new("/NA32SSL/ENGLISH/BBLADE/BBSTART.SSL")));
        assert!(!is_scene(Path::new("EACT/EBBLADE.smf")));
        assert!(!LAUNCHABLE_EXTENSIONS.contains(&"ssl"));
    }

    #[test]
    fn applications_and_packages_are_launchable() {
        for path in ["EACT/EBBLADE.smf", "Dragon.sgm", "collection.ZIP"] {
            let extension = Path::new(path).extension().unwrap().to_str().unwrap();
            assert!(
                LAUNCHABLE_EXTENSIONS
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(extension)),
                "{path} should be launchable"
            );
        }
    }

    #[test]
    fn extension_field_matches_the_allow_list() {
        assert_eq!(launchable_extensions_field(), "smf|sgm|zip");
    }

    #[test]
    fn rejection_names_the_scene_and_the_alternatives() {
        let message = scene_rejection(Path::new("/games/NA32SSL/ENGLISH/BBLADE/BBSTART.SSL"));
        assert!(message.contains("BBSTART.SSL"));
        assert!(message.contains(".smf"));
        assert!(message.contains(".zip"));
    }
}
