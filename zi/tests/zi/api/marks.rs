use std::ops::Range;

use crate::new;

fn check_marks_eq<'a>(
    marks: impl IntoIterator<Item = (Range<usize>, &'a zi::Mark)>,
    expected: impl IntoIterator<Item = (Range<usize>, zi::MarkId)>,
) {
    let marks = marks.into_iter().map(|(range, mark)| (range, mark.id())).collect::<Vec<_>>();
    let expected = expected.into_iter().collect::<Vec<_>>();
    assert_eq!(marks, expected);
}

#[tokio::test]
async fn marks() {
    let cx = new("abc\ndef\n").await;

    cx.with(|editor| {
        let a = editor.create_mark(zi::Active, zi::Mark::builder(2));
        check_marks_eq(editor.marks(zi::Active, ..), [(2..2, a)]);

        let b = editor.create_mark(zi::Active, zi::Mark::builder(4));
        check_marks_eq(editor.marks(zi::Active, ..), [(2..2, a), (4..4, b)]);

        editor.edit(zi::Active, &zi::deltas![0..1 => ""]).unwrap();
        check_marks_eq(editor.marks(zi::Active, ..), [(1..1, a), (3..3, b)]);
    })
    .await;

    cx.cleanup().await;
}
