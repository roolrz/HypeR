// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Record layout admission used by the Native ABI validator.

mod records;

use super::{Error, invalid, schema, validate_identifier};
pub(super) use records::{require_record_field, validate_records};
