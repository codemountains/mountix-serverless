pub trait TagBaseMapper {
    fn new(key: u32) -> Self;
    fn to_tag(&self) -> Result<String, ()>;
}

pub struct TagMapper {
    key: u32,
}

impl TagBaseMapper for TagMapper {
    fn new(key: u32) -> Self {
        Self { key }
    }

    fn to_tag(&self) -> Result<String, ()> {
        let prefix = "Tag_".to_string();
        match self.key {
            1 => Ok(format!("{}{}", prefix, "百名山".to_string())),
            _ => Err(()),
        }
    }
}
