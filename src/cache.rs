use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Instant;

pub(crate) static CACHE: LazyLock<Cache> = LazyLock::new(Cache::default);
type IconMap = BTreeMap<(String, u16, u16), CacheEntry>;
type ThemeMap = BTreeMap<String, IconMap>;

#[derive(Default)]
pub(crate) struct Cache(Mutex<ThemeMap>);

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
        self.0.lock().unwrap().clear();
    }

    pub fn insert<P: AsRef<Path>>(
        &self,
        theme: &str,
        size: u16,
        scale: u16,
        icon_name: &str,
        icon_path: &Option<P>,
    ) {
        let mut theme_map = self.0.lock().unwrap();
        let entry = icon_path
            .as_ref()
            .map(|path| CacheEntry::Found(path.as_ref().to_path_buf()))
            .unwrap_or(CacheEntry::NotFound(Instant::now()));

        match theme_map.get_mut(theme) {
            Some(icon_map) => {
                icon_map.insert((icon_name.to_string(), size, scale), entry);
            }
            None => {
                let mut icon_map = BTreeMap::new();
                icon_map.insert((icon_name.to_string(), size, scale), entry);
                theme_map.insert(theme.to_string(), icon_map);
            }
        }
    }

    pub fn get(&self, theme: &str, size: u16, scale: u16, icon_name: &str) -> CacheEntry {
        let theme_map = self.0.lock().unwrap();

        theme_map
            .get(theme)
            .map(|icon_map| icon_map.get(&(icon_name.to_string(), size, scale)))
            .and_then(|path| path.cloned())
            .unwrap_or(CacheEntry::Unknown)
    }

    pub fn reset_none(&self) {
        let mut theme_map = self.0.lock().unwrap();

        for (_theme_name, theme) in theme_map.iter_mut() {
            for (_icon_data, cached_icon) in theme.iter_mut() {
                if matches!(cached_icon, CacheEntry::NotFound(_)) {
                    *cached_icon = CacheEntry::Unknown;
                }
            }
        }
    }
}
