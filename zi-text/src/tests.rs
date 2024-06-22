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
