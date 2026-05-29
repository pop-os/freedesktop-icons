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
//! ```rust
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
//! ```rust
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
//! ```rust
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
use memmap2::Mmap;
use theme::BASE_PATHS;

use crate::cache::{CACHE, CacheEntry};
use crate::theme::{THEMES, Theme, try_build_icon_path};
use std::ffi::OsStr;
use std::hash::{Hash, Hasher};
use std::io::BufRead;
use std::ops::ControlFlow;
use std::path::PathBuf;

mod cache;
mod theme;
mod walk_dir;

/// Return the list of installed themes on the system
///
/// ## Example
/// ```rust,no_run
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
            let file = std::fs::File::open(index)
                .and_then(|file| unsafe { Mmap::map(&file) })
                .ok()?;
            let mut reader = std::io::Cursor::new(file.as_ref());

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
    extra_paths: &'a [PathBuf],
}

/// Build an icon lookup for the given icon name.
///
/// ## Example
/// ```rust
/// # fn main() {
/// use cosmic_freedesktop_icons::lookup;
///
/// let icon = lookup("firefox").find();
/// # }
pub fn lookup(name: &str) -> LookupBuilder<'_> {
    LookupBuilder::new(name)
}

impl<'a> LookupBuilder<'a> {
    /// Restrict the lookup to the given icon size.
    ///
    /// ## Example
    /// ```rust
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .with_size(48)
    ///     .find();
    /// # }
    #[inline]
    pub fn with_size(mut self, size: u16) -> Self {
        self.size = size;
        self
    }

    /// Restrict the lookup to the given scale.
    ///
    /// ## Example
    /// ```rust
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .with_scale(2)
    ///     .find();
    /// # }
    #[inline]
    pub fn with_scale(mut self, scale: u16) -> Self {
        self.scale = scale;
        self
    }

    /// Add the given theme to the current lookup :
    /// ## Example
    /// ```rust
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .with_theme("Papirus")
    ///     .find();
    /// # }
    #[inline]
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
    /// ```rust
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .with_scale(2)
    ///     .with_cache()
    ///     .find();
    /// # }
    #[inline]
    pub fn with_cache(mut self) -> Self {
        self.cache = true;
        self
    }

    /// By default [`find`] will prioritize Png over Svg icon.
    /// Use this if you need to prioritize Svg icons. This could be useful
    /// if you need a modifiable icon, to match a user theme for instance.
    ///
    /// ## Example
    /// ```rust
    /// # fn main() {
    /// use cosmic_freedesktop_icons::lookup;
    ///
    /// let icon = lookup("firefox")
    ///     .force_svg()
    ///     .find();
    /// # }
    #[inline]
    pub fn force_svg(mut self) -> Self {
        self.force_svg = true;
        self
    }

    /// Search additional directories for the icon as flat paths (no theme hierarchy).
    /// These paths are searched before the theme chain.
    #[inline]
    pub fn with_extra_paths<'b: 'a>(mut self, paths: &'b [PathBuf]) -> Self {
        self.extra_paths = paths;
        self
    }

