//! `find` and `find_range` with every kind of index and none, and the types queries return.

use rel::{Column, Database, Id, Ref, Relation};

#[derive(Relation, Debug, Clone, PartialEq)]
struct Item {
    #[index(unique)]
    name: String,
    #[index(ordered)]
    price: u32,
    #[index]
    colour: String,
    stock: u32,
}

#[derive(Database, Default)]
struct Shop {
    items: Relation<Item>,
}

fn shop() -> (Shop, [Id<Item>; 3]) {
    let mut db = Shop::default();
    let mut item = |name: &str, price, colour: &str, stock| {
        db.insert(Item { name: name.into(), price, colour: colour.into(), stock }).unwrap()
    };
    let ids = [item("pen", 2, "red", 10), item("ink", 5, "red", 0), item("pad", 3, "blue", 4)];
    (db, ids)
}

fn ids<'a>(rows: impl Iterator<Item = Ref<'a, Item>>) -> Vec<Id<Item>> {
    let mut ids = rows.map(|r| r.id()).collect::<Vec<_>>();
    ids.sort();
    ids
}

#[test]
fn find_uses_each_index_kind_or_scans() {
    let (db, [pen, ink, pad]) = shop();
    // Unique hash, ordered, hash, and no index.
    assert_eq!(ids(db.items.find(item::name, "ink".into())), [ink]);
    assert_eq!(ids(db.items.find(item::name, "nib".into())), []);
    assert_eq!(ids(db.items.find(item::price, 3)), [pad]);
    assert_eq!(ids(db.items.find(item::colour, "red".into())), [pen, ink]);
    assert_eq!(ids(db.items.find(item::stock, 0)), [ink]);
}

#[test]
fn find_range_uses_an_ordered_index_or_scans() {
    let (db, [pen, ink, pad]) = shop();
    // Ordered index.
    assert_eq!(ids(db.items.find_range(item::price, 3..)), [ink, pad]);
    // A hash index can't answer a range, and neither can no index: both scan.
    assert_eq!(ids(db.items.find_range(item::name, "j".to_string()..)), [pen, pad]);
    assert_eq!(ids(db.items.find_range(item::stock, 1..=10)), [pen, pad]);
}

#[test]
fn ref_gives_the_id_and_row() {
    let (db, [pen, ..]) = shop();
    let r = db.items.get_ref(pen).unwrap();
    let copy = Clone::clone(&r);
    assert_eq!(copy.id(), pen);
    assert_eq!(copy.row().name, "pen");
    // Derefs to the row.
    assert_eq!(r.price, 2);
    assert_eq!(
        format!("{r:?}"),
        r#"Item#0v0 Item { name: "pen", price: 2, colour: "red", stock: 10 }"#
    );
}

#[test]
fn columns_are_copyable_handles() {
    let (db, [pen, ..]) = shop();
    let column: Column<Item, u32> = Clone::clone(&item::stock);
    assert_eq!(column.name(), "stock");
    assert_eq!(*column.get(&db.items[pen]), 10);
}

#[test]
fn iter_mut_changes_unindexed_columns() {
    let (mut db, [pen, ink, pad]) = shop();
    for (id, row) in db.items.iter_mut() {
        if id != ink {
            *row.stock += 1;
        }
        // Indexed columns are read-only here.
        let _: &u32 = row.price;
    }
    assert_eq!([pen, ink, pad].map(|id| db.items[id].stock), [11, 0, 5]);
    assert_eq!(db.items.ids().collect::<Vec<_>>(), [pen, ink, pad]);
}

#[test]
fn ids_of_dead_or_foreign_rows_get_nothing() {
    let (mut db, [pen, ink, pad]) = shop();
    db.delete(ink).unwrap();
    assert!(db.items.get(ink).is_none());
    assert!(db.items.get_mut(ink).is_none());

    // An id from a relation with more slots than this one.
    let mut empty = Relation::<Item>::default();
    assert!(empty.get(pad).is_none());
    assert!(empty.get_mut(pen).is_none());
    assert!(empty.get_ref(pad).is_none());
}

#[test]
fn iteration_skips_deleted_rows() {
    let (mut db, [pen, ink, pad]) = shop();
    db.delete(ink).unwrap();
    assert_eq!(db.items.ids().collect::<Vec<_>>(), [pen, pad]);
    assert_eq!(db.items.iter_mut().count(), 2);
    assert_eq!(db.items.len(), 2);
}

#[test]
fn debug_output() {
    let (mut db, [_, ink, pad]) = shop();
    db.delete(ink).unwrap();
    db.delete(pad).unwrap();
    assert_eq!(
        format!("{:?}", db.items),
        r#"{Item#0v0: Item { name: "pen", price: 2, colour: "red", stock: 10 }}"#
    );
    assert_eq!(format!("{ink:?}"), "Item#1v0");
}

#[test]
#[should_panic(expected = "no such Item: Item#1v0")]
fn indexing_a_deleted_row_panics() {
    let (mut db, [_, ink, _]) = shop();
    db.delete(ink).unwrap();
    let _ = &db.items[ink];
}
