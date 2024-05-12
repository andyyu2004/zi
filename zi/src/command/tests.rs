use expect_test::expect;

use super::{Command, Commands};

#[test]
fn parse_commands() {
    for (src, expect) in [
        (
            r#"set x y; set y z"#,
            expect![[r#"
                set x y;set y z;
            "#]],
        ),
        (
            // trailing semicolon
            r#"set x y; set y z;"#,
            expect![[r#"
                set x y;set y z;
            "#]],
        ),
        (
            // newline separator
            r#"set x y
set y z"#,
            expect![[r#"
                set x y;set y z;
            "#]],
        ),
    ] {
        match src.parse::<Commands>() {
            Ok(cmd) => expect.assert_debug_eq(&cmd),
            Err(err) => expect.assert_eq(&err.to_string()),
        };
    }
}

#[test]
fn parse_command() {
    for (src, expect) in [
        (
            "foo",
            expect![[r#"
            foo
        "#]],
        ),
        (
            "foo bar",
            expect![[r#"
            foo bar
        "#]],
        ),
        (
            "set x y",
            expect![[r#"
                set x y
            "#]],
        ),
        (":extra colon", expect![[r#"found ":""#]]),
        (" \n", expect![[r#"found "\n""#]]),
    ] {
        match src.parse::<Command>() {
            Ok(cmd) => expect.assert_debug_eq(&cmd),
            Err(err) => expect.assert_eq(&err.to_string()),
        };
    }
}
