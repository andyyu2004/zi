use expect_test::{expect, Expect};

use super::*;
use crate::{Color, Style};

fn r(s: &str) -> Range {
    s.parse().unwrap()
}

#[test]
fn intersection() {
    #[track_caller]
    fn check(a: &str, b: &str, expected: bool) {
        if expected {
            assert!(r(a).intersects(&r(b)));
            assert!(r(b).intersects(&r(a)));
        } else {
            assert!(!r(a).intersects(&r(b)));
            assert!(!r(b).intersects(&r(a)));
        }
    }

    check("1:1..1:1", "1:1..1:1", false);
    check("1:1..1:5", "1:5..1:10", false);
    check("1:1..1:5", "1:6..1:10", false);
    check("1:1..1:5", "1:3..1:10", true);
}

#[test]
fn range_segments() {
    #[track_caller]
    fn check(a: &str, b: &str, (before, inside, after): (&str, &str, &str)) {
        assert_eq!(r(a).segments(&r(b)), (r(before), r(inside), r(after)));
        assert_eq!(r(b).segments(&r(a)), (r(before), r(inside), r(after)));
    }

    check("1:1..1:5", "1:3..1:10", ("1:1..1:3", "1:3..1:5", "1:5..1:10"));
    check("1:1..1:5", "1:1..1:5", ("1:1..1:1", "1:1..1:5", "1:5..1:5"));
}

#[test]
#[should_panic]
fn invalid_range() {
    r("1:5..1:1");
}

#[test]
fn range_merge_iter() {
    #[track_caller]
    fn check<T>(
        xs: impl IntoIterator<Item = (&'static str, T)>,
        ys: impl IntoIterator<Item = (&'static str, T)>,
        expect: Expect,
    ) where
        T: Merge + Copy + fmt::Debug,
    {
        use fmt::Write;
        let ts = RangeMergeIter::<_, _, T>::new(
            xs.into_iter().map(|(s, t)| (s.parse::<Range>().unwrap(), t)),
            ys.into_iter().map(|(s, t)| (s.parse::<Range>().unwrap(), t)),
        );
        let mut display = String::new();
        for (range, t) in ts {
            writeln!(display, "{range} => {t:?}").unwrap();
        }
        expect.assert_eq(&display);
    }

    check(
        [("1:1..1:5", *Style::new().with_fg(Color::Rgb(0, 1, 2)))],
        [("1:1..1:5", *Style::new().with_bg(Color::Rgb(1, 2, 3)))],
        expect![[r#"
            1:1..1:5 => Style { fg=#000102 bg=#010203 }
        "#]],
    );

    check(
        [("1:1..1:6", *Style::new().with_fg(Color::Rgb(0, 1, 2)))],
        [("1:1..1:5", *Style::new().with_bg(Color::Rgb(1, 2, 3)))],
        expect![[r#"
            1:1..1:5 => Style { fg=#000102 bg=#010203 }
            1:5..1:6 => Style { fg=#000102 }
        "#]],
    );

    check(
        [("1:1..1:5", *Style::new().with_fg(Color::Rgb(0, 1, 2)))],
        [("1:1..1:6", *Style::new().with_bg(Color::Rgb(1, 2, 3)))],
        expect![[r#"
            1:1..1:5 => Style { fg=#000102 bg=#010203 }
            1:5..1:6 => Style { bg=#010203 }
        "#]],
    );

    check(
        [("1:2..1:5", *Style::new().with_fg(Color::Rgb(0, 1, 2)))],
        [("1:1..1:6", *Style::new().with_bg(Color::Rgb(1, 2, 3)))],
        expect![[r#"
            1:1..1:2 => Style { bg=#010203 }
            1:2..1:5 => Style { fg=#000102 bg=#010203 }
            1:5..1:6 => Style { bg=#010203 }
        "#]],
    );

    check(
        [("1:2..1:10", *Style::new().with_fg(Color::Rgb(0, 1, 2)))],
        [("1:1..1:6", *Style::new().with_bg(Color::Rgb(1, 2, 3)))],
        expect![[r#"
            1:1..1:2 => Style { bg=#010203 }
            1:2..1:6 => Style { fg=#000102 bg=#010203 }
            1:6..1:10 => Style { fg=#000102 }
        "#]],
    );

    check(
        [("1:1..1:6", *Style::new().with_fg(Color::Rgb(0, 1, 2)))],
        [("1:2..1:10", *Style::new().with_bg(Color::Rgb(1, 2, 3)))],
        expect![[r#"
            1:1..1:2 => Style { fg=#000102 }
            1:2..1:6 => Style { fg=#000102 bg=#010203 }
            1:6..1:10 => Style { bg=#010203 }
        "#]],
    );

    check(
        [
            ("1:1..1:6", *Style::new().with_fg(Color::Rgb(0, 1, 2))),
            ("1:8..1:12", *Style::new().with_fg(Color::Rgb(2, 3, 4))),
        ],
        [("1:2..1:10", *Style::new().with_fg(Color::Rgb(42, 42, 42)).with_bg(Color::Rgb(1, 2, 3)))],
        expect![[r#"
            1:1..1:2 => Style { fg=#000102 }
            1:2..1:6 => Style { fg=#2a2a2a bg=#010203 }
            1:6..1:10 => Style { fg=#2a2a2a bg=#010203 }
            1:8..1:12 => Style { fg=#020304 }
        "#]],
    );
}
