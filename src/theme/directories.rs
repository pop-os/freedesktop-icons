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
    pub fn directory_size_distance(&self, size: i16, scale: i16) -> i16 {
        match self.type_ {
            DirectoryType::Fixed => self.size * self.scale - size * scale,

            DirectoryType::Scalable => {
                let scaled_requested_size = size * scale;
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
                let scaled_requested_size = size * scale;
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

impl From<&[u8]> for DirectoryType {
    fn from(value: &[u8]) -> Self {
        match value[0] {
            b'F' => DirectoryType::Fixed,
            b'S' => DirectoryType::Scalable,
            _ => DirectoryType::Threshold,
        }
    }
}
