use expect_test::{expect, Expect};

use super::*;
use crate::style::Style;

fn r(s: &str) -> PointRange {
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
    check("9:1..9:1", "9:0..9:29", ("9:0..9:1", "9:1..9:1", "9:1..9:29"));
}

#[test]
#[should_panic]
fn invalid_range() {
    r("1:5..1:1");
}

#[test]
fn range_merge_iter() {
    #[track_caller]
    fn check(
        xs: impl IntoIterator<Item = (&'static str, &'static str)>,
        ys: impl IntoIterator<Item = (&'static str, &'static str)>,
        expect: Expect,
    ) {
        use fmt::Write;
        let ts = RangeMergeIter::<_, _, Style>::new(
            xs.into_iter().map(|(s, t)| (s.parse::<PointRange>().unwrap(), t.parse().unwrap())),
            ys.into_iter().map(|(s, t)| (s.parse::<PointRange>().unwrap(), t.parse().unwrap())),
        );

        let mut display = String::new();
        let mut prev: Option<PointRange> = None;
        for (range, style) in ts {
            // help RA out a bit with inference
            let range: PointRange = range;
            let style: Style = style;
            assert!(!range.is_empty(), "should not yield empty ranges");

            if let Some(prev_range) = prev {
                // ranges yielded should be non-overlapping and in order
                assert!(
                    !prev_range.intersects(&range),
                    "prev and current range intersected: {prev_range} {range}"
                );
                assert!(
                    prev_range.end <= range.start,
                    "prev and current range are not ordered correctly: {prev_range} {range}"
                );
            }
            prev = Some(range);

            writeln!(display, "{range} => {style}").unwrap();
        }
        expect.assert_eq(&display);
    }

    check([], [], expect![]);

    check(
        [("1:1..1:5", "fg=#000102")],
        [("1:2..1:2", "bg=#010203")],
        expect![[r#"
            1:1..1:2 => fg=#000102
            1:2..1:5 => fg=#000102
        "#]],
    );

    check(
        [("1:1..1:5", "fg=#000102")],
        [("1:1..1:5", "bg=#010203")],
        expect![[r#"
            1:1..1:5 => fg=#000102 bg=#010203
        "#]],
    );

    check(
        [("1:1..1:5", "fg=#000102")],
        [("1:1..1:6", "bg=#010203")],
        expect![[r#"
            1:1..1:5 => fg=#000102 bg=#010203
            1:5..1:6 => bg=#010203
        "#]],
    );

    check(
        [("1:1..1:5", "fg=#000102")],
        [("1:2..1:6", "bg=#010203")],
        expect![[r#"
            1:1..1:2 => fg=#000102
            1:2..1:5 => fg=#000102 bg=#010203
            1:5..1:6 => bg=#010203
        "#]],
    );

    check(
        [("1:2..1:5", "fg=#000102")],
        [("1:1..1:6", "bg=#010203")],
        expect![[r#"
            1:1..1:2 => bg=#010203
            1:2..1:5 => fg=#000102 bg=#010203
            1:5..1:6 => bg=#010203
        "#]],
    );

    check(
        [("1:2..1:10", "fg=#000102")],
        [("1:1..1:6", "bg=#010203")],
        expect![[r#"
            1:1..1:2 => bg=#010203
            1:2..1:6 => fg=#000102 bg=#010203
            1:6..1:10 => fg=#000102
        "#]],
    );

    check(
        [("1:2..1:10", "fg=#000102")],
        [("1:2..1:10", "bg=#010203")],
        expect![[r#"
            1:2..1:10 => fg=#000102 bg=#010203
        "#]],
    );

    check(
        [("1:1..1:6", "fg=#000102"), ("1:8..1:12", "fg=#020304")],
        [("1:2..1:10", "fg=#424242 bg=#010203")],
        expect![[r#"
            1:1..1:2 => fg=#000102
            1:2..1:6 => fg=#424242 bg=#010203
            1:6..1:8 => fg=#424242 bg=#010203
            1:8..1:10 => fg=#424242 bg=#010203
            1:10..1:12 => fg=#020304
        "#]],
    );

    check(
        [("1:1..1:4", "fg=#123456"), ("1:6..1:10", "fg=#abcdef")],
        [("1:1..1:100", "bg=#000000")],
        expect![[r#"
            1:1..1:4 => fg=#123456 bg=#000000
            1:4..1:6 => bg=#000000
            1:6..1:10 => fg=#abcdef bg=#000000
            1:10..1:100 => bg=#000000
        "#]],
    );

    check(
        [("1:1..1:4", "fg=#123456"), ("1:6..1:10", "fg=#abcdef")],
        [],
        expect![[r#"
            1:1..1:4 => fg=#123456
            1:6..1:10 => fg=#abcdef
        "#]],
    );

    check(
        [("1:1..1:4", "fg=#123456"), ("1:6..1:10", "fg=#abcdef"), ("2:1..2:5", "fg=#13579a")],
        [("1:1..1:100", "bg=#000000")],
        expect![[r#"
            1:1..1:4 => fg=#123456 bg=#000000
            1:4..1:6 => bg=#000000
            1:6..1:10 => fg=#abcdef bg=#000000
            1:10..1:100 => bg=#000000
            2:1..2:5 => fg=#13579a
        "#]],
    );
}
