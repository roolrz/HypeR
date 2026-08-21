//! Exercises the application-memory capability copy boundary.

use hyper::mm::ForeignMemory;

use crate::kernel::mm::{AddressSpaceId, UserAddressSpace, copy_from_user, copy_to_user};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Copy,
    Payload,
}

struct TestAddressSpace {
    bytes: [u8; 64],
}

impl ForeignMemory for TestAddressSpace {
    type Error = Error;

    fn address_base(&self) -> u64 {
        0x20_0000
    }

    fn address_size(&self) -> u64 {
        self.bytes.len() as u64
    }

    fn page_size(&self) -> usize {
        16
    }

    fn read_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        destination: &mut [u8],
    ) -> Result<(), Self::Error> {
        let start = page_index * self.page_size() + page_offset;
        let source = self
            .bytes
            .get(start..start + destination.len())
            .ok_or(Error::Copy)?;
        destination.copy_from_slice(source);
        Ok(())
    }

    fn write_page(
        &mut self,
        page_index: usize,
        page_offset: usize,
        source: &[u8],
    ) -> Result<(), Self::Error> {
        let start = page_index * self.page_size() + page_offset;
        let destination = self
            .bytes
            .get_mut(start..start + source.len())
            .ok_or(Error::Copy)?;
        destination.copy_from_slice(source);
        Ok(())
    }
}

impl UserAddressSpace for TestAddressSpace {
    fn id(&self) -> AddressSpaceId {
        AddressSpaceId(7)
    }
}

pub(super) fn run() -> Result<(), Error> {
    let mut address_space = TestAddressSpace { bytes: [0; 64] };
    let payload = *b"checked app-memory boundary";
    copy_to_user(&mut address_space, 0x20_000d, &payload).map_err(|_| Error::Copy)?;

    let mut copied = [0; 27];
    copy_from_user(&mut address_space, 0x20_000d, &mut copied).map_err(|_| Error::Copy)?;
    if address_space.id() != AddressSpaceId(7) || copied != payload {
        return Err(Error::Payload);
    }
    Ok(())
}
