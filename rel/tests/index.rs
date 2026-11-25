//! The index types on their own.

use std::ops::Bound;

use rel::{HashIndex, LookupIndex, OrdIndex, UniqueHashIndex, UniqueOrdIndex};

fn all<I: Ord>(ids: impl Iterator<Item = I>) -> Vec<I> {
    let mut ids = ids.collect::<Vec<_>>();
    ids.sort();
    ids
}

const EVERYTHING: (Bound<&str>, Bound<&str>) = (Bound::Unbounded, Bound::Unbounded);

#[test]
fn hash_index() {
    let mut index = HashIndex::<&str, u32>::default();
    index.insert("a", 1);
    index.insert("a", 2);
    index.insert("b", 3);
    assert_eq!(all(index.get("a")), [1, 2]);
    assert_eq!(all(index.lookup(&"b")), [3]);
    assert!(index.range(EVERYTHING).is_none());

    index.remove(&"a", 1);
    assert_eq!(all(index.get("a")), [2]);
    index.remove(&"a", 2);
    assert_eq!(all(index.get("a")), []);
    // Removing what isn't there does nothing.
    index.remove(&"a", 2);
    index.remove(&"b", 4);
    assert_eq!(all(index.get("b")), [3]);
}

#[test]
fn unique_hash_index() {
    let mut index = UniqueHashIndex::<&str, u32>::default();
    index.insert("a", 1);
    assert!(index.contains(&"a"));
    assert_eq!(index.get("a"), Some(1));
    assert_eq!(all(index.lookup(&"a")), [1]);
    assert_eq!(all(index.lookup(&"b")), []);
    assert!(index.range(EVERYTHING).is_none());

    index.remove(&"a", 1);
    assert!(!index.contains(&"a"));
}

#[test]
fn ord_index() {
    let mut index = OrdIndex::<&str, u32>::default();
    index.insert("a", 1);
    index.insert("b", 2);
    index.insert("b", 3);
    index.insert("c", 4);
    assert_eq!(all(index.get("b")), [2, 3]);
    assert_eq!(all(index.lookup(&"c")), [4]);
    assert_eq!(all(OrdIndex::range(&index, "b"..)), [2, 3, 4]);
    let range = (Bound::Excluded("a"), Bound::Excluded("c"));
    assert_eq!(all(LookupIndex::range(&index, range).unwrap()), [2, 3]);

    index.remove(&"b", 2);
    assert_eq!(all(index.get("b")), [3]);
    index.remove(&"b", 3);
    assert_eq!(all(index.get("b")), []);
    index.remove(&"b", 3);
    index.remove(&"c", 5);
    assert_eq!(all(index.get("c")), [4]);
}

#[test]
fn unique_ord_index() {
    let mut index = UniqueOrdIndex::<&str, u32>::default();
    index.insert("a", 1);
    index.insert("b", 2);
    assert!(index.contains(&"b"));
    assert_eq!(index.get("a"), Some(1));
    assert_eq!(all(index.lookup(&"b")), [2]);
    assert_eq!(all(UniqueOrdIndex::range(&index, .."b")), [1]);
    assert_eq!(all(LookupIndex::range(&index, EVERYTHING).unwrap()), [1, 2]);

    index.remove(&"a", 1);
    assert_eq!(index.get("a"), None);
}
