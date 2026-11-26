use std::ops::Range;

use zi::NamespaceId;

use crate::new;

fn check_marks_eq<'a>(
    marks: impl IntoIterator<Item = (NamespaceId, Range<usize>, &'a zi::Mark)>,
    expected: impl IntoIterator<Item = (NamespaceId, Range<usize>, zi::MarkId)>,
) {
    let marks =
        marks.into_iter().map(|(ns, range, mark)| (ns, range, mark.id())).collect::<Vec<_>>();
    let expected = expected.into_iter().collect::<Vec<_>>();
    assert_eq!(marks, expected);
}

#[tokio::test]
async fn marks() {
    let cx = new("abc\ndef\n").await;

    cx.with(|editor| {
        let ns = editor.default_namespace();
        let a = editor.create_mark(zi::Active, ns, zi::Mark::builder(2));
        check_marks_eq(editor.marks(zi::Active, ..), [(ns, 2..2, a)]);

        let b = editor.create_mark(zi::Active, ns, zi::Mark::builder(4));
        check_marks_eq(editor.marks(zi::Active, ..), [(ns, 2..2, a), (ns, 4..4, b)]);

        editor.edit(zi::Active, &zi::deltas![0..1 => ""]).unwrap();
        check_marks_eq(editor.marks(zi::Active, ..), [(ns, 1..1, a), (ns, 3..3, b)]);
    })
    .await;

    cx.cleanup().await;
}

#[tokio::test]
async fn create_and_delete_mark_many_times() {
    let cx = new("abc\ndef\n").await;

    cx.with(|editor| {
        let ns = editor.default_namespace();
        for _ in 0..3000 {
            let id = editor.create_mark(zi::Active, ns, zi::Mark::builder(2));
            check_marks_eq(editor.marks(zi::Active, ..), [(ns, 2..2, id)]);
            editor.delete_mark(zi::Active, ns, id);
            check_marks_eq(editor.marks(zi::Active, ..), []);
        }
    })
    .await;

    cx.cleanup().await;
}
