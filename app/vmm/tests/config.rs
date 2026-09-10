// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

#[test]
fn saving_refuses_to_clobber_an_existing_config() -> std::io::Result<()> {
    let path = std::env::temp_dir().join(format!("hyper-vmm-save-{}.json", std::process::id()));
    let _ = std::fs::remove_file(&path);
    super::save_config(&path, vec![])?;
    let original = std::fs::read(&path)?;
    assert!(
        super::save_config(&path, vec![])
            .is_err_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists)
    );
    assert_eq!(std::fs::read(&path)?, original);
    std::fs::remove_file(path)
}
