use zi_text::{deltas, Bias, MarkTree};

fn check_marks(tree: &MarkTree, expect: impl IntoIterator<Item = usize>) {
    let marks = tree.iter();
    assert!(marks.eq(expect));
}

#[test]
fn mark_tree() {
    let mut tree = MarkTree::default();
    tree.add(1, Bias::Left);
    tree.add(2, Bias::Left);

    check_marks(&tree, [1, 2]);
    tree.edit(&deltas![1..1 => "x"]);
    check_marks(&tree, [2, 3]);
}
