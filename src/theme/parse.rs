use crate::theme::Theme;
use crate::theme::directories::{Directory, DirectoryType};
use bstr::{BStr, ByteSlice};

impl Theme {
    pub(super) fn get_all_directories<'a>(
        &'a self,
        file: &'a [u8],
    ) -> impl Iterator<Item = Directory<'a>> + 'a {
        let mut iterator = sections(file);

        std::iter::from_fn(move || {
            let mut name = "";
            let mut size = None;
            let mut max_size = None;
            let mut min_size = None;
            let mut threshold = None;
            let mut scale = None;
            // let mut context = None;
            let mut dtype = DirectoryType::default();

            #[allow(clippy::while_let_on_iterator)]
            while let Some(event) = iterator.next() {
                match event {
                    DirectorySection::Property(key, value) => {
                        if name.is_empty() || name == "Icon Theme" {
                            continue;
                        }

                        match key {
                            b"Size" => size = btoi::btoi(value).ok(),
                            b"Scale" => scale = btoi::btoi(value).ok(),
                            // "Context" => context = Some(value),
                            b"Type" => dtype = DirectoryType::from(value),
                            b"MaxSize" => max_size = btoi::btoi(value).ok(),
                            b"MinSize" => min_size = btoi::btoi(value).ok(),
                            b"Threshold" => threshold = btoi::btoi(value).ok(),
                            _ => (),
                        }
                    }

                    DirectorySection::Section(new_name) => {
                        name = std::str::from_utf8(new_name).unwrap_or("");
                        size = None;
                        max_size = None;
                        min_size = None;
                        threshold = None;
                        scale = None;
                        dtype = DirectoryType::default();
                    }

                    DirectorySection::EndSection => {
                        if name.is_empty() || name == "Icon Theme" {
                            continue;
                        }

                        let size = size.take()?;

                        return Some(Directory {
                            name,
                            size,
                            scale: scale.unwrap_or(1),
                            // context,
                            type_: dtype,
                            maxsize: max_size.unwrap_or(size),
                            minsize: min_size.unwrap_or(size),
                            threshold: threshold.unwrap_or(2),
                        });
                    }
                }
            }

            None
        })
    }

    pub fn inherits<'a>(&self, file: &'a [u8]) -> impl Iterator<Item = &'a [u8]> {
        icon_theme_section(file)
            .find(|&(key, _)| key == b"Inherits")
            .into_iter()
            .flat_map(|(_, parents)| {
                BStr::new(parents)
                    .split(|&char| char == b',')
                    // Filtering out 'hicolor' since we are going to fallback there anyway
                    .filter(|parent| parent != &b"hicolor")
            })
    }
}

#[derive(Debug)]
enum DirectorySection<'a> {
    Property(&'a [u8], &'a [u8]),
    EndSection,
    Section(&'a [u8]),
}

fn sections(file: &[u8]) -> impl Iterator<Item = DirectorySection<'_>> {
    let mut finished = false;
    let mut table_found = false;
    let mut section: &[u8] = b"";
    let mut prev = 0;
    let mut line_indices = memchr::memchr_iter(b'\n', file);

    std::iter::from_fn(move || {
        if finished {
            return None;
        }

        if !section.is_empty() {
            let new_section = section;
            section = b"";
            return Some(DirectorySection::Section(new_section));
        }

        loop {
            let line_pos = match line_indices.next() {
                Some(pos) => pos,
                None => {
                    let value = if !finished {
                        Some(DirectorySection::EndSection)
                    } else {
                        None
                    };
                    finished = true;
                    return value;
                }
            };

            let line = BStr::new(&file[prev..line_pos]).trim_ascii();
            prev = line_pos + 1;

            if line.is_empty() {
                continue;
            }

            if line[0] == b'[' {
                section = &line[1..line.len() - 1];
                if table_found {
                    return Some(DirectorySection::EndSection);
                } else {
                    table_found = true;
                    return Some(DirectorySection::Section(section));
                }
            }

            if let Some((key, value)) = memchr::memchr(b'=', line).map(|pos| unsafe {
                // Position was already validated by memchr.
                line.split_at_unchecked(pos)
            }) {
                return Some(DirectorySection::Property(key, &value[1..]));
            }
        }
    })
}

fn icon_theme_section(file: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> + '_ {
    let mut found_table = false;
    let mut prev = 0;
    let mut line_indices = memchr::memchr_iter(b'\n', file);

    std::iter::from_fn(move || {
        loop {
            let line_pos = line_indices.next()?;
            let line = BStr::new(&file[prev..line_pos]).trim_ascii();
            prev = line_pos + 1;

            if line.is_empty() {
                continue;
            }

            if line[0] == b'[' {
                if found_table {
                    return None;
                } else {
                    let section = &line[1..line.len() - 1];
                    found_table = section == b"Icon Theme";
                }
            }

            if let Some((key, value)) = memchr::memchr(b'=', line).map(|pos| unsafe {
                // Position was already validated by memchr.
                line.split_at_unchecked(pos)
            }) {
                return Some((key, &value[1..]));
            }
        }
    })
}

#[cfg(test)]

