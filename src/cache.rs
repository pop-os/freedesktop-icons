use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, RwLock};
use std::time::Instant;

pub(crate) static CACHE: LazyLock<Cache> = LazyLock::new(Cache::default);
type Theme = Box<str>;
type Icon = Box<str>;
type SizedMap = BTreeMap<(u16, u16), CacheEntry>;
type IconMap = BTreeMap<Icon, SizedMap>;
type ThemeMap = BTreeMap<Theme, IconMap>;

#[derive(Default)]
pub(crate) struct Cache(RwLock<ThemeMap>);

#[derive(Debug, Clone, PartialEq)]
pub enum CacheEntry {
    // We already looked for this and nothing was found, indicates we should not try to perform a lookup.
    NotFound(Instant),
    // We have this entry.
    Found(PathBuf),
    // We don't know this entry yet, indicate we should perform a lookup.
    Unknown,
}

impl Cache {
    pub fn clear(&self) {
        self.0.write().unwrap().clear();
    }

    pub fn insert<P: AsRef<Path>>(
        &self,
        theme: &str,
        size: u16,
        scale: u16,
        icon_name: &str,
        icon_path: &Option<P>,
    ) {
        let mut inner = self.0.write().unwrap();
        let entry = icon_path
            .as_ref()
            .map(|path| CacheEntry::Found(path.as_ref().to_path_buf()))
            .unwrap_or(CacheEntry::NotFound(Instant::now()));

        inner
            .entry(theme.into())
            .or_insert_with(IconMap::default)
            .entry(icon_name.into())
            .or_insert_with(BTreeMap::default)
            .insert((size, scale), entry);
    }

    pub fn get(&self, theme: &str, size: u16, scale: u16, icon_name: &str) -> CacheEntry {
        let inner = self.0.read().unwrap();

        inner
            .get(theme)
            .and_then(|icon_map| icon_map.get(icon_name))
            .and_then(|icon_map| icon_map.get(&(size, scale)).cloned())
            .unwrap_or(CacheEntry::Unknown)
    }

    pub fn reset_none(&self) {
        let mut inner = self.0.write().unwrap();
        for (_theme_name, theme) in inner.iter_mut() {
            for (_, cached_icons) in theme.iter_mut() {
                for (_, cached_icon) in cached_icons.iter_mut() {
                    if matches!(cached_icon, CacheEntry::NotFound(_)) {
                        *cached_icon = CacheEntry::Unknown;
                    }
                }
            }
        }
    }
}
