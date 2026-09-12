// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

use super::{CommandLine, ParseError};

#[test]
fn parses_quotes_escapes_and_empty_arguments() -> Result<(), ParseError> {
    let command = CommandLine::parse(br#"echo "two words" 'three' four\ five """#)?;
    assert!(!command.is_empty());
    assert_eq!(command.len(), 5);
    assert_eq!(command.argument(0), Some("echo"));
    assert_eq!(command.argument(1), Some("two words"));
    assert_eq!(command.argument(2), Some("three"));
    assert_eq!(command.argument(3), Some("four five"));
    assert_eq!(command.argument(4), Some(""));
    Ok(())
}

#[test]
fn recognizes_an_empty_command_line() -> Result<(), ParseError> {
    let command = CommandLine::parse(b" \t ")?;
    assert!(command.is_empty());
    assert_eq!(command.len(), 0);
    Ok(())
}

#[test]
fn rejects_oversized_lines_arguments_and_nul() {
    assert!(matches!(
        CommandLine::parse(&vec![b'x'; super::MAX_LINE_BYTES + 1]),
        Err(ParseError::LineTooLong)
    ));
    let too_many = vec!["x"; super::MAX_ARGUMENTS + 1].join(" ");
    assert!(matches!(
        CommandLine::parse(too_many.as_bytes()),
        Err(ParseError::TooManyArguments)
    ));
    assert!(matches!(
        CommandLine::parse(b"echo a\0b"),
        Err(ParseError::InvalidSyntax)
    ));
}

#[test]
fn follows_posix_comments_and_double_quote_escaping() -> Result<(), ParseError> {
    let command = CommandLine::parse(br#"echo "a\qb" '# literal' # comment"#)?;
    assert_eq!(
        command.arguments().collect::<Vec<_>>(),
        ["echo", r"a\qb", "# literal"]
    );
    Ok(())
}

#[test]
fn rejects_incomplete_syntax() {
    assert!(matches!(
        CommandLine::parse(b"echo 'missing"),
        Err(ParseError::InvalidSyntax)
    ));
    assert!(matches!(
        CommandLine::parse(b"echo trailing\\"),
        Err(ParseError::InvalidSyntax)
    ));
}

#[test]
fn pipeline_operators_respect_quotes_and_escapes() -> Result<(), ParseError> {
    let pipeline = super::Pipeline::parse(br#"echo 'a|b' \>x|grep "a|b">out 2>>err"#)?;
    assert_eq!(pipeline.0.len(), 2);
    assert_eq!(
        pipeline.0[0].command.arguments().collect::<Vec<_>>(),
        ["echo", "a|b", ">x"]
    );
    assert_eq!(pipeline.0[1].redirects[0].path, "out");
    assert_eq!(pipeline.0[1].redirects[1].stream, 2);
    assert!(pipeline.0[1].redirects[1].append);
    Ok(())
}
#[test]
fn malformed_pipelines_are_rejected() {
    for input in [
        "| cat",
        "cat |",
        "cat || cat",
        "cat >",
        "cat < | cat",
        "echo a; echo b",
        "cat &",
        "cat 'bad",
    ] {
        assert!(super::Pipeline::parse(input.as_bytes()).is_err(), "{input}");
    }
}
#[test]
fn redirects_preserve_order_and_comments() -> Result<(), ParseError> {
    let pipeline = super::Pipeline::parse(b"cat<input >one >>two # | ignored")?;
    assert_eq!(pipeline.0.len(), 1);
    assert_eq!(
        pipeline.0[0]
            .redirects
            .iter()
            .map(|r| r.path.as_str())
            .collect::<Vec<_>>(),
        ["input", "one", "two"]
    );
    Ok(())
}

#[test]
fn escaped_whitespace_does_not_start_comment_or_fd_prefix() -> Result<(), ParseError> {
    let pipeline = super::Pipeline::parse(br"echo \ #literal \ 2>out")?;
    assert_eq!(
        pipeline.0[0].command.arguments().collect::<Vec<_>>(),
        ["echo", " #literal", " 2"]
    );
    assert_eq!(pipeline.0[0].redirects[0].stream, 1);
    Ok(())
}
