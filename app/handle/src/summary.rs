// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Counts apply the same filters and visibility as the corresponding row listing.

use hyper_os::handle::ObjectKind;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

#[derive(Default)]
pub(crate) struct Summary(BTreeMap<u32, Counts>);

#[derive(Default)]
struct Counts {
    objects: BTreeSet<u64>,
    handles: usize,
}

impl Summary {
    pub(crate) fn record(&mut self, kind: ObjectKind, koid: u64, handle: bool) {
        let counts = self.0.entry(kind.as_raw()).or_default();
        counts.objects.insert(koid);
        counts.handles += usize::from(handle);
    }

    pub(crate) fn write(
        &self,
        handles: bool,
        headers: bool,
        out: &mut impl Write,
    ) -> io::Result<()> {
        if headers {
            if handles {
                writeln!(out, "KIND                       OBJECTS    HANDLES")?;
            } else {
                writeln!(out, "KIND                       OBJECTS")?;
            }
        }
        for (kind, counts) in &self.0 {
            let name = ObjectKind::KNOWN
                .iter()
                .find(|value| value.as_raw() == *kind)
                .map_or_else(
                    || format!("unknown(0x{kind:08x})"),
                    |kind| kind.name().to_owned(),
                );
            write!(out, "{:<26} {:>7}", name, counts.objects.len())?;
            if handles {
                write!(out, " {:>10}", counts.handles)?;
            }
            writeln!(out)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/summary.rs"]
mod tests;