    /// Execute the current lookup
    /// if no icon is found in the current theme fallback to
    /// `/usr/share/icons/hicolor` theme and then to `/usr/share/pixmaps`.
    #[inline]
    pub fn find(self) -> Option<PathBuf> {
        if self.name.is_empty() {
            return None;
        }

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
            extra_paths: &[],
        }
    }

    // Recursively lookup for icon in the given theme and its parents
    fn lookup_in_theme(&self) -> Option<PathBuf> {
        // Consult an app's private extra_paths before the cache; results are never cached.
        if let Some(path) = self.lookup_in_extra_paths() {
            return Some(path);
        }

        // If cache is activated, attempt to get the icon there first
        // If the icon was previously search but not found, we return
        // `None` early, otherwise, attempt to perform a lookup
        if self.cache {
            match self.cache_lookup(self.theme) {
                CacheEntry::Found(icon) => return Some(icon),
                CacheEntry::NotFound(last_check) if last_check.elapsed().as_secs() < 5 => {
                    return None;
                }
                _ => (),
            }
        }

        // Records theme paths that have already been searched.
        let searched_themes = &mut Vec::new();
        // Record themes whose inherits have been searched.
        let search_inherits = &mut Vec::new();

        // Then lookup in the given theme
        THEMES
            .get(self.theme.as_bytes())
            .or_else(|| THEMES.get("hicolor".as_bytes()))
            .and_then(|icon_themes| {
                let icon = icon_themes
                    .iter()
                    // Search the active icon themes
                    .find_map(|theme| self.search_theme(searched_themes, theme))
                    // Search the inherits of those icon themes.
                    .or_else(|| {
                        icon_themes.iter().find_map(|t| {
                            self.search_theme_inherits(search_inherits, searched_themes, t)
                        })
                    })
                    // Search the cosmic icon theme
                    .or_else(|| self.search_inherited_theme(searched_themes, "Cosmic".as_bytes()))
                    // Search the hicolor icon theme if it was not previously searched
                    .or_else(|| self.search_inherited_theme(searched_themes, "hicolor".as_bytes()))
                    // GNOME applications may rely on the gnome theme
                    .or_else(|| self.search_inherited_theme(searched_themes, "gnome".as_bytes()))
                    // Ubuntu applications may require Yaru
                    .or_else(|| self.search_inherited_theme(searched_themes, "Yaru".as_bytes()))
                    .or_else(|| {
                        let extensions = if self.force_svg {
                            [".svg", ".png", ".xpm"]
                        } else {
                            [".png", ".svg", ".xpm"]
                        };

                        let mut name_buf = String::new();

                        extensions
                            .into_iter()
                            .try_for_each(|ext| {
                                BASE_PATHS.iter().try_for_each(|theme_base_dir| {
                                    let mut path = theme_base_dir.clone();
                                    if try_build_icon_path(&mut path, &mut name_buf, self.name, ext)
                                    {
                                        return ControlFlow::Break(path);
                                    }
                                    name_buf.clear();
                                    ControlFlow::Continue(())
                                })
                            })
                            .break_value()
                    });

                if self.cache {
                    self.store(self.theme, icon)
                } else {
                    icon
                }
            })
    }

    /// Size-aware walk of the app-private `extra_paths`, returning the candidate
    /// closest to the requested size. Results are never cached.
    fn lookup_in_extra_paths(&self) -> Option<PathBuf> {
        if self.extra_paths.is_empty() {
            return None;
        }

        // Extension preference: lower rank wins ties between equal-sized candidates.
        let ext_rank = |ext: &str| -> Option<u8> {
            let order: [&str; 3] = if self.force_svg {
                ["svg", "png", "xpm"]
            } else {
                ["png", "svg", "xpm"]
            };
            order.iter().position(|e| *e == ext).map(|p| p as u8)
        };

        let target = u32::from(self.size).max(1) * u32::from(self.scale).max(1);

        // Sort key (smaller wins): (size_class, tiebreak, ext_rank).
        let mut best: Option<((u8, i64, u8), PathBuf)> = None;

        for file_path in walk_dir::Iter::new(self.extra_paths.iter().cloned()) {
            let Some(file_name) = file_path.file_stem().and_then(OsStr::to_str) else {
                continue;
            };
            if file_name != self.name {
                continue;
            }

            let Some(ext) = file_path.extension().and_then(OsStr::to_str) else {
                continue;
            };
            let Some(ext_rank) = ext_rank(ext) else {
                continue;
            };

            // Parse size only from components below the matching root, so a numeric
            // ancestor (e.g. the UID in /run/user/1000) is never read as a size.
            let relative = self
                .extra_paths
                .iter()
                .filter_map(|root| root.canonicalize().ok())
                .find_map(|root| {
                    file_path
                        .strip_prefix(&root)
                        .ok()
                        .map(std::path::Path::to_path_buf)
                });
            let parsed = match relative.as_deref() {
                Some(rel) => parse_size_from_path(rel, ext),
                None => None,
            };
            let (size_class, tiebreak) = match parsed {
                None => (0u8, 0i64),                       // scalable / size-agnostic: ideal
                Some(size) if size == target => (0, 0),    // exact
                Some(size) if size > target => (1, i64::from(size)), // downscale: smallest >= target
                Some(size) => (2, -i64::from(size)),       // upscale: largest available
            };

            let key = (size_class, tiebreak, ext_rank);
            match &best {
                Some((best_key, _)) if *best_key <= key => {}
                _ => best = Some((key, file_path)),
            }
        }

        best.map(|(_, path)| path)
    }

    #[inline]
    pub fn cache_clear(&mut self) {
        CACHE.clear();
    }

    #[inline]
    pub fn cache_reset_none(&mut self) {
        CACHE.reset_none();
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

    /// Search a theme by its path for a matching icon if not already searched.
    fn search_theme(&self, searched_themes: &mut Vec<u64>, theme: &Theme) -> Option<PathBuf> {
        // Store hash of the theme.
        let theme_hash = {
            let mut hasher = std::hash::DefaultHasher::new();
            theme.path.0.hash(&mut hasher);
            hasher.finish()
        };

        if let Err(pos) = searched_themes.binary_search(&theme_hash) {
            searched_themes.insert(pos, theme_hash);
            return theme.try_get_icon(self.name, self.size, self.scale, self.force_svg);
        }

        None
    }

    // Search the inherits of a theme if not already searched.
    fn search_theme_inherits(
        &self,
        search_inherits: &mut Vec<u64>,
        searched_themes: &mut Vec<u64>,
        theme: &Theme,
    ) -> Option<PathBuf> {
        // Store hash of the theme.
        let theme_hash = {
            let mut hasher = std::hash::DefaultHasher::new();
            theme.path.0.hash(&mut hasher);
            hasher.finish()
        };

        if let Err(pos) = search_inherits.binary_search(&theme_hash) {
            search_inherits.insert(pos, theme_hash);
            let Ok(file) = theme::read_ini_theme(&theme.index) else {
                return None;
            };

            // Search all inherited themes that we haven't already searched
            return theme
                .inherits(file.as_ref())
                .into_iter()
                .find_map(|parent| self.search_inherited_theme(searched_themes, parent));
        }

        None
    }

    /// Search the inherits of a theme by its name if not already searched.
    fn search_inherited_theme(
        &self,
        searched_themes: &mut Vec<u64>,
        theme: &[u8],
    ) -> Option<PathBuf> {
        THEMES
            .get(theme)?
            .iter()
            .find_map(|t| self.search_theme(searched_themes, t))
    }
}

