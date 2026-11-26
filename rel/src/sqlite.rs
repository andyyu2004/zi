//! `SQLite` snapshots for an in-memory database, enabled by the `rusqlite` feature.
//! Ordinary fields use the conversions supplied by rusqlite or by their types.
//! Generated columns leave storage affinity unspecified so those conversions
//! determine how values are stored. rel encodes primary and foreign keys.
//! Generate and apply [`Schema::initial_migration`] before saving the first snapshot.
//! [Schema] can be serialized and checked into source control for migration generation.
//! SQL foreign keys enforce references. Run application deletes through [Database]
//! to apply rel's cascade, restrict, and set-null semantics.
//!
//! ```
//! use rel::{Database, Relation, sqlite::{self, rusqlite::Connection}};
//!
//! #[derive(Relation)]
//! struct Note {
//!     text: String,
//! }
//!
//! #[derive(Database, Default)]
//! struct Notes {
//!     notes: Relation<Note>,
//! }
//!
//! # fn main() -> rel::sqlite::Result<()> {
//! let mut conn = Connection::open_in_memory()?;
//! let tx = conn.transaction()?;
//! tx.execute_batch(&sqlite::schema::<Notes>().initial_migration())?;
//! tx.commit()?;
//!
//! let mut notes = Notes::default();
//! let id = notes.insert(Note { text: "hello".into() })?;
//! notes.save(&mut conn)?;
//! let notes = Notes::load(&mut conn)?;
//! assert_eq!(notes.get(id).unwrap().text, "hello");
//! # Ok(())
//! # }
//! ```

mod schema;

pub use rusqlite;
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
pub use schema::{Column, ForeignKey, Index, Schema, Table};

use crate::{Database, Id, Key, RawId, Relation, Row};

/// A snapshot or schema error.
#[derive(Debug)]
pub enum Error {
    Sql(rusqlite::Error),
    Constraint(crate::Error),
    InvalidSlots(String),
    UnsupportedMigration(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sql(error) => error.fmt(f),
            Self::Constraint(error) => error.fmt(f),
            Self::InvalidSlots(table) => write!(f, "invalid slot metadata for {table}"),
            Self::UnsupportedMigration(change) => write!(f, "migration needs custom SQL: {change}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sql(error) => Some(error),
            Self::Constraint(error) => Some(error),
            Self::InvalidSlots(_) | Self::UnsupportedMigration(_) => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sql(error)
    }
}

impl From<crate::Error> for Error {
    fn from(error: crate::Error) -> Self {
        Self::Constraint(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Generate a snapshot schema for a database.
pub fn schema<D: SnapshotSchema<Sqlite>>() -> Schema {
    Schema { tables: D::tables() }
}

// The derives implement these traits for a generic conversion type, so field
// bounds are checked when save/load selects Sqlite, even for concrete rows.
#[doc(hidden)]
pub struct Sqlite;

#[doc(hidden)]
pub trait Bind<T> {
    fn bind(stmt: &mut rusqlite::Statement<'_>, column: usize, value: &T) -> rusqlite::Result<()>;
}

impl<T: ToSql> Bind<T> for Sqlite {
    fn bind(stmt: &mut rusqlite::Statement<'_>, column: usize, value: &T) -> rusqlite::Result<()> {
        stmt.raw_bind_parameter(column, value)
    }
}

#[doc(hidden)]
pub trait Decode<T> {
    fn decode(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<T>;
}

impl<T: FromSql> Decode<T> for Sqlite {
    fn decode(row: &rusqlite::Row<'_>, column: usize) -> rusqlite::Result<T> {
        row.get(column)
    }
}

#[doc(hidden)]
pub struct SqlKey<K>(pub K);

fn pack_id(id: RawId) -> i64 {
    (i64::from(id.index()) << 32) | i64::from(id.generation())
}

impl<K: Key> ToSql for SqlKey<K> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(pack_id(self.0.raw()).into())
    }
}

impl<K: Key> FromSql for SqlKey<K> {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let id = i64::column_result(value)?;
        Ok(Self(K::from_raw(RawId::new((id >> 32) as u32, id as u32))))
    }
}

impl<T: 'static> ToSql for Id<T> {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(pack_id(self.raw()).into())
    }
}

impl<T: 'static> FromSql for Id<T> {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        SqlKey::column_result(value).map(|key| key.0)
    }
}

