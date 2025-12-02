use std::path::PathBuf;
use std::sync::LazyLock;
use xdg::BaseDirectories;

pub(crate) static BASE_PATHS: LazyLock<Vec<PathBuf>> = LazyLock::new(icon_theme_base_paths);

/// Look in $HOME/.icons (for backwards compatibility), in $XDG_DATA_DIRS/icons, in $XDG_DATA_DIRS/pixmaps and in /usr/share/pixmaps (in that order).
/// Paths that are not found are filtered out.
fn icon_theme_base_paths() -> Vec<PathBuf> {
    let base_dirs = BaseDirectories::new();

    let data_dirs = base_dirs
        .get_data_dirs()
        .into_iter()
        .flat_map(|p| [p.join("icons"), p.join("pixmaps")]);

    let data_home_dirs = base_dirs
        .get_data_home()
        .into_iter()
        .flat_map(|data_home| [data_home.join("icons"), data_home.join("pixmaps")].into_iter());

    let home_dir = std::env::home_dir()
        .into_iter()
        .map(|home| home.join(".icons"));

    data_dirs
        .chain(data_home_dirs)
        .chain(home_dir)
        .filter(|p| p.exists())
        .collect()
}

#[derive(Clone, Debug)]
pub struct ThemePath(pub PathBuf);

#[cfg(test)]
mod test {
    use crate::theme::paths::icon_theme_base_paths;
    use crate::theme::{Theme, get_all_themes};
    use speculoos::prelude::*;

    #[test]
    fn should_get_all_themes() {
        let themes = get_all_themes();
        assert_that!(themes.get(&b"hicolor"[..])).is_some();
    }

    #[test]
    fn should_get_theme_paths_ordered() {
        let base_paths = icon_theme_base_paths();
        assert_that!(base_paths).is_not_empty()
    }

    #[test]
    fn should_read_theme_index() {
        let themes = get_all_themes();
        let themes: Vec<&Theme> = themes.values().flatten().collect();
        assert_that!(themes).is_not_empty();
    }
}
