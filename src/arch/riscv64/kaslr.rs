use super::memory::KERNEL_BASE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidImage,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    pub kernel_base: u64,
    pub offset: u64,
}

pub fn select(_seed: Option<u64>, image_size: u64) -> Result<Layout, Error> {
    if image_size == 0 {
        return Err(Error::InvalidImage);
    }
    Ok(Layout {
        kernel_base: KERNEL_BASE,
        offset: 0,
    })
}
