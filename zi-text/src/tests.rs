use proptest::prelude::*;
use proptest::proptest;

use super::*;

#[test]
fn invert_deltas() {
    #[track_caller]
    fn t(text: &str, expected: &str, deltas: impl IntoIterator<Item = Delta<'static>>) {
        let deltas = Deltas::new(deltas);
        let mut text = crop::Rope::from(text);
        let original = text.clone();
        let inverted = deltas.apply(&mut text);
        assert_eq!(
            text.to_string(),
            expected,
            "applying the deltas should result in the expected text"
        );
        inverted.apply(&mut text);
        assert_eq!(
            text, original,
            "applying the inverse deltas should result in the original text"
        );
    }

    t("abc", "xabc", [Delta::new(0..0, "x")]);
    t("abc", "xb", [Delta::new(0..1, "x"), Delta::new(2..3, "")]);
    t("abc", "xab", [Delta::new(0..0, "x"), Delta::new(2..3, "")]);

    t(
        "abc
def
ghi",
        "xabc
ydef
zghi",
        [Delta::new(0..0, "x"), Delta::new(4..4, "y"), Delta::new(8..8, "z")],
    );
}

#[track_caller]
fn check_apply_and_invert(
    text: &str,
    expected: &str,
    deltas: impl IntoIterator<Item = Delta<'static>>,
) {
    let deltas = Deltas::new(deltas);
    let mut text = crop::Rope::from(text);
    let original = text.clone();
    let inverted = deltas.apply(&mut text);
    assert_eq!(text.to_string(), expected);
    inverted.apply(&mut text);
    assert_eq!(text, original);
}

#[test]
fn adjacent_replace_replace() {
    let t = check_apply_and_invert;
    t("abc", "xyc", [Delta::new(0..1, "x"), Delta::new(1..2, "y")]);
    t("abc", "xyc", [Delta::new(1..2, "y"), Delta::new(0..1, "x")]);
    t("abcd", "d", [Delta::new(0..1, ""), Delta::new(1..3, "")]);
    t("abc", "c", [Delta::new(0..1, ""), Delta::new(1..2, "")]);
}

#[test]
fn adjacent_replace_then_insert_at_end() {
    let t = check_apply_and_invert;
    t("abc", "xybc", [Delta::new(0..1, "x"), Delta::new(1..1, "y")]);
    t("abc", "xybc", [Delta::new(1..1, "y"), Delta::new(0..1, "x")]);
    t("abc", "ybc", [Delta::new(0..1, ""), Delta::new(1..1, "y")]);
}

#[test]
fn adjacent_insert_at_start_of_replace() {
    let t = check_apply_and_invert;
    t("abc", "xybc", [Delta::new(0..0, "x"), Delta::new(0..1, "y")]);
    t("abc", "xybc", [Delta::new(0..1, "y"), Delta::new(0..0, "x")]);
    t("abc", "ybc", [Delta::new(0..0, "y"), Delta::new(0..1, "")]);
}

#[test]
fn inserts_at_same_point_keep_order() {
    let t = check_apply_and_invert;
    t("abc", "axyzbc", [Delta::new(1..1, "x"), Delta::new(1..1, "y"), Delta::new(1..1, "z")]);
    t("abc", "xyzbc", [Delta::new(0..0, "x"), Delta::new(0..1, "z"), Delta::new(0..0, "y")]);
}

#[test]
#[should_panic(expected = "deltas must not overlap")]
fn overlapping_deltas() {
    let _ = Deltas::new([Delta::new(0..2, "x"), Delta::new(1..3, "y")]);
}

#[test]
#[should_panic(expected = "deltas must not overlap")]
fn overlapping_deltas_insert_inside() {
    let _ = Deltas::new([Delta::new(0..2, "x"), Delta::new(1..1, "y")]);
}

#[test]
fn floor_char_boundary() {
    let s = "aé日€b".repeat(1000);
    let rope = crop::Rope::from(s.as_str());
    assert!(rope.chunks().count() > 1);
    let slice = rope.byte_slice(..);
    for i in 0..=s.len() + 2 {
        let expected = s.floor_char_boundary(i);
        assert_eq!(TextSlice::floor_char_boundary(&s.as_str(), i), expected, "str at {i}");
        assert_eq!(TextSlice::floor_char_boundary(&slice, i), expected, "rope at {i}");
    }
}

// return ascii only so any byte range is valid
fn ascii_str_and_range() -> impl Strategy<Value = (String, ops::Range<usize>)> {
    // using a prefix character x and + to ensure string of length at least 2
    "x[ -~]+"
        .prop_flat_map(|vec| {
            let len = vec.len();
            // minus 1 here to ensure the next step will pass a non-empty range
            (Just(vec), 0..len - 1)
        })
        .prop_flat_map(|(vec, start)| {
            let end = vec.len();
            assert!(start < end, "proptest requires non-empty range");
            (Just(vec), Just(start), start..end)
        })
        .prop_map(|(vec, start, end)| (vec, start..end))
}

proptest! {
    #[test]
    fn prop_invert_delta((mut s, range) in ascii_str_and_range(), replacement in "[ -~]*") {
        let delta = Delta::new(range, replacement);
        let original = s.clone();
        let invert = delta.apply(&mut s);
        invert.apply(&mut s);
        assert_eq!(s, original, "applying the inverse delta should result in the original text");
    }
}