mod test {
    const ADWAITA_INDEX: &str = "[Icon Theme]
Name=Adwaita\u{0020}
Comment=The Only One
Example=folder
Inherits=hicolor

# KDE Specific Stuff
DisplayDepth=32

# Directory list
Directories=16x16/actions,16x16/apps,16x16/categories,16x16/devices,16x16/emblems,16x16/emotes,16x16/legacy,16x16/mimetypes,16x16/places,16x16/status,16x16/ui,scalable/devices,scalable/mimetypes,scalable/places,scalable/status,scalable/actions,scalable/apps,scalable/categories,scalable/emblems,scalable/emotes,scalable/legacy,scalable/ui,symbolic-up-to-32/status,symbolic/actions,symbolic/apps,symbolic/categories,symbolic/devices,symbolic/emblems,symbolic/emotes,symbolic/mimetypes,symbolic/places,symbolic/status,symbolic/legacy,symbolic/ui,

[16x16/actions]
Context=Actions
Size=16
Type=Fixed

[16x16/apps]
Context=Applications
Size=16
Type=Fixed

[16x16/categories]
Context=Categories
Size=16
Type=Fixed

[16x16/devices]
Context=Devices
Size=16
Type=Fixed

[16x16/emblems]
Context=Emblems
Size=16
Type=Fixed

[16x16/emotes]
Context=Emotes
Size=16
Type=Fixed

[16x16/legacy]
Context=Legacy
Size=16
Type=Fixed

[16x16/mimetypes]
Context=MimeTypes
Size=16
Type=Fixed

[16x16/places]
Context=Places
Size=16
Type=Fixed

[16x16/status]
Context=Status
Size=16
Type=Fixed

[16x16/ui]
Context=UI
Size=16
Type=Fixed

[scalable/devices]
Context=Devices
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/mimetypes]
Context=MimeTypes
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/places]
Context=Places
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/status]
Context=Status
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/actions]
Context=Actions
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/apps]
Context=Applications
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/categories]
Context=Categories
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/emblems]
Context=Emblems
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/emotes]
Context=Emotes
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/legacy]
Context=Legacy
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[scalable/ui]
Context=UI
Size=128
MinSize=8
MaxSize=512
Type=Scalable

[symbolic-up-to-32/status]
Context=Status
Size=16
MinSize=16
MaxSize=32
Type=Scalable

[symbolic/actions]
Context=Actions
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/apps]
Context=Applications
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/categories]
Context=Categories
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/devices]
Context=Devices
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/emblems]
Context=Emblems
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/emotes]
Context=Emotes
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/mimetypes]
Context=MimeTypes
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/places]
Context=Places
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/status]
Context=Status
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/legacy]
Context=Legacy
Size=16
MinSize=8
MaxSize=512
Type=Scalable

[symbolic/ui]
Context=UI
Size=16
MinSize=8
MaxSize=512
Type=Scalable";

    #[test]
    fn icon_theme_section() {
        let mut iterator = super::icon_theme_section(ADWAITA_INDEX.as_bytes());

        let (key, value) = iterator.next().unwrap();
        assert_eq!(key, b"Name");
        assert_eq!(value, b"Adwaita");
        let (key, value) = iterator.next().unwrap();
        assert_eq!(key, b"Comment");
        assert_eq!(value, b"The Only One");
        let (key, value) = iterator.next().unwrap();
        assert_eq!(key, b"Example");
        assert_eq!(value, b"folder");
        let (key, value) = iterator.next().unwrap();
        assert_eq!(key, b"Inherits");
        assert_eq!(value, b"hicolor");
        let (key, value) = iterator.next().unwrap();
        assert_eq!(key, b"DisplayDepth");
        assert_eq!(value, b"32");
        let (key, value) = iterator.next().unwrap();
        assert_eq!(key, b"Directories");
        assert_eq!(value, b"16x16/actions,16x16/apps,16x16/categories,16x16/devices,16x16/emblems,16x16/emotes,16x16/legacy,16x16/mimetypes,16x16/places,16x16/status,16x16/ui,scalable/devices,scalable/mimetypes,scalable/places,scalable/status,scalable/actions,scalable/apps,scalable/categories,scalable/emblems,scalable/emotes,scalable/legacy,scalable/ui,symbolic-up-to-32/status,symbolic/actions,symbolic/apps,symbolic/categories,symbolic/devices,symbolic/emblems,symbolic/emotes,symbolic/mimetypes,symbolic/places,symbolic/status,symbolic/legacy,symbolic/ui,");
        assert_eq!(iterator.next(), None);
    }

    #[test]
    #[cfg(feature = "local_tests")]
    fn should_get_theme_parents() {
        use speculoos::prelude::*;
        for theme in crate::THEMES.get("Arc").unwrap() {
            let file = crate::theme::read_ini_theme(&theme.index).ok().unwrap();
            let file = std::str::from_utf8(file.as_ref()).ok().unwrap();
            let parents = theme.inherits(file);

            assert_that!(parents).does_not_contain("hicolor");

            assert_that!(parents).is_equal_to(vec![
                "Moka",
                "Faba",
                "elementary",
                "Adwaita",
                "gnome",
            ]);
        }
    }
}
