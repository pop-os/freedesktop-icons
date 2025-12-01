use crate::theme::paths::ThemePath;
use memmap2::Mmap;
pub(crate) use paths::BASE_PATHS;
use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

mod directories;
mod parse;
mod paths;

pub static THEMES: LazyLock<BTreeMap<Vec<u8>, Vec<Theme>>> = LazyLock::new(get_all_themes);

#[inline]
pub fn read_ini_theme(path: &Path) -> std::io::Result<Mmap> {
    std::fs::File::open(path).and_then(|file| unsafe { Mmap::map(&file) })
}

#[derive(Debug)]
pub struct Theme {
    pub path: ThemePath,
    pub index: PathBuf,
}

impl Theme {
    #[inline]
    pub fn try_get_icon(
        &self,
        name: &str,
        size: u16,
        scale: u16,
        force_svg: bool,
    ) -> Option<PathBuf> {
        let file = read_ini_theme(&self.index).ok()?;
        self.try_get_icon_exact_size(file.as_ref(), name, size, scale, force_svg)
            .or_else(|| self.try_get_icon_closest_size(file.as_ref(), name, size, scale, force_svg))
    }

    #[inline]
    fn try_get_icon_exact_size(
        &self,
        file: &[u8],
        name: &str,
        size: u16,
        scale: u16,
        force_svg: bool,
    ) -> Option<PathBuf> {
        self.try_fold_icon_path(self.match_size(file, size, scale), name, force_svg)
    }

    #[inline]
    fn match_size<'a>(
        &'a self,
        file: &'a [u8],
        size: u16,
        scale: u16,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.get_all_directories(file)
            .filter(move |directory| directory.match_size(size, scale))
            .map(|dir| dir.name)
    }

    #[inline]
    fn try_get_icon_closest_size(
        &self,
        file: &[u8],
        name: &str,
        size: u16,
        scale: u16,
        force_svg: bool,
    ) -> Option<PathBuf> {
        self.try_fold_icon_path(self.closest_match_size(file, size, scale), name, force_svg)
    }

    fn try_fold_icon_path<'a>(
        &self,
        mut dir_names: impl Iterator<Item = &'a str>,
        name: &str,
        force_svg: bool,
    ) -> Option<PathBuf> {
        dir_names
            .try_fold(self.path().clone(), move |mut path, dir_name| {
                path.push(dir_name);
                if try_build_icon_path(&mut path, name, force_svg) {
                    ControlFlow::Break(path)
                } else {
                    let components = dir_name
                        .as_bytes()
                        .iter()
                        .fold(2, |n, c| n + (*c == b'/') as u32)
                        as usize;

                    for _ in 0..components {
                        path.pop();
                    }

                    ControlFlow::Continue(path)
                }
            })
            .break_value()
    }

    fn closest_match_size<'a>(
        &'a self,
        file: &'a [u8],
        size: u16,
        scale: u16,
    ) -> impl Iterator<Item = &'a str> + 'a {
        let dirs = self.get_all_directories(file);

        dirs.fold(Vec::<(&'a str, i16)>::new(), |mut sorted, directory| {
            let distance = directory.directory_size_distance(size, scale);
            if distance < i16::MAX {
                let a = distance.abs();
                let pos = sorted
                    .binary_search_by(|(_, b)| b.cmp(&a))
                    .unwrap_or_else(|pos| pos);
                sorted.insert(pos, (directory.name, a));
            }

            sorted
        })
        .into_iter()
        .map(|(name, _)| name)
    }

    fn path(&self) -> &PathBuf {
        &self.path.0
    }
}

pub(super) fn try_build_icon_path<'a>(path: &'a mut PathBuf, name: &str, force_svg: bool) -> bool {
    let mut name_buf = String::with_capacity(name.len() + 4);
    name_buf.push_str(name);
    path.push(name);
    if force_svg {
        try_build_ext(path, &mut name_buf, name, ".svg")
            || try_build_ext(path, &mut name_buf, name, ".png")
            || try_build_ext(path, &mut name_buf, name, ".xmp")
    } else {
        try_build_ext(path, &mut name_buf, name, ".png")
            || try_build_ext(path, &mut name_buf, name, ".svg")
            || try_build_ext(path, &mut name_buf, name, ".xmp")
    }
}

