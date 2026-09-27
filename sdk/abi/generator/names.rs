// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Public ABI identifier spelling shared by validation and renderers.

pub(super) fn upper_snake(identifier: &str) -> String {
    identifier.to_ascii_uppercase()
}

pub(super) fn upper_camel(identifier: &str) -> String {
    let mut output = String::new();
    for component in identifier.split('_') {
        let mut characters = component.chars();
        if let Some(first) = characters.next() {
            output.extend(first.to_uppercase());
            output.extend(characters);
        }
    }
    output
}
