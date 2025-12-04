use std::path::{Path, PathBuf};

use crate::cache::{CACHE, CacheEntry};

const PWA_THEME_DARK: &str = "pwa-hicolor-dark";
const PWA_THEME_LIGHT: &str = "pwa-hicolor-light";

/// Return a direct hicolor icon path for Chromium/CRX PWAs when the themed
/// lookup fails. Uses a small cache keyed by name/size/scale to avoid repeated
/// filesystem scans.
pub fn lookup_chromium_pwa_icon(
    name: &str,
    requested_px: u16,
    prefer_dark: bool,
    use_cache: bool,
) -> Option<PathBuf> {
    lookup_chromium_pwa_icon_with_paths(
        name,
        requested_px,
        prefer_dark,
        use_cache,
        &default_hicolor_paths(),
    )
}

/// Variant that accepts explicit search paths. Intended for tests.
pub fn lookup_chromium_pwa_icon_with_paths(
    name: &str,
    requested_px: u16,
    prefer_dark: bool,
    use_cache: bool,
    base_dirs: &[PathBuf],
) -> Option<PathBuf> {
    let Some(crx) = extract_crx_id(name) else {
        return None;
    };

    let theme_key = if prefer_dark {
        PWA_THEME_DARK
    } else {
        PWA_THEME_LIGHT
    };
    if use_cache {
        match CACHE.get(theme_key, requested_px, 1, name) {
            CacheEntry::Found(path) => return Some(path),
            CacheEntry::NotFound(_) => return None,
            CacheEntry::Unknown => {}
        }
    }

    let sizes = ordered_sizes(requested_px);

    // 1) Fast path: exact name match by requested size preference.
    for base in base_dirs {
        for size in &sizes {
            let candidate = base.join(size).join("apps").join(format!("{name}.png"));
            if candidate.exists() {
                if use_cache {
                    CACHE.insert(theme_key, requested_px, 1, name, &Some(&candidate));
                }
                return Some(candidate);
            }
        }
    }

    // 2) CRX-aware search: pick the best matching asset by theme/maskable bias.
    let mut best: Option<(PathBuf, i32)> = None;
    for base in base_dirs {
        for size in &sizes {
            let apps_dir = base.join(size).join("apps");
            if !apps_dir.exists() {
                continue;
            }

            let Ok(read) = std::fs::read_dir(&apps_dir) else {
                continue;
            };

            for ent in read.flatten() {
                let path = ent.path();
                let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };

                if !file_name.contains(&crx) {
                    continue;
                }

                let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
                    continue;
                };
                let ext_score = if ext.eq_ignore_ascii_case("png") {
                    2
                } else if ext.eq_ignore_ascii_case("svg") {
                    1
                } else {
                    0
                };
                if ext_score == 0 {
                    continue;
                }

                let lower = file_name.to_ascii_lowercase();
                let is_dark_tag = lower.contains("dark");
                let is_light_tag = lower.contains("light");
                let is_maskable = lower.contains("maskable");
                let is_monochrome = lower.contains("monochrome");

                let theme_score = if prefer_dark {
                    if is_dark_tag {
                        2
                    } else if is_light_tag {
                        0
                    } else {
                        1
                    }
                } else if is_light_tag {
                    2
                } else if is_dark_tag {
                    0
                } else {
                    1
                };
                let mask_score = if is_maskable {
                    2
                } else if is_monochrome {
                    0
                } else {
                    1
                };

                let score = theme_score * 100 + mask_score * 10 + ext_score;
                if best.as_ref().map_or(true, |(_, s)| score > *s) {
                    best = Some((path.clone(), score));
                }
            }
        }
    }

    if let Some((picked, _)) = best {
        if use_cache {
            CACHE.insert(theme_key, requested_px, 1, name, &Some(&picked));
        }
        return Some(picked);
    }

    if use_cache {
        CACHE.insert(theme_key, requested_px, 1, name, &None::<&Path>);
    }
    None
}

fn ordered_sizes(requested_px: u16) -> Vec<&'static str> {
    const SIZES: &[&str] = &[
        "512x512", "256x256", "192x192", "128x128", "96x96", "64x64", "48x48", "32x32", "24x24",
        "22x22", "16x16",
    ];

    let mut sizes = SIZES.to_vec();
    if let Some(pos) = sizes
        .iter()
        .position(|s| s.starts_with(&format!("{}x{}", requested_px, requested_px)))
    {
        let preferred = sizes.remove(pos);
        sizes.insert(0, preferred);
    }
    sizes
}

