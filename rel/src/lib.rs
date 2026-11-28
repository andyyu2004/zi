//! In-memory relational data: rows are plain structs, relations hold them,
//! a database groups relations and enforces keys across them.

mod database;
mod error;
mod id;
mod index;
mod query;
mod relation;

pub use database::{Child, Database, HasRelation, KeyIn, References};
pub use error::{Error, Result};
pub use id::{Id, Key, RawId};
pub use index::{HashIndex, LookupIndex, OrdIndex, UniqueHashIndex, UniqueOrdIndex};
pub use query::{Column, Join, Ref, RefersTo};
pub use rel_derive::{Database, Relation};
pub use relation::{OnDelete, Reference, Relation, Row, Unconstrained};

#[cfg(feature = "rusqlite")]
pub mod sqlite;
