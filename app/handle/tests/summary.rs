// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::*;
use hyper_os::handle::{FileObject, TypedObject};

#[test]
fn duplicated_handles_share_one_object_but_recycled_ids_do_not() -> io::Result<()> {
    let mut summary = Summary::default();
    summary.record(FileObject::KIND, 0x100000001, true);
    summary.record(FileObject::KIND, 0x100000001, true);
    summary.record(FileObject::KIND, 0x200000001, true);
    let mut output = Vec::new();
    summary.write(true, false, &mut output)?;
    let row = String::from_utf8(output).map_err(io::Error::other)?;
    assert_eq!(
        row.split_whitespace().collect::<Vec<_>>(),
        ["file", "2", "3"]
    );
    Ok(())
}