/// Infer the pixel size from a root-relative path: an `NxN` or bare `N` directory
/// component yields that size; `scalable`/`svg` yield `None` (size-agnostic).
fn parse_size_from_path(path: &std::path::Path, ext: &str) -> Option<u32> {
    if ext.eq_ignore_ascii_case("svg") {
        return None;
    }

    let parts: Vec<&str> = path
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    // Scan directories leaf-first (skip the file name) so the nearest size wins.
    for part in parts.iter().rev().skip(1) {
        if part.eq_ignore_ascii_case("scalable") {
            return None;
        }

        // `48x48` -> 48
        if let Some((w, h)) = part.split_once(['x', 'X'])
            && let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>())
            && w == h
            && w != 0
        {
            return Some(w);
        }

        // bare `48`
        if let Ok(size) = part.parse::<u32>()
            && size != 0
        {
            return Some(size);
        }
    }

    None
}

// WARNING: these test are highly dependent on your installed icon-themes.
// If you want to run them, make sure you have 'Papirus' and 'Arc' icon-themes installed.
#[cfg(test)]
mod test {
    use crate::{CACHE, CacheEntry, lookup};
    use speculoos::prelude::*;
    use std::path::PathBuf;

    #[test]
    fn hicolor_firefox_24_png() {
        let firefox = lookup("firefox").find();

        asserting!("Firefox contains only a 16x16 and 32x32 icon, so 16x16 should be returned")
            .that(&firefox)
            .is_some()
            .is_equal_to(PathBuf::from(
                "/usr/share/icons/hicolor/16x16/apps/firefox.png",
            ));
    }

    #[test]
    fn hicolor_firefox_48_png() {
        let firefox = lookup("firefox").with_size(48).find();

        asserting!("Firefox has a 48x48 icon, so that should be returned")
            .that(&firefox)
            .is_some()
            .is_equal_to(PathBuf::from(
                "/usr/share/icons/hicolor/48x48/apps/firefox.png",
            ));
    }

    #[test]
    fn hicolor_firefox_svg_fallback_to_png() {
        let firefox = lookup("firefox").force_svg().find();

        asserting!("Lookup with no parameters should return an existing icon")
            .that(&firefox)
            .is_some()
            .is_equal_to(PathBuf::from(
                "/usr/share/icons/hicolor/16x16/apps/firefox.png",
            ));
    }

