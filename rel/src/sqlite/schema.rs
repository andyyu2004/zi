use std::fmt::Write;

use serde::{Deserialize, Serialize};

use super::{Error, Result};

pub(crate) const ROW_ID: &str = "__rel_id";
pub(crate) const SLOTS: &str = "__rel_slots";

/// A serializable description of the snapshot tables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schema {
    pub tables: Vec<Table>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub foreign_keys: Vec<ForeignKey>,
    pub indexes: Vec<Index>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Column {
    pub name: String,
    pub sql_type: String,
    pub nullable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKey {
    pub column: String,
    pub table: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub columns: Vec<String>,
    pub unique: bool,
}

pub(crate) fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

impl Schema {
    /// Generate SQL to initialize a fresh `SQLite` database for snapshots.
    pub fn initial_migration(&self) -> String {
        let mut sql = format!(
            "CREATE TABLE {SLOTS} (table_name TEXT NOT NULL, position INTEGER NOT NULL, \
             row_id INTEGER NOT NULL, PRIMARY KEY (table_name, position));\n"
        );
        for table in &self.tables {
            table.create(&mut sql);
        }
        sql
    }

    /// Generate SQL for added tables, nullable columns, and indexes.
    ///
    /// Existing columns, foreign keys, and indexes must keep their definitions.
    /// Use a hand-written migration for other changes, then record the new schema.
    pub fn migration_from(&self, previous: &Self) -> Result<String> {
        let mut sql = String::new();
        for old in &previous.tables {
            if !self.tables.iter().any(|table| table.name == old.name) {
                return Err(Error::UnsupportedMigration(format!("removed table {}", old.name)));
            }
        }
        for table in &self.tables {
            if let Some(old) = previous.tables.iter().find(|old| old.name == table.name) {
                table.migrate(old, &mut sql)?;
            } else {
                table.create(&mut sql);
            }
        }
        Ok(sql)
    }
}

impl Column {
    fn definition(&self) -> String {
        let constraint = if self.nullable { "" } else { " NOT NULL" };
        let name = ident(&self.name);
        if self.sql_type.is_empty() {
            format!("{name}{constraint}")
        } else {
            format!("{name} {}{constraint}", self.sql_type)
        }
    }
}

impl Table {
    fn create(&self, sql: &mut String) {
        let mut definitions = vec![format!("{} INTEGER PRIMARY KEY", ident(ROW_ID))];
        definitions.extend(self.columns.iter().map(Column::definition));
        definitions.extend(self.foreign_keys.iter().map(|key| {
            format!(
                "FOREIGN KEY ({}) REFERENCES {} ({}) DEFERRABLE INITIALLY DEFERRED",
                ident(&key.column),
                ident(&key.table),
                ident(ROW_ID),
            )
        }));
        writeln!(sql, "CREATE TABLE {} ({});", ident(&self.name), definitions.join(", ")).unwrap();
        for index in &self.indexes {
            self.create_index(index, sql);
        }
    }

    fn create_index(&self, index: &Index, sql: &mut String) {
        let unique = if index.unique { "UNIQUE " } else { "" };
        // Encode each name with its length to keep the generated names distinct.
        let mut name = format!("__rel_index_{}_{}", self.name.len(), self.name);
        let mut columns = Vec::new();
        for column in &index.columns {
            write!(name, "_{}_{column}", column.len()).unwrap();
            let quoted = ident(column);
            // Rust treats two None values as equal. SQLite UNIQUE allows multiple
            // NULLs, so include a null flag and a replacement value for that case.
            if index.unique && self.columns.iter().any(|c| c.name == *column && c.nullable) {
                columns.push(format!("({quoted} IS NULL), coalesce({quoted}, 0)"));
            } else {
                columns.push(quoted);
            }
        }
        writeln!(
            sql,
            "CREATE {unique}INDEX {} ON {} ({});",
            ident(&name),
            ident(&self.name),
            columns.join(", "),
        )
        .unwrap();
    }

    fn migrate(&self, old: &Self, sql: &mut String) -> Result<()> {
        let unsupported =
            |change: &str| Error::UnsupportedMigration(format!("{change} in table {}", self.name));
        if old.foreign_keys.len() != self.foreign_keys.len()
            || !old.foreign_keys.iter().all(|key| self.foreign_keys.contains(key))
        {
            return Err(unsupported("changed foreign keys"));
        }
        for column in &old.columns {
            if !self.columns.contains(column) {
                return Err(unsupported(&format!("changed or removed column {}", column.name)));
            }
        }
        if !old.indexes.iter().all(|index| self.indexes.contains(index)) {
            return Err(unsupported("changed or removed indexes"));
        }
        for column in &self.columns {
            if old.columns.contains(column) {
                continue;
            }
            if !column.nullable {
                return Err(unsupported(&format!("added required column {}", column.name)));
            }
            writeln!(sql, "ALTER TABLE {} ADD COLUMN {};", ident(&self.name), column.definition())
                .unwrap();
        }
        for index in &self.indexes {
            if !old.indexes.contains(index) {
                self.create_index(index, sql);
            }
        }
        Ok(())
    }
}