fn default_hicolor_paths() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        dirs.push(home.join(".local/share/icons/hicolor"));
        dirs.push(home.join(".local/share/flatpak/exports/share/icons/hicolor"));

        let var_app = home.join(".var/app");
        if var_app.exists() {
            if let Ok(read) = std::fs::read_dir(&var_app) {
                for ent in read.flatten() {
                    let p = ent.path().join("data/icons/hicolor");
                    if p.exists() {
                        dirs.push(p);
                    }
                }
            }
        }
    }
    dirs.push(PathBuf::from("/usr/share/icons/hicolor"));
    dirs.push(PathBuf::from("/usr/local/share/icons/hicolor"));
    dirs.push(PathBuf::from(
        "/var/lib/flatpak/exports/share/icons/hicolor",
    ));
    dirs.push(PathBuf::from(
        "/usr/share/flatpak/exports/share/icons/hicolor",
    ));
    dirs
}

fn is_crx_id(candidate: &str) -> bool {
    candidate.len() == 32 && candidate.chars().all(|c| matches!(c, 'a'..='p'))
}

fn is_crx_bytes(bytes: &[u8]) -> bool {
    bytes.len() == 32 && bytes.iter().all(|b| matches!(b, b'a'..=b'p'))
}

/// Extract a Chromium CRX id (32 lowercase hex-like chars in the range a-p).
pub fn extract_crx_id(value: &str) -> Option<String> {
    if let Some(rest) = value.strip_prefix("chrome-") {
        if let Some(first) = rest.split(&['-', '_'][..]).next() {
            if is_crx_id(first) {
                return Some(first.to_string());
            }
        }
    }
    if let Some(rest) = value.strip_prefix("crx_") {
        let token = rest
            .split(|c: char| !c.is_ascii_lowercase())
            .next()
            .unwrap_or(rest);
        if is_crx_id(token) {
            return Some(token.to_string());
        }
    }
    if is_crx_id(value) {
        return Some(value.to_string());
    }

    for window in value.as_bytes().windows(32) {
        if is_crx_bytes(window) {
            // SAFETY: `is_crx_bytes` guarantees the window is ASCII.
            let slice = std::str::from_utf8(window).expect("ASCII window");
            return Some(slice.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn picks_exact_name_first() {
        let tmp = tempdir().unwrap();
        let base = tmp.path().to_path_buf();
        let apps_dir = base.join("64x64").join("apps");
        fs::create_dir_all(&apps_dir).unwrap();
        let icon_path = apps_dir.join("example.png");
        fs::write(&icon_path, []).unwrap();

        let found = lookup_chromium_pwa_icon_with_paths(
            "chrome-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-default",
            64,
            false,
            true,
            &[base],
        );
        assert_eq!(found.as_deref(), Some(icon_path.as_path()));
    }

    #[test]
    fn scores_crx_icons_by_theme_and_maskable() {
        let tmp = tempdir().unwrap();
        let base = tmp.path().to_path_buf();
        let apps_dir = base.join("64x64").join("apps");
        fs::create_dir_all(&apps_dir).unwrap();

        // Light theme preference should pick the light maskable PNG over dark SVG.
        let crx = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let dark = apps_dir.join(format!("chrome-{crx}-dark.svg"));
        let light_maskable = apps_dir.join(format!("chrome-{crx}-maskable-light.png"));
        fs::write(&dark, []).unwrap();
        fs::write(&light_maskable, []).unwrap();

        let found_light = lookup_chromium_pwa_icon_with_paths(
            &format!("crx_{crx}"),
            64,
            false,
            false,
            &[base.clone()],
        )
        .unwrap();
        assert_eq!(found_light, light_maskable);

        // Dark preference should pick the dark SVG when no dark PNG exists.
        let found_dark =
            lookup_chromium_pwa_icon_with_paths(&format!("crx_{crx}"), 64, true, false, &[base])
                .unwrap();
        assert_eq!(found_dark, dark);
    }
}
