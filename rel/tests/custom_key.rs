use rel::{Database, Id, Key, RawId, Relation};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
struct BufferId(u64);

impl Key for BufferId {
    fn from_raw(raw: RawId) -> Self {
        BufferId((u64::from(raw.generation()) << 32) | u64::from(raw.index()))
    }

    fn raw(self) -> RawId {
        RawId::new(self.0 as u32, (self.0 >> 32) as u32)
    }
}

#[derive(Relation)]
#[relation(key = BufferId)]
struct Buffer {
    id: BufferId,
    text: String,
}

#[derive(Relation)]
struct View {
    #[references(Buffer, on_delete = cascade)]
    buffer: BufferId,
}

#[derive(Database, Default)]
struct Editor {
    buffers: Relation<Buffer>,
    views: Relation<View>,
}

#[test]
fn custom_key_type() {
    let mut db = Editor::default();
    let a: BufferId = db.insert_with(|id| Buffer { id, text: "a".into() }).unwrap();
    let view: Id<View> = db.insert(View { buffer: a }).unwrap();

    assert_eq!(db.buffers[a].id, a);
    assert_eq!(db.get(a).map(|b| b.text.as_str()), Some("a"));
    assert_eq!(db.views.by_buffer(&a).map(|v| v.id()).collect::<Vec<_>>(), [view]);

    db.delete(a).unwrap();
    assert!(db.views.is_empty());
    assert!(db.get(a).is_none());
}

#[cfg(feature = "rusqlite")]
mod sqlite {
    use rel::sqlite::rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};
    use rel::sqlite::rusqlite::{self, ToSql};

    use super::*;

    impl ToSql for BufferId {
        fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
            Ok(self.0.to_be_bytes().to_vec().into())
        }
    }

    impl FromSql for BufferId {
        fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
            <[u8; 8]>::column_result(value).map(|bytes| Self(u64::from_be_bytes(bytes)))
        }
    }

    #[test]
    fn round_trip_custom_key() -> rel::sqlite::Result<()> {
        let mut conn = rusqlite::Connection::open_in_memory()?;
        conn.execute_batch(&rel::sqlite::schema::<Editor>().initial_migration())?;
        let mut db = Editor::default();
        let stale = db.insert_with(|id| Buffer { id, text: String::new() })?;
        db.delete(stale)?;
        let buffer = db.insert_with(|id| Buffer { id, text: "a".into() })?;
        let view = db.insert(View { buffer })?;
        db.save(&mut conn)?;

        let loaded = Editor::load(&mut conn)?;
        assert_eq!(loaded.get(buffer).unwrap().id, buffer);
        assert_eq!(loaded.get(view).unwrap().buffer, buffer);
        assert!(loaded.get(stale).is_none());
        Ok(())
    }
}

#[cfg(feature = "rusqlite")]
#[test]
fn save_and_load_require_only_their_own_conversion() -> rel::sqlite::Result<()> {
    use rel::sqlite::rusqlite;

    mod write {
        use rel::sqlite::rusqlite::types::ToSqlOutput;
        use rel::sqlite::rusqlite::{self, ToSql};

        use super::*;

        struct WriteOnly(i64);

        impl ToSql for WriteOnly {
            fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
                self.0.to_sql()
            }
        }

        #[derive(Relation)]
        pub struct Item {
            value: WriteOnly,
        }

        #[derive(Database, Default)]
        pub struct Db {
            items: Relation<Item>,
        }

        pub fn save(conn: &mut rusqlite::Connection) -> rel::sqlite::Result<()> {
            let mut db = Db::default();
            db.insert(Item { value: WriteOnly(42) })?;
            db.save(conn)
        }
    }

    mod read {
        use rel::sqlite::rusqlite::types::{FromSql, FromSqlResult, ValueRef};
        use rel::sqlite::rusqlite::{self};

        use super::*;

        struct ReadOnly(i64);

        impl FromSql for ReadOnly {
            fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
                i64::column_result(value).map(Self)
            }
        }

        #[derive(Relation)]
        struct Item {
            value: ReadOnly,
        }

        #[derive(Database, Default)]
        struct Db {
            items: Relation<Item>,
        }

        pub fn load(conn: &mut rusqlite::Connection) -> rel::sqlite::Result<i64> {
            Ok(Db::load(conn)?.items.iter().next().unwrap().value.0)
        }
    }

    let mut conn = rusqlite::Connection::open_in_memory()?;
    conn.execute_batch(&rel::sqlite::schema::<write::Db>().initial_migration())?;
    write::save(&mut conn)?;
    assert_eq!(read::load(&mut conn)?, 42);
    Ok(())
}
