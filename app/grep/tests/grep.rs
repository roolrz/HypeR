// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0
use super::*;
#[test]
fn regex_lines_and_unterminated_input() -> Result<(), Box<dyn std::error::Error>> {
    let args = Grep::try_parse_from(["grep", "-in", "^hello$"])?;
    let (regex, _) = args.matcher()?;
    let mut output = Vec::new();
    assert!(args.scan(&regex, &b"no\nHello\nHELLO"[..], &mut output, "-", false)?);
    assert_eq!(output, b"2:Hello\n3:HELLO\n");
    Ok(())
}
#[test]
fn fixed_invert_count_and_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let args = Grep::try_parse_from(["grep", "-Fvc", "a.b"])?;
    let (regex, _) = args.matcher()?;
    let mut output = Vec::new();
    assert!(args.scan(&regex, &b"a.b\naxb\n\xff\n"[..], &mut output, "file", true)?);
    assert_eq!(output, b"file:2\n");
    Ok(())
}
#[test]
fn multiple_patterns_and_missing_pattern() -> Result<(), Box<dyn std::error::Error>> {
    let args = Grep::try_parse_from(["grep", "-e", "one", "-e", "two", "file"])?;
    let (regex, files) = args.matcher()?;
    assert!(regex.is_match(b"two"));
    assert_eq!(files, vec![PathBuf::from("file")]);
    assert!(Grep::try_parse_from(["grep"])?.matcher().is_err());
    Ok(())
}

#[test]
fn quiet_and_filename_modes_override_counts() -> Result<(), Box<dyn std::error::Error>> {
    for flag in ["-qc", "-lc"] {
        let args = Grep::try_parse_from(["grep", flag, "absent"])?;
        let (regex, _) = args.matcher()?;
        let mut output = Vec::new();
        assert!(!args.scan(&regex, &b"text\n"[..], &mut output, "file", true)?);
        assert!(output.is_empty());
    }
    Ok(())
}
