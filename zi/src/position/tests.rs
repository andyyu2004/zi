use super::*;

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
fn segments() {
    #[track_caller]
    fn check(a: &str, b: &str, (before, inside, after): (&str, &str, &str)) {
        assert_eq!(r(a).segments(&r(b)), (r(before), r(inside), r(after)));
        assert_eq!(r(b).segments(&r(a)), (r(before), r(inside), r(after)));
    }

    check("1:1..1:5", "1:3..1:10", ("1:1..1:3", "1:3..1:5", "1:5..1:10"));
}

#[test]
#[should_panic]
fn invalid_range() {
    r("1:5..1:1");
}
