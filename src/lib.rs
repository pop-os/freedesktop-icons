//! # freedesktop-icons
//!
//! This crate provides a [freedesktop icon](https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html#implementation_notes) lookup implementation.
//!
//! It exposes a single lookup function to find icons based on their `name`, `theme`, `size` and `scale`.
//!
//! ## Example
//!
//! **Simple lookup:**
//!
//! The following snippet get an icon from the default 'hicolor' theme
//! with the default scale (`1`) and the default size (`24`).
//!
//! ```rust, no_run
//! # fn main() {
//! use cosmic_freedesktop_icons::lookup;
//!
//! let icon = lookup("firefox").find();
//! # }
//!```
//!
//! **Complex lookup:**
//!
//! If you have specific requirements for your lookup you can use the provided builder functions:
//!
//! ```rust, no_run
//! # fn main() {
//! use cosmic_freedesktop_icons::lookup;
//!
//! let icon = lookup("firefox")
//!     .with_size(48)
//!     .with_scale(2)
//!     .with_theme("Arc")
//!     .find();
//! # }
//!```
//! **Cache:**
//!
//! If your application is going to repeat the same icon lookups multiple times
//! you can use the internal cache to improve performance.
//!
//! ```rust, no_run
//! # fn main() {
//! use cosmic_freedesktop_icons::lookup;
//!
//! let icon = lookup("firefox")
//!     .with_size(48)
//!     .with_scale(2)
//!     .with_theme("Arc")
//!     .with_cache()
//!     .find();
//! # }
//! ```
use theme::BASE_PATHS;

use crate::cache::{CacheEntry, CACHE};
use crate::theme::{try_build_icon_path, THEMES};
use std::io::BufRead;
use std::path::PathBuf;

mod cache;
mod theme;

/// Return the list of installed themes on the system
///
/// ## Example
/// ```rust, no_run
/// # fn main() {
/// use cosmic_freedesktop_icons::list_themes;
///
/// let themes: Vec<String> = list_themes();
///
/// assert_eq!(themes, vec![
///     "Adwaita", "Arc", "Breeze Light", "HighContrast", "Papirus", "Papirus-Dark",
///     "Papirus-Light", "Breeze", "Breeze Dark", "Breeze", "ePapirus", "ePapirus-Dark", "Hicolor"
/// ])
/// # }
pub fn list_themes() -> Vec<String> {
    let mut themes = THEMES
        .values()
        .flatten()
        .map(|path| &path.index)
        .filter_map(|index| {
            let file = std::fs::File::open(index).ok()?;
            let mut reader = std::io::BufReader::new(file);

            let mut line = String::new();
            while let Ok(read) = reader.read_line(&mut line) {
                if read == 0 {
                    break;
                }

                if let Some(name) = line.strip_prefix("Name=") {
                    return Some(name.trim().to_owned());
                }

                line.clear();
            }

            None
        })
        .collect::<Vec<_>>();
    themes.dedup();
    themes
}

/// The lookup builder struct, holding all the lookup query parameters.
pub struct LookupBuilder<'a> {
    name: &'a str,
    cache: bool,
    force_svg: bool,
    scale: u16,
    size: u16,
    theme: &'a str,
}

/// Build an icon lookup for the given icon name.
///
/// ## Example
/// ```rust, no_run
/// # fn main() {
/// use cosmic_freedesktop_icons::lookup;
///
/// let icon = lookup("firefox").find();
/// # }
pub fn lookup(name: &str) -> LookupBuilder {
    LookupBuilder::new(name)
}

impl<'a> LookupBuilder<'a> {
    /// Restrict the lookup to the given icon size.
    ///
    /// ## Example
    /// ```rust, no_run
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .with_size(48)
    ///     .find();
    /// # }
    pub fn with_size(mut self, size: u16) -> Self {
        self.size = size;
        self
    }

    /// Restrict the lookup to the given scale.
    ///
    /// ## Example
    /// ```rust, no_run
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .with_scale(2)
    ///     .find();
    /// # }
    pub fn with_scale(mut self, scale: u16) -> Self {
        self.scale = scale;
        self
    }

