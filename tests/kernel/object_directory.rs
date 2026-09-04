// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Runtime proof that global object discovery neither owns nor resurrects objects.

use crate::kernel::accounting::{ResourceDomain, ResourceLimits};
use crate::kernel::object::{Event, ObjectRef, ObjectScanCursor};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Error {
    Construction,
    MissingLiveObject,
    RetainedDeadObject,
}

pub(super) fn run() -> Result<(), Error> {
    let domain =
        ResourceDomain::try_new_root(ResourceLimits::UNLIMITED).map_err(|_| Error::Construction)?;
    let event = Event::try_new(&domain).map_err(|_| Error::Construction)?;
    let object = ObjectRef::try_new(event).map_err(|_| Error::Construction)?;
    let koid = object.koid();
    if !contains(koid) {
        return Err(Error::MissingLiveObject);
    }
    drop(object);
    if contains(koid) {
        return Err(Error::RetainedDeadObject);
    }
    drop(domain);
    Ok(())
}

fn contains(target: crate::kernel::object::Koid) -> bool {
    let mut cursor = Some(ObjectScanCursor::start());
    while let Some(position) = cursor {
        let page = crate::kernel::object::scan(position);
        if page.entries().any(|snapshot| snapshot.koid == target) {
            return true;
        }
        cursor = page.next();
    }
    false
}
