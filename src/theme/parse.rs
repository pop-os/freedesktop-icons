use crate::theme::directories::{Directory, DirectoryType};
use crate::theme::Theme;

fn icon_theme_section(file: &str) -> impl Iterator<Item = (&str, &str)> + '_ {
    ini_core::Parser::new(file)
        .skip_while(|item| *item != ini_core::Item::Section("Icon Theme"))
        .take_while(|item| *item != ini_core::Item::SectionEnd)
        .filter_map(|item| {
            if let ini_core::Item::Property(key, value) = item {
                Some((key, value?))
            } else {
                None
            }
        })
}

fn directory_section<'a: 'b, 'b>(
    file: &'a str,
    section: &'b str,
) -> impl Iterator<Item = (&'a str, &'a str)> + 'b {
    ini_core::Parser::new(file)
        .skip_while(|item| *item != ini_core::Item::Section(section))
        .take_while(|item| *item != ini_core::Item::SectionEnd)
        .filter_map(|item| {
            if let ini_core::Item::Property(key, value) = item {
                Some((key, value?))
            } else {
                None
            }
        })
}

impl Theme {
    pub(super) fn get_all_directories<'a>(
        &'a self,
        file: &'a str,
    ) -> impl Iterator<Item = Directory<'a>> + 'a {
        self.directories(file)
            .into_iter()
            .filter_map(|name| self.get_directory(file, name))
            .chain(
                self.scaled_directories(file)
                    .into_iter()
                    .filter_map(|name| self.get_directory(file, name)),
            )
    }

    fn scaled_directories<'a>(&self, file: &'a str) -> Vec<&'a str> {
        icon_theme_section(file)
            .find(|&(key, _)| key == "ScaledDirectories")
            .map(|(_, dirs)| dirs.split(',').collect())
            .unwrap_or_default()
    }

    pub fn inherits<'a>(&self, file: &'a str) -> Vec<&'a str> {
        icon_theme_section(file)
            .find(|&(key, _)| key == "Inherits")
            .map(|(_, parents)| {
                parents
                    .split(',')
                    // Filtering out 'hicolor' since we are going to fallback there anyway
                    .filter(|parent| parent != &"hicolor")
                    .collect()
            })
            .unwrap_or_default()
    }

    fn directories<'a>(&self, file: &'a str) -> Vec<&'a str> {
        icon_theme_section(file)
            .find(|&(key, _)| key == "Directories")
            .map(|(_, dirs)| dirs.split(',').collect())
            .unwrap_or_default()
    }

    fn get_directory<'a>(&'a self, file: &'a str, name: &'a str) -> Option<Directory<'a>> {
        let mut size = None;
        let mut max_size = None;
        let mut min_size = None;
        let mut threshold = None;
        let mut scale = None;
        // let mut context = None;
        let mut dtype = DirectoryType::default();

        for (key, value) in directory_section(file, name) {
            match key {
                "Size" => size = str::parse(value).ok(),
                "Scale" => scale = str::parse(value).ok(),
                // "Context" => context = Some(value),
                "Type" => dtype = DirectoryType::from(value),
                "MaxSize" => max_size = str::parse(value).ok(),
                "MinSize" => min_size = str::parse(value).ok(),
                "Threshold" => threshold = str::parse(value).ok(),
                _ => (),
            }
        }

        let size = size?;

        Some(Directory {
            name,
            size,
            scale: scale.unwrap_or(1),
            // context,
            type_: dtype,
            maxsize: max_size.unwrap_or(size),
            minsize: min_size.unwrap_or(size),
            threshold: threshold.unwrap_or(2),
        })
    }
}

#[cfg(test)]
mod test {
    use crate::THEMES;
    use speculoos::prelude::*;

    #[test]
    fn should_get_theme_parents() {
        for theme in THEMES.get("Arc").unwrap() {
            let file = crate::theme::read_ini_theme(&theme.index).unwrap_or_default();
            let parents = theme.inherits(&file);

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