    /// Add the given theme to the current lookup :
    /// ## Example
    /// ```rust, no_run
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .with_theme("Papirus")
    ///     .find();
    /// # }
    pub fn with_theme<'b: 'a>(mut self, theme: &'b str) -> Self {
        self.theme = theme;
        self
    }

    /// Store the result of the lookup in cache, subsequent
    /// lookup will first try to get the cached icon.
    /// This can drastically increase lookup performances for application
    /// that repeat the same lookups, an application launcher for instance.
    ///
    /// ## Example
    /// ```rust, no_run
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .with_scale(2)
    ///     .with_cache()
    ///     .find();
    /// # }
    pub fn with_cache(mut self) -> Self {
        self.cache = true;
        self
    }

    /// By default [`find`] will prioritize Png over Svg icon.
    /// Use this if you need to prioritize Svg icons. This could be useful
    /// if you need a modifiable icon, to match a user theme for instance.
    ///
    /// ## Example
    /// ```rust, no_run
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .force_svg()
    ///     .find();
    /// # }
    pub fn force_svg(mut self) -> Self {
        self.force_svg = true;
        self
    }

    /// Execute the current lookup
    /// if no icon is found in the current theme fallback to
    /// `/usr/share/icons/hicolor` theme and then to `/usr/share/pixmaps`.
    pub fn find(self) -> Option<PathBuf> {
        // Lookup for an icon in the given theme and fallback to 'hicolor' default theme
        self.lookup_in_theme()
    }

    fn new<'b: 'a>(name: &'b str) -> Self {
        Self {
            name,
            cache: false,
            force_svg: false,
            scale: 1,
            size: 24,
            theme: "hicolor",
        }
    }

    // Recursively lookup for icon in the given theme and its parents
    fn lookup_in_theme(&self) -> Option<PathBuf> {
        // If cache is activated, attempt to get the icon there first
        // If the icon was previously search but not found, we return
        // `None` early, otherwise, attempt to perform a lookup
        if self.cache {
            match self.cache_lookup(self.theme) {
                CacheEntry::Found(icon) => {
                    return Some(icon);
                }
                CacheEntry::NotFound => {
                    return None;
                }
                CacheEntry::Unknown => {}
            };
        }

        // Then lookup in the given theme
        THEMES
            .get(self.theme)
            .or_else(|| THEMES.get("hicolor"))
            .and_then(|icon_themes| {
                let icon = icon_themes
                    .iter()
                    .find_map(|theme| {
                        theme.try_get_icon(self.name, self.size, self.scale, self.force_svg)
                    })
                    .or_else(|| {
                        // Fallback to the parent themes recursively
                        let mut parents = icon_themes
                            .iter()
                            .flat_map(|t| {
                                let file = theme::read_ini_theme(&t.index);

                                t.inherits(file.as_ref())
                                    .into_iter()
                                    .map(String::from)
                                    .collect::<Vec<String>>()
                            })
                            .collect::<Vec<_>>();
                        parents.dedup();
                        parents.into_iter().find_map(|parent| {
                            THEMES.get(&parent).and_then(|parent| {
                                parent.iter().find_map(|t| {
                                    t.try_get_icon(self.name, self.size, self.scale, self.force_svg)
                                })
                            })
                        })
                    })
                    .or_else(|| {
                        THEMES.get("hicolor").and_then(|icon_themes| {
                            icon_themes.iter().find_map(|theme| {
                                theme.try_get_icon(self.name, self.size, self.scale, self.force_svg)
                            })
                        })
                    })
                    .or_else(|| {
                        for theme_base_dir in BASE_PATHS.iter() {
                            if let Some(icon) =
                                try_build_icon_path(self.name, theme_base_dir, self.force_svg)
                            {
                                return Some(icon);
                            }
                        }
                        None
                    })
                    .or_else(|| {
                        try_build_icon_path(self.name, "/usr/share/pixmaps", self.force_svg)
                    })
                    .or_else(|| {
                        let p = PathBuf::from(&self.name);
                        if let (Some(name), Some(parent)) = (p.file_stem(), p.parent()) {
                            try_build_icon_path(&name.to_string_lossy(), parent, self.force_svg)
                        } else {
                            None
                        }
                    });

                if self.cache {
                    self.store(self.theme, icon)
                } else {
                    icon
                }
            })
    }

