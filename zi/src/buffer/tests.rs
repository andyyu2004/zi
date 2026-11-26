use zi_core::NamespaceId;

use super::mark::{Mark, Marks};

#[test]
fn replace_marks_many_times() {
    let mut marks = Marks::default();
    let ns = <NamespaceId as rel::Key>::from_raw(rel::RawId::new(0, 0));
    for _ in 0..3000 {
        marks.replace(10, ns, [Mark::builder(1), Mark::builder(3).width(2), Mark::builder(7)]);
    }

    let ranges = marks.iter(..).map(|(n, range, _)| (n, range)).collect::<Vec<_>>();
    assert_eq!(ranges, [(ns, 1..1), (ns, 3..5), (ns, 7..7)]);

    let ids = marks.iter(..).map(|(_, _, mark)| mark.id()).collect::<Vec<_>>();
    for (i, &id) in ids.iter().enumerate() {
        assert!(!ids[..i].contains(&id));
        assert!(marks.delete(ns, id).is_some());
    }
    assert_eq!(marks.iter(..).count(), 0);
}

#[test]
fn marks_at_eof() {
    let ns = <NamespaceId as rel::Key>::from_raw(rel::RawId::new(0, 0));
    let mut created = Marks::default();
    created.create(10, ns, Mark::builder(10));
    let mut replaced = Marks::default();
    replaced.replace(10, ns, [Mark::builder(10)]);

    for mut marks in [created, replaced] {
        let ranges = marks.iter(..).map(|(_, range, _)| range).collect::<Vec<_>>();
        assert_eq!(ranges, [10..10]);
        marks.drain(ns, ..);
        assert_eq!(marks.iter(..).count(), 0);
    }
}
