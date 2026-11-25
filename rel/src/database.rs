use std::any::TypeId;
use std::collections::HashSet;

use crate::{Column, Error, Key, OnDelete, RawId, Relation, Result, Row};

/// Access to the relation of `T` in a database. Implemented by `#[derive(Database)]`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no `Relation<{T}>`",
    label = "this needs `{Self}` to have a `Relation<{T}>`",
    note = "a foreign key references `{T}`, so the database needs a `Relation<{T}>` field"
)]
pub trait HasRelation<T: Row> {
    fn relation(&self) -> &Relation<T>;
    fn relation_mut(&mut self) -> &mut Relation<T>;
}

/// The row type an id type belongs to in database `D`, so `db.delete(id)` can infer it.
/// Implemented by `#[derive(Database)]`. Two relations in one database can't share an id type.
pub trait KeyIn<D: ?Sized>: Key {
    type Row: Row<Key = Self>;
}

/// Foreign key checks of a row against the database `D`. Implemented by
/// `#[derive(Relation)]` for every `D` that has the relations the row references,
/// so writing a row to a database without them doesn't compile.
pub trait References<D: ?Sized> {
    fn check_references(&self, db: &D) -> Result<()>;
}

/// A row that refers to a row being deleted.
#[doc(hidden)]
pub struct Child {
    pub row_type: TypeId,
    pub id: RawId,
    pub relation: &'static str,
    pub column: &'static str,
    pub references: &'static str,
    pub on_delete: OnDelete,
    /// The position of the foreign key in the row type's `REFERENCES`.
    pub reference: usize,
}

impl<T: Row> Relation<T> {
    /// Call `f` with every row whose foreign key refers to the row `parent` of type `parent_type`.
    #[doc(hidden)]
    pub fn visit_children(&self, parent_type: TypeId, parent: RawId, f: &mut dyn FnMut(Child)) {
        for (position, reference) in T::REFERENCES.iter().enumerate() {
            if (reference.parent)() != parent_type {
                continue;
            }
            (reference.children)(self.indexes(), parent, &mut |id| {
                f(Child {
                    row_type: TypeId::of::<T>(),
                    id,
                    relation: T::NAME,
                    column: reference.column,
                    references: reference.references,
                    on_delete: reference.on_delete,
                    reference: position,
                });
            });
        }
    }

    /// Remove a row as part of a delete that the database has already checked.
    #[doc(hidden)]
    pub fn remove_checked(&mut self, id: RawId) {
        self.remove(T::Key::from_raw(id));
    }

    /// Null a `set_null` foreign key as part of a delete that the database has already checked.
    #[doc(hidden)]
    pub fn set_null_checked(&mut self, id: RawId, reference: usize) {
        let set_null =
            T::REFERENCES[reference].set_null.expect("only set_null foreign keys are nulled");
        self.modify_unchecked(T::Key::from_raw(id), set_null);
    }
}

/// A set of relations with foreign keys enforced between them. Implemented by `#[derive(Database)]`.
pub trait Database {
    /// Call `f` with every row in the database that refers to the row `parent` of type `parent_type`.
    #[doc(hidden)]
    fn visit_children(&self, parent_type: TypeId, parent: RawId, f: &mut dyn FnMut(Child));

    /// Remove the row `id` of type `row_type`, which a delete has already checked.
    #[doc(hidden)]
    fn remove_checked(&mut self, row_type: TypeId, id: RawId);

    /// Null the foreign key at position `reference` of the row `id` of type `row_type`.
    #[doc(hidden)]
    fn set_null_checked(&mut self, row_type: TypeId, id: RawId, reference: usize);

    fn get<K: KeyIn<Self>>(&self, id: K) -> Option<&K::Row>
    where
        Self: HasRelation<K::Row>,
    {
        HasRelation::<K::Row>::relation(self).get(id)
    }

    /// Insert a row, checking its foreign keys and unique indexes.
    fn insert<T: Row + References<Self>>(&mut self, row: T) -> Result<T::Key>
    where
        Self: HasRelation<T>,
    {
        self.insert_with(|_| row)
    }

