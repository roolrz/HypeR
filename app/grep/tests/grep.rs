// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0
use super::*;

#[test]
fn selected_line_limit_respects_inversion_and_zero_never_reads()
-> Result<(), Box<dyn std::error::Error>> {
    let args = Grep::try_parse_from(["grep", "-m", "2", "-vcn", "skip"])?;
    let (regex, _) = args.matcher()?;
    let mut input = io::Cursor::new(b"skip\none\ntwo\nthree\n");
    let mut output = Vec::new();
    assert!(args.scan(&regex, &mut input, &mut output, "file", false)?);
    assert_eq!(output, b"2\n");
    assert_eq!(input.position(), 13);
    let args = Grep::try_parse_from(["grep", "-m", "0", "-c", "."])?;
    output.clear();
    assert!(!args.scan(&regex, &mut input, &mut output, "file", false)?);
    assert_eq!(input.position(), 13);
    assert_eq!(output, b"0\n");
    Ok(())
}

#[test]
fn full_line_matching_wraps_all_alternatives_and_literal_patterns()
-> Result<(), Box<dyn std::error::Error>> {
    let args = Grep::try_parse_from(["grep", "-x", "-e", "one|two", "-e", "three"])?;
    let (regex, _) = args.matcher()?;
    assert!(regex.is_match(b"two"));
    assert!(regex.is_match(b"three"));
    assert!(!regex.is_match(b"twosome"));
    assert!(!regex.is_match(b"someone"));
    let args = Grep::try_parse_from(["grep", "-Fx", "a.b"])?;
    let (regex, _) = args.matcher()?;
    assert!(regex.is_match(b"a.b"));
    assert!(!regex.is_match(b"axb"));
    Ok(())
}
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