    #[inline]
    fn cache_lookup(&self, theme: &str) -> CacheEntry {
        CACHE.get(theme, self.size, self.scale, self.name)
    }

    #[inline]
    fn store(&self, theme: &str, icon: Option<PathBuf>) -> Option<PathBuf> {
        CACHE.insert(theme, self.size, self.scale, self.name, &icon);
        icon
    }
}

#[cfg(test)]
mod test {
    use crate::{lookup, CacheEntry, CACHE};
    use speculoos::prelude::*;
    use std::{
        env,
        path::{Path, PathBuf},
        sync::LazyLock,
    };

    pub(super) static TEST_ASSETS_PATH: LazyLock<PathBuf> = LazyLock::new(|| {
        let data_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("test_assets");
        assert!(
            data_dir.exists(),
            "The `test_assets` folder should be in the package root"
        );
        data_dir
    });

    /// Override the default search path(s) with a path we control.
    ///
    /// This grants us more control over tests rather than relying on the system having the
    /// themes we need.
    pub(super) fn set_fake_icons_path() {
        env::set_var("XDG_DATA_DIRS", TEST_ASSETS_PATH.to_str().unwrap());
    }

    #[test]
    fn simple_lookup() {
        set_fake_icons_path();
        let browser = lookup("browser").find();

        let icon_path = TEST_ASSETS_PATH.join("icons/hicolor/scalable/apps/browser.svg");
        asserting!("Lookup with no parameters should return an existing icon")
            .that(&browser)
            .is_some()
            .is_equal_to(icon_path);
    }

    #[test]
    fn theme_lookup() {
        set_fake_icons_path();
        let cosmic_fake = lookup("cosmic-fake").with_theme("cosmic-base").find();

        let icon_path = TEST_ASSETS_PATH.join("icons/cosmic-base/16x16/apps/cosmic-fake.svg");
        asserting!("Lookup with no parameters should return an existing icon")
            .that(&cosmic_fake)
            .is_some()
            .is_equal_to(icon_path);
    }

    #[test]
    fn should_fallback_to_parent_theme() {
        set_fake_icons_path();
        let icon = lookup("video-single-display-symbolic")
            .with_theme("cosmic-base-dark")
            .find();

        let icon_path = TEST_ASSETS_PATH
            .join("icons/cosmic-base/scalable/devices/video-single-display-symbolic.svg");
        asserting!(
            "Lookup for an icon in the cosmic-dark theme should find the icon in its parent"
        )
        .that(&icon)
        .is_some()
        .is_equal_to(icon_path);
    }

    #[test]
    fn should_fallback_to_pixmaps_ultimately() {
        set_fake_icons_path();
        let archlinux_logo = lookup("archlinux-logo")
            .with_size(16)
            .with_scale(1)
            .with_theme("COSMIC")
            .find();

        asserting!("When lookup fail in theme, icon should be found in '/usr/share/pixmaps'")
            .that(&archlinux_logo)
            .is_some()
            .is_equal_to(PathBuf::from("/usr/share/pixmaps/archlinux-logo.png"));
    }

    #[test]
    fn compare_to_linincon_with_theme() {
        set_fake_icons_path();
        let lin_cosmic_fake = linicon::lookup_icon("cosmic-fake")
            .from_theme("cosmic-base")
            .next()
            .unwrap()
            .unwrap()
            .path;

        let cosmic_fake = lookup("cosmic-fake")
            .with_size(16)
            .with_scale(1)
            .with_theme("cosmic-base")
            .find();

        asserting!("Given the same input parameter, lookup should output be the same as linincon")
            .that(&cosmic_fake)
            .is_some()
            .is_equal_to(lin_cosmic_fake);
    }

    #[test]
    fn should_not_attempt_to_lookup_a_not_found_cached_icon() {
        let not_found = lookup("not-found").with_cache().find();

        assert_that!(not_found).is_none();

        let expected_cache_result = CACHE.get("hicolor", 24, 1, "not-found");

        asserting!("When lookup fails a first time, subsequent attempts should fail from cache")
            .that(&expected_cache_result)
            .is_equal_to(CacheEntry::NotFound);
    }
}