#[inline]
fn try_build_ext(path: &mut PathBuf, name_buf: &mut String, name: &str, ext: &'static str) -> bool {
    name_buf.truncate(name.len());
    name_buf.push_str(ext);
    path.set_file_name(&name_buf);
    path.exists()
}

// Iter through the base paths and get all theme directories
pub(super) fn get_all_themes() -> BTreeMap<Vec<u8>, Vec<Theme>> {
    let mut icon_themes = BTreeMap::<Vec<u8>, Vec<_>>::new();
    let mut found_indices = BTreeMap::new();
    let mut to_revisit = Vec::new();

    for theme_base_dir in BASE_PATHS.iter() {
        let dir_iter = match theme_base_dir.read_dir() {
            Ok(dir) => dir,
            Err(why) => {
                tracing::error!(?why, dir = ?theme_base_dir, "unable to read icon theme directory");
                continue;
            }
        };

        for entry in dir_iter.filter_map(std::io::Result::ok) {
            let name = entry.file_name();
            let fallback_index = found_indices.get(&name);
            if let Some(theme) = Theme::from_path(entry.path(), fallback_index) {
                if fallback_index.is_none() {
                    found_indices.insert(name.clone(), theme.index.clone());
                }
                icon_themes
                    .entry(name.as_bytes().to_owned())
                    .or_default()
                    .push(theme);
            } else if entry.path().is_dir() {
                to_revisit.push(entry);
            }
        }
    }

    for entry in to_revisit {
        let name = entry.file_name();
        let fallback_index = found_indices.get(&name);
        if let Some(theme) = Theme::from_path(entry.path(), fallback_index) {
            icon_themes
                .entry(name.as_bytes().to_owned())
                .or_default()
                .push(theme);
        }
    }

    icon_themes
}

impl Theme {
    pub(crate) fn from_path<P: AsRef<Path>>(path: P, index: Option<&PathBuf>) -> Option<Self> {
        let mut path = path.as_ref().to_path_buf();
        let is_dir = path.is_dir();
        path.push("index.theme");
        let local_index_exists = path.exists();
        let has_index = local_index_exists || index.is_some();

        if !has_index || !is_dir {
            return None;
        }

        index
            .cloned()
            .or_else(|| local_index_exists.then_some(path.clone()))
            .map(|index| Theme {
                path: ThemePath({
                    path.pop();
                    path
                }),
                index,
            })
    }
}

#[cfg(test)]
mod test {
    use crate::THEMES;
    use speculoos::prelude::*;
    use std::path::PathBuf;

    #[test]
    fn get_one_icon() {
        let themes = THEMES.get(&b"Adwaita"[..]).unwrap();
        println!(
            "{:?}",
            themes.iter().find_map(|t| {
                let file = super::read_ini_theme(&t.index).ok()?;
                t.try_get_icon_exact_size(file.as_ref(), "edit-delete-symbolic", 24, 1, false)
            })
        );
    }

    #[test]
    fn should_get_png_first() {
        let themes = THEMES.get(&b"hicolor"[..]).unwrap();
        let icon = themes.iter().find_map(|t| {
            let file = super::read_ini_theme(&t.index).ok()?;
            t.try_get_icon_exact_size(file.as_ref(), "blueman", 24, 1, true)
        });
        assert_that!(icon).is_some().is_equal_to(PathBuf::from(
            "/usr/share/icons/hicolor/22x22/apps/blueman.png",
        ));
    }

    #[test]
    fn should_get_svg_first() {
        let themes = THEMES.get(&b"hicolor"[..]).unwrap();
        let icon = themes.iter().find_map(|t| {
            let file = super::read_ini_theme(&t.index).ok()?;
            t.try_get_icon_exact_size(file.as_ref(), "blueman", 24, 1, false)
        });
        assert_that!(icon).is_some().is_equal_to(PathBuf::from(
            "/usr/share/icons/hicolor/22x22/apps/blueman.png",
        ));
    }
}