    #[test]
    fn cosmic_weather_storm_symbolic() {
        let firefox = lookup("weather-storm-symbolic").find();

        asserting!("Is the cosmic icon theme installed?")
            .that(&firefox)
            .is_some()
            .is_equal_to(PathBuf::from(
                "/usr/share/icons/Cosmic/scalable/status/weather-storm-symbolic.svg",
            ));
    }

    #[test]
    fn cosmic_weather_storm_symbolic_force_svg() {
        let firefox = lookup("weather-storm-symbolic").force_svg().find();

        asserting!("Is the cosmic icon theme installed?")
            .that(&firefox)
            .is_some()
            .is_equal_to(PathBuf::from(
                "/usr/share/icons/Cosmic/scalable/status/weather-storm-symbolic.svg",
            ));
    }

    #[test]
    fn flatpak_slack() {
        let home = std::env::home_dir().unwrap();

        assert_eq!(
            lookup("com.slack.Slack").find(),
            Some(home.join(
                ".local/share/flatpak/exports/share/icons/hicolor/scalable/apps/com.slack.Slack.svg"
            )),
            "Is the Slack flatpak installed locally?"
        );
    }

    #[test]
    fn vscode_pixmap() {
        assert_eq!(
            lookup("vscode").find(),
            Some(PathBuf::from("/usr/share/pixmaps/vscode.png")),
            "Is VS Code installed locally on the host?"
        );
    }

    #[test]
    fn libreoffice_startcenter() {
        assert_eq!(
            lookup("libreoffice-startcenter").find(),
            Some(PathBuf::from(
                "/usr/share/icons/hicolor/24x24/apps/libreoffice-startcenter.png"
            )),
            "Is libreoffice installed locally on the host?"
        );
    }

    #[test]
    fn gnome_advanced_network() {
        assert_eq!(
            lookup("preferences-system-network").find(),
            Some(PathBuf::from(
                "/usr/share/icons/gnome/24x24/categories/preferences-system-network.png"
            )),
            "Is the gnome icon theme installed?"
        );
    }

    #[test]
    fn ubuntu_additional_drivers() {
        assert_eq!(
            lookup("jockey").find(),
            Some(PathBuf::from("/usr/share/icons/Yaru/24x24/apps/jockey.png")),
            "Is the gnome icon theme installed?"
        );
    }

    #[test]
    #[cfg(feature = "local_tests")]
    fn theme_lookup() {
        let firefox = lookup("firefox").with_theme("Papirus").find();

        asserting!("Lookup with no parameters should return an existing icon")
            .that(&firefox)
            .is_some()
            .is_equal_to(PathBuf::from(
                "/usr/share/icons/Papirus/24x24/apps/firefox.svg",
            ));
    }

    #[test]
    #[cfg(feature = "local_tests")]
    fn should_fallback_to_parent_theme() {
        let icon = lookup("video-single-display-symbolic")
            .with_theme("Arc")
            .find();

        asserting!("Lookup for an icon in the Arc theme should find the icon in its parent")
            .that(&icon)
            .is_some()
            .is_equal_to(PathBuf::from(
                "/usr/share/icons/Adwaita/symbolic/devices/video-single-display-symbolic.svg",
            ));
    }

    #[test]
    #[cfg(feature = "local_tests")]
    fn should_fallback_to_pixmaps_utlimately() {
        let archlinux_logo = lookup("archlinux-logo")
            .with_size(16)
            .with_scale(1)
            .with_theme("Papirus")
            .find();

        asserting!("When lookup fail in theme, icon should be found in '/usr/share/pixmaps'")
            .that(&archlinux_logo)
            .is_some()
            .is_equal_to(PathBuf::from("/usr/share/pixmaps/archlinux-logo.png"));
    }

    #[test]
    fn should_not_attempt_to_lookup_a_not_found_cached_icon() {
        let not_found = lookup("not-found").with_cache().find();

        assert_that!(not_found).is_none();

        let expected_cache_result = CACHE.get("hicolor", 24, 1, "not-found");

        assert!(
            matches!(expected_cache_result, CacheEntry::NotFound(..)),
            "When lookup fails a first time, subsequent attempts should fail from cache"
        );
    }

    // --- extra_paths resolver tests ----------------------------------------

    /// RAII scratch directory of placeholder files; cleans up on drop.
    struct TempTree {
        root: PathBuf,
    }

    impl TempTree {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let pid = std::process::id();
            let root = std::env::temp_dir().join(format!("fdi_extra_paths_{tag}_{pid}_{n}"));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("create temp tree root");
            Self { root }
        }

