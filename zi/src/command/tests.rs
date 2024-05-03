use expect_test::expect;

use super::Command;

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
        (
            "set x=y",
            expect![[r#"
                set x=y
            "#]],
        ),
        (
            ":extra colon",
            expect![[r#"
                :extra colon
            "#]],
        ),
        (" \n", expect!["found end of input"]),
    ] {
        match src.parse::<Command>() {
            Ok(cmd) => expect.assert_debug_eq(&cmd),
            Err(err) => expect.assert_eq(&err),
        };
    }
}
