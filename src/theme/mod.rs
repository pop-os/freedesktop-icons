use crate::theme::directories::DirectoryType;
use crate::theme::paths::ThemePath;
use memmap2::Mmap;
pub(crate) use paths::BASE_PATHS;
use std::cmp::Ordering;
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
        prefer_svg: bool,
    ) -> Option<PathBuf> {
        let file = read_ini_theme(&self.index).ok()?;
        self.try_get_icon_closest_size(file.as_ref(), name, size, scale, prefer_svg)
    }

    #[inline]
    fn try_get_icon_closest_size(
        &self,
        file: &[u8],
        name: &str,
        size: u16,
        scale: u16,
        prefer_svg: bool,
    ) -> Option<PathBuf> {
        self.try_fold_icon_path(
            self.closest_match_size(file, size, scale, prefer_svg),
            name,
            prefer_svg,
        )
    }

    fn try_fold_icon_path<'a>(
        &self,
        dir_names: Vec<(&'a str, i16, bool)>,
        name: &str,
        prefer_svg: bool,
    ) -> Option<PathBuf> {
        let extensions = if prefer_svg {
            [".svg", ".png", ".xpm"]
        } else {
            [".png", ".svg", ".xpm"]
        };

        extensions.into_iter().find_map(|ext| {
            dir_names
                .iter()
                .try_fold(
                    (self.path().clone(), String::new()),
                    move |(mut path, mut name_buf), (dir_name, _, _)| {
                        path.push(dir_name);
                        if try_build_icon_path(&mut path, &mut name_buf, name, ext) {
                            ControlFlow::Break(path)
                        } else {
                            name_buf.clear();
                            let components = dir_name
                                .as_bytes()
                                .iter()
                                .fold(2, |n, c| n + (*c == b'/') as u32)
                                as usize;

                            for _ in 0..components {
                                path.pop();
                            }

                            ControlFlow::Continue((path, name_buf))
                        }
                    },
                )
                .break_value()
        })
    }

    fn closest_match_size<'a>(
        &'a self,
        file: &'a [u8],
        size: u16,
        scale: u16,
        prefer_svg: bool,
    ) -> Vec<(&'a str, i16, bool)> {
        let mut unsorted = self.get_all_directories(file).fold(
            Vec::<(&'a str, i16, bool)>::new(),
            |mut unsorted, directory| {
                let is_scalable = matches!(directory.type_, DirectoryType::Scalable);
                let distance = directory.directory_size_distance(size as i16, scale as i16);
                unsorted.push((directory.name, distance.abs(), is_scalable));
                unsorted
            },
        );

        unsorted.sort_by(|a, b| {
            let ordering = if prefer_svg {
                b.2.cmp(&a.2)
            } else {
                a.2.cmp(&b.2)
            };
            match ordering {
                Ordering::Equal => a.1.cmp(&b.1),
                _ => ordering,
            }
        });

        unsorted
    }

    fn path(&self) -> &PathBuf {
        &self.path.0
    }
}

pub(super) fn try_build_icon_path<'a>(
    path: &'a mut PathBuf,
    name_buf: &'a mut String,
    name: &str,
    extension: &'static str,
) -> bool {
    name_buf.push_str(name);
    path.push(name);
    try_build_ext(path, name_buf, name, extension)
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
                t.try_get_icon_closest_size(file.as_ref(), "edit-delete-symbolic", 24, 1, false)
            })
        );
    }

    #[test]
    fn should_get_png_first() {
        let themes = THEMES.get(&b"hicolor"[..]).unwrap();
        let icon = themes.iter().find_map(|t| {
            let file = super::read_ini_theme(&t.index).ok()?;
            t.try_get_icon_closest_size(file.as_ref(), "blueman", 22, 1, false)
        });
        assert_that!(icon).is_some().is_equal_to(PathBuf::from(
            "/usr/share/icons/hicolor/22x22/apps/blueman.png",
        ));
    }

    #[test]
    fn should_get_png_first_92() {
        let themes = THEMES.get(&b"hicolor"[..]).unwrap();
        let icon = themes.iter().find_map(|t| {
            let file = super::read_ini_theme(&t.index).ok()?;
            t.try_get_icon_closest_size(file.as_ref(), "blueman", 92, 1, false)
        });
        assert_that!(icon).is_some().is_equal_to(PathBuf::from(
            "/usr/share/icons/hicolor/96x96/apps/blueman.png",
        ));
    }

    #[test]
    fn should_get_svg_first() {
        let themes = THEMES.get(&b"hicolor"[..]).unwrap();
        let icon = themes.iter().find_map(|t| {
            let file = super::read_ini_theme(&t.index).ok()?;
            t.try_get_icon_closest_size(file.as_ref(), "blueman", 24, 1, true)
        });
        assert_that!(icon).is_some().is_equal_to(PathBuf::from(
            "/usr/share/icons/hicolor/scalable/apps/blueman.svg",
        ));
    }

    #[test]
    fn should_get_svg_first_96() {
        let themes = THEMES.get(&b"hicolor"[..]).unwrap();
        let icon = themes.iter().find_map(|t| {
            let file = super::read_ini_theme(&t.index).ok()?;
            t.try_get_icon_closest_size(file.as_ref(), "blueman", 96, 1, true)
        });
        assert_that!(icon).is_some().is_equal_to(PathBuf::from(
            "/usr/share/icons/hicolor/scalable/apps/blueman.svg",
        ));
    }
}