    /// Insert a row built from its own id.
    fn insert_with<T: Row + References<Self>>(
        &mut self,
        f: impl FnOnce(T::Key) -> T,
    ) -> Result<T::Key>
    where
        Self: HasRelation<T>,
    {
        let id = HasRelation::<T>::relation_mut(self).reserve();
        let row = f(id);
        if let Err(err) = row.check_references(self) {
            HasRelation::<T>::relation_mut(self).release(id);
            return Err(err);
        }
        HasRelation::<T>::relation_mut(self).fill(id, row)?;
        Ok(id)
    }

    /// Change one column of a row, checking unique indexes and foreign keys.
    /// Returns the old value. On error the row is left as it was.
    fn set<T: Row + References<Self>, K>(
        &mut self,
        id: T::Key,
        column: Column<T, K>,
        value: K,
    ) -> Result<K>
    where
        Self: HasRelation<T>,
    {
        let old = HasRelation::<T>::relation_mut(self).set(id, column.get_mut, value)?;
        let row = HasRelation::<T>::relation(self).get(id).expect("row was just set");
        if let Err(err) = row.check_references(self) {
            HasRelation::<T>::relation_mut(self)
                .set(id, column.get_mut, old)
                .expect("putting the old value back can't break a unique index");
            return Err(err);
        }
        Ok(old)
    }

    /// Change a row. The closure edits a copy; if the result breaks a foreign key
    /// or unique index, the row is left as it was.
    fn update<K: KeyIn<Self>>(&mut self, id: K, f: impl FnOnce(&mut K::Row)) -> Result<()>
    where
        K::Row: Clone + References<Self>,
        Self: HasRelation<K::Row>,
    {
        let mut row = HasRelation::<K::Row>::relation(self)
            .get(id)
            .ok_or(Error::NotFound { relation: K::Row::NAME })?
            .clone();
        f(&mut row);
        row.check_references(self)?;
        HasRelation::<K::Row>::relation_mut(self).replace(id, row)
    }

    /// Delete a row and, through `cascade` foreign keys, the rows that refer to it.
    /// Surviving rows that refer to a deleted row through a `set_null` foreign key
    /// have that column set to `None`.
    ///
    /// A `restrict` foreign key only refuses the delete if the referring row would
    /// survive it, so a row that is itself removed by a cascade doesn't block.
    /// Nothing changes if the delete is refused.
    fn delete<K: KeyIn<Self>>(&mut self, id: K) -> Result<()>
    where
        Self: HasRelation<K::Row>,
    {
        if !HasRelation::<K::Row>::relation(self).contains(id) {
            return Err(Error::NotFound { relation: K::Row::NAME });
        }

        let root = (TypeId::of::<K::Row>(), id.raw());
        let mut doomed = HashSet::from([root]);
        let mut queue = vec![root];
        let mut restricted = Vec::new();
        let mut nulled = Vec::new();

        while let Some((parent_type, parent)) = queue.pop() {
            self.visit_children(parent_type, parent, &mut |child| {
                let key = (child.row_type, child.id);
                match child.on_delete {
                    OnDelete::Cascade => {
                        if doomed.insert(key) {
                            queue.push(key);
                        }
                    }
                    OnDelete::Restrict => restricted.push((key, child)),
                    OnDelete::SetNull => nulled.push((key, child.reference)),
                }
            });
        }

        if let Some((_, child)) = restricted.iter().find(|(key, _)| !doomed.contains(key)) {
            return Err(Error::Restrict {
                relation: child.relation,
                column: child.column,
                references: child.references,
            });
        }

        for ((row_type, raw), reference) in nulled {
            if !doomed.contains(&(row_type, raw)) {
                self.set_null_checked(row_type, raw, reference);
            }
        }
        for (row_type, raw) in doomed {
            self.remove_checked(row_type, raw);
        }
        Ok(())
    }
}