        /// Create a placeholder file at `rel`; returns its canonicalized path.
        fn touch(&self, rel: &str) -> PathBuf {
            let path = self.root.join(rel);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create parent dirs");
            }
            std::fs::write(&path, [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a])
                .expect("write placeholder file");
            path.canonicalize().unwrap_or(path)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn extra_paths_flat_layout_resolves() {
        let tree = TempTree::new("flat");
        let expected = tree.touch("my-private-icon.png");
        let extra = [tree.root.clone()];

        let found = lookup("my-private-icon")
            .with_extra_paths(&extra)
            .find();

        asserting!("a flat extra_paths layout should still resolve")
            .that(&found)
            .is_some()
            .is_equal_to(expected);
    }

    #[test]
    fn extra_paths_hicolor_tree_resolves() {
        // The Dropbox case: an app ships a private hicolor hierarchy.
        let tree = TempTree::new("hicolor");
        let expected = tree.touch("hicolor/48x48/apps/dropbox.png");
        let extra = [tree.root.clone()];

        let found = lookup("dropbox")
            .with_size(48)
            .with_extra_paths(&extra)
            .find();

        asserting!("a private hicolor tree under extra_paths should resolve")
            .that(&found)
            .is_some()
            .is_equal_to(expected);
    }

    #[test]
    fn extra_paths_size_aware_picks_requested_size() {
        // Both 16x16 and 48x48 present: the requested size must be picked.
        let tree = TempTree::new("sizeaware");
        let _small = tree.touch("hicolor/16x16/apps/sizey.png");
        let expected = tree.touch("hicolor/48x48/apps/sizey.png");
        let extra = [tree.root.clone()];

        let found = lookup("sizey")
            .with_size(48)
            .with_extra_paths(&extra)
            .find();

        asserting!("extra_paths walk must be size-aware and pick 48x48")
            .that(&found)
            .is_some()
            .is_equal_to(expected);

        // And asking for 16 must pick the 16x16 asset.
        let small_expected = tree.root.join("hicolor/16x16/apps/sizey.png");
        let small_expected = small_expected.canonicalize().unwrap_or(small_expected);
        let found_small = lookup("sizey")
            .with_size(16)
            .with_extra_paths(&extra)
            .find();

        asserting!("extra_paths walk must be size-aware and pick 16x16")
            .that(&found_small)
            .is_some()
            .is_equal_to(small_expected);
    }

    #[test]
    fn extra_paths_resolves_despite_negative_cache() {
        // A prior negative-cache miss for this name must not shadow the extra_paths walk.
        let tree = TempTree::new("cached");
        let expected = tree.touch("cached-only-icon.png");
        let extra = [tree.root.clone()];

        // Seed a negative cache entry for the bare name (no theme will have it).
        let miss = lookup("cached-only-icon").with_cache().find();
        assert_that!(miss).is_none();
        assert!(
            matches!(
                CACHE.get("hicolor", 24, 1, "cached-only-icon"),
                CacheEntry::NotFound(..)
            ),
            "precondition: the bare-name lookup should have cached a miss"
        );

        // Now the same name via extra_paths (with cache still enabled) must resolve.
        let found = lookup("cached-only-icon")
            .with_cache()
            .with_extra_paths(&extra)
            .find();

        asserting!("extra_paths must be consulted before the (negative) cache")
            .that(&found)
            .is_some()
            .is_equal_to(expected);
    }

    #[test]
    fn extra_paths_numeric_root_does_not_misparse_as_size() {
        // A numeric root basename (e.g. /run/user/<uid>) must not be read as a size.
        let tree = TempTree::new("numeric");
        let numeric_root = tree.root.join("1000");
        std::fs::create_dir_all(&numeric_root).unwrap();

        let png = numeric_root.join("flatey.png");
        let svg = numeric_root.join("flatey.svg");
        for p in [&png, &svg] {
            std::fs::write(p, [0x89, b'P', b'N', b'G']).unwrap();
        }
        let png = png.canonicalize().unwrap_or(png);
        let extra = [numeric_root.clone()];

        // Default png>svg priority must hold: the "1000" root must not be a size.
        let found = lookup("flatey")
            .with_size(32)
            .with_extra_paths(&extra)
            .find();

        asserting!("numeric root basename must not be read as icon size")
            .that(&found)
            .is_some()
            .is_equal_to(png);
    }
}