#[doc(hidden)]
pub trait RowSchema<C>: Row {
    fn table() -> Table;
}

#[doc(hidden)]
pub trait WriteRow<C>: RowSchema<C> {
    fn bind(&self, stmt: &mut rusqlite::Statement<'_>) -> rusqlite::Result<()>;
}

#[doc(hidden)]
pub trait ReadRow<C>: RowSchema<C> {
    fn decode(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self>;
}

#[doc(hidden)]
pub trait SnapshotSchema<C>: Database {
    fn tables() -> Vec<Table>;
}

#[doc(hidden)]
pub trait WriteSnapshot<C>: Database {
    fn write_snapshot(&self, tx: &rusqlite::Transaction<'_>) -> Result<()>;
}

#[doc(hidden)]
pub trait ReadSnapshot<C>: Database + Sized {
    fn read_snapshot(self, tx: &rusqlite::Transaction<'_>) -> Result<Self>;
}

use schema::{ROW_ID, SLOTS, ident};

impl<T: Row> Relation<T> {
    #[doc(hidden)]
    pub fn write_snapshot<C>(&self, tx: &rusqlite::Transaction<'_>) -> Result<()>
    where
        T: WriteRow<C>,
    {
        let table = T::table();
        let name = ident(&table.name);
        tx.execute(&format!("DELETE FROM {name}"), [])?;
        tx.execute(&format!("DELETE FROM {SLOTS} WHERE table_name = ?"), [&table.name])?;

        let columns = std::iter::once(ident(ROW_ID))
            .chain(table.columns.iter().map(|column| ident(&column.name)))
            .collect::<Vec<_>>();
        let placeholders = vec!["?"; columns.len()].join(", ");
        let mut insert = tx.prepare(&format!(
            "INSERT INTO {name} ({}) VALUES ({placeholders})",
            columns.join(", "),
        ))?;
        for row in self.iter() {
            insert.raw_bind_parameter(1, SqlKey(row.id()))?;
            row.row().bind(&mut insert)?;
            insert.raw_execute()?;
        }

        let mut insert = tx.prepare(&format!(
            "INSERT INTO {SLOTS} (table_name, position, row_id) VALUES (?, ?, ?)"
        ))?;
        for (position, raw) in self.vacant_slots().enumerate() {
            let id = SqlKey(T::Key::from_raw(raw));
            insert.execute(rusqlite::params![table.name, position as i64, id])?;
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn read_snapshot<C>(tx: &rusqlite::Transaction<'_>) -> Result<Self>
    where
        T: ReadRow<C>,
    {
        let table = T::table();
        let columns = std::iter::once(ident(ROW_ID))
            .chain(table.columns.iter().map(|column| ident(&column.name)))
            .collect::<Vec<_>>()
            .join(", ");
        let mut select = tx.prepare(&format!("SELECT {columns} FROM {}", ident(&table.name)))?;
        let live = select.query_map([], |row| {
            Ok((row.get::<_, SqlKey<T::Key>>(0)?.0.raw(), Some(T::decode(row)?)))
        })?;
        let mut slots = live.collect::<rusqlite::Result<Vec<_>>>()?;

        let mut select = tx.prepare(&format!(
            "SELECT row_id, position FROM {SLOTS} WHERE table_name = ? ORDER BY position"
        ))?;
        let mut rows = select.query([&table.name])?;
        let mut free = Vec::new();
        while let Some(row) = rows.next()? {
            let id = row.get::<_, SqlKey<T::Key>>(0)?.0.raw();
            if row.get::<_, i64>(1)? != free.len() as i64 {
                return Err(Error::InvalidSlots(table.name));
            }
            free.push(id.index());
            slots.push((id, None));
        }

        slots.sort_unstable_by_key(|(id, _)| id.index());
        if slots.iter().enumerate().any(|(index, (id, _))| index != id.index() as usize) {
            return Err(Error::InvalidSlots(table.name));
        }
        Ok(Self::from_snapshot(slots, free)?)
    }
}
