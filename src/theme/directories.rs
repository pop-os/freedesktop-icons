#[derive(Debug)]
pub struct Directory<'a> {
    pub name: &'a str,
    pub size: i16,
    pub scale: i16,
    pub type_: DirectoryType,
    pub maxsize: i16,
    pub minsize: i16,
    pub threshold: i16,
}

impl Directory<'_> {
    pub fn match_size(&self, size: u16, scale: u16) -> bool {
        let scale = scale as i16;
        let size = size as i16;

        if self.scale != scale {
            false
        } else {
            match self.type_ {
                DirectoryType::Fixed => self.size == size,
                DirectoryType::Scalable => self.minsize <= size && size <= self.maxsize,
                DirectoryType::Threshold => {
                    self.size - self.threshold <= size && size <= self.size + self.threshold
                }
            }
        }
    }

    pub fn directory_size_distance(&self, size: u16, scale: u16) -> i16 {
        let scale = scale as i16;
        let scaled_requested_size = size as i16 * scale;

        match self.type_ {
            DirectoryType::Fixed => self.size * self.scale - scaled_requested_size,
            DirectoryType::Scalable => {
                let min_scaled_size = self.minsize * self.scale;
                if scaled_requested_size < min_scaled_size {
                    min_scaled_size - scaled_requested_size
                } else {
                    let max_scaled_size = self.maxsize * self.scale;
                    if scaled_requested_size < max_scaled_size {
                        scaled_requested_size - max_scaled_size
                    } else {
                        0
                    }
                }
            }
            DirectoryType::Threshold => {
                if scaled_requested_size < (self.size - self.threshold) * scale {
                    self.minsize * self.scale - scaled_requested_size
                } else if scaled_requested_size > (self.size + self.threshold) * scale {
                    scaled_requested_size - self.maxsize * self.scale
                } else {
                    0
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum DirectoryType {
    Fixed,
    Scalable,
    Threshold,
}

impl Default for DirectoryType {
    fn default() -> Self {
        Self::Threshold
    }
}

impl From<&str> for DirectoryType {
    fn from(value: &str) -> Self {
        match value.as_bytes()[0] {
            b'F' => DirectoryType::Fixed,
            b'S' => DirectoryType::Scalable,
            _ => DirectoryType::Threshold,
        }
    }
}
