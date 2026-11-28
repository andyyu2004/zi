impl<__RelSqlite> ::rel::sqlite::RowSchema<__RelSqlite> for Post {
    fn table() -> ::rel::sqlite::Table {
        ::rel::sqlite::Table {
            name: <Self as ::rel::Row>::NAME.into(),
            columns: ::std::vec![
                ::rel::sqlite::Column { name : "author".into(), sql_type : "INTEGER"
                .into(), nullable : false, }, ::rel::sqlite::Column { name : "title"
                .into(), sql_type : "".into(), nullable : false, }, ::rel::sqlite::Column
                { name : "published".into(), sql_type : "".into(), nullable : true, }
            ],
            foreign_keys: ::std::vec![
                ::rel::sqlite::ForeignKey { column : "author".into(), table : < User as
                ::rel::Row > ::NAME.into(), }
            ],
            indexes: ::std::vec![
                ::rel::sqlite::Index { columns : ::std::vec!["published".into()], unique
                : false }, ::rel::sqlite::Index { columns : ::std::vec!["author".into()],
                unique : false }
            ],
        }
    }
}
impl<__RelSqlite> ::rel::sqlite::WriteRow<__RelSqlite> for Post
where
    __RelSqlite: ::rel::sqlite::Bind<String>,
    __RelSqlite: ::rel::sqlite::Bind<Option<u64>>,
{
    fn bind(
        &self,
        stmt: &mut ::rel::sqlite::rusqlite::Statement<'_>,
    ) -> ::rel::sqlite::rusqlite::Result<()> {
        let _ = &stmt;
        stmt.raw_bind_parameter(2usize, ::rel::sqlite::SqlKey(self.author))?;
        <__RelSqlite as ::rel::sqlite::Bind<String>>::bind(stmt, 3usize, &self.title)?;
        <__RelSqlite as ::rel::sqlite::Bind<
            Option<u64>,
        >>::bind(stmt, 4usize, &self.published)?;
        ::std::result::Result::Ok(())
    }
}
impl<__RelSqlite> ::rel::sqlite::ReadRow<__RelSqlite> for Post
where
    __RelSqlite: ::rel::sqlite::Decode<String>,
    __RelSqlite: ::rel::sqlite::Decode<Option<u64>>,
{
    fn decode(
        row: &::rel::sqlite::rusqlite::Row<'_>,
    ) -> ::rel::sqlite::rusqlite::Result<Self> {
        let _ = row;
        ::std::result::Result::Ok(Self {
            author: row.get::<_, ::rel::sqlite::SqlKey<Id<User>>>(1usize)?.0,
            title: <__RelSqlite as ::rel::sqlite::Decode<String>>::decode(row, 2usize)?,
            published: <__RelSqlite as ::rel::sqlite::Decode<
                Option<u64>,
            >>::decode(row, 3usize)?,
        })
    }
}
#[doc(hidden)]
#[derive(Default)]
struct PostIndexes {
    published: ::rel::OrdIndex<Option<u64>, <Post as ::rel::Row>::Key>,
    author: ::rel::HashIndex<Id<User>, <Post as ::rel::Row>::Key>,
}
/// Columns of
#[doc = concat!("[`", "Post", "`],")]
/// for queries and `Database::set`.
#[allow(non_upper_case_globals, unused_imports)]
mod post {
    use super::*;
    pub(super) const author: ::rel::Column<Post, Id<User>> = ::rel::Column {
        name: "author",
        get: {
            fn get(row: &Post) -> &Id<User> {
                &row.author
            }
            get
        },
        get_mut: {
            fn get_mut(row: &mut Post) -> &mut Id<User> {
                &mut row.author
            }
            get_mut
        },
        index: ::std::option::Option::Some({
            fn index(
                indexes: &PostIndexes,
            ) -> &dyn ::rel::LookupIndex<Id<User>, <Post as ::rel::Row>::Key> {
                &indexes.author
            }
            index
        }),
    };
    pub(super) const title: ::rel::Column<Post, String> = ::rel::Column {
        name: "title",
        get: {
            fn get(row: &Post) -> &String {
                &row.title
            }
            get
        },
        get_mut: {
            fn get_mut(row: &mut Post) -> &mut String {
                &mut row.title
            }
            get_mut
        },
        index: ::std::option::Option::None,
    };
    pub(super) const published: ::rel::Column<Post, Option<u64>> = ::rel::Column {
        name: "published",
        get: {
            fn get(row: &Post) -> &Option<u64> {
                &row.published
            }
            get
        },
        get_mut: {
            fn get_mut(row: &mut Post) -> &mut Option<u64> {
                &mut row.published
            }
            get_mut
        },
        index: ::std::option::Option::Some({
            fn index(
                indexes: &PostIndexes,
            ) -> &dyn ::rel::LookupIndex<Option<u64>, <Post as ::rel::Row>::Key> {
                &indexes.published
            }
            index
        }),
    };
}
/// Mutable access to a row of
#[doc = concat!("[`", "Post", "`]")]
/// with its indexed and foreign key columns read-only.
#[allow(dead_code)]
struct PostMut<'a> {
    author: &'a Id<User>,
    title: &'a mut String,
    published: &'a Option<u64>,
}
impl ::rel::Row for Post {
    type Key = ::rel::Id<Post>;
    type Indexes = PostIndexes;
    type Mut<'a> = PostMut<'a>;
    fn project(&mut self) -> Self::Mut<'_> {
        PostMut {
            author: &self.author,
            title: &mut self.title,
            published: &self.published,
        }
    }
    const NAME: &'static str = "Post";
    const REFERENCES: &'static [::rel::Reference<Self>] = &[
        ::rel::Reference {
            column: "author",
            references: "User",
            parent: {
                fn parent() -> ::std::any::TypeId {
                    ::std::any::TypeId::of::<User>()
                }
                parent
            },
            on_delete: ::rel::OnDelete::Cascade,
            children: {
                fn children(
                    indexes: &PostIndexes,
                    parent: ::rel::RawId,
                    f: &mut dyn ::std::ops::FnMut(::rel::RawId),
                ) {
                    for id in ::rel::LookupIndex::lookup(
                        &indexes.author,
                        &<<User as ::rel::Row>::Key as ::rel::Key>::from_raw(parent),
                    ) {
                        f(::rel::Key::raw(id));
                    }
                }
                children
            },
            set_null: ::std::option::Option::None,
        },
    ];
    fn check_unique(&self, indexes: &Self::Indexes) -> ::rel::Result<()> {
        ::std::result::Result::Ok(())
    }
    fn index_insert(&self, indexes: &mut Self::Indexes, id: Self::Key) {
        indexes.published.insert(::std::clone::Clone::clone(&self.published), id);
        indexes.author.insert(::std::clone::Clone::clone(&self.author), id);
    }
    fn index_remove(&self, indexes: &mut Self::Indexes, id: Self::Key) {
        indexes.published.remove(&::std::clone::Clone::clone(&self.published), id);
        indexes.author.remove(&::std::clone::Clone::clone(&self.author), id);
    }
}
impl<D: ?Sized> ::rel::References<D> for Post
where
    D: ::rel::HasRelation<User>,
{
    fn check_references(&self, db: &D) -> ::rel::Result<()> {
        let _ = db;
        if !::rel::HasRelation::<User>::relation(db).contains(self.author) {
            return ::std::result::Result::Err(::rel::Error::ForeignKey {
                relation: "Post",
                column: "author",
                references: "User",
            });
        }
        ::std::result::Result::Ok(())
    }
}
/// Index lookups on a relation of
#[doc = concat!("[`", "Post", "`].")]
trait PostQueries {
    fn by_published(
        &self,
        range: impl ::std::ops::RangeBounds<Option<u64>>,
    ) -> impl ::std::iter::Iterator<Item = ::rel::Ref<'_, Post>>;
    fn by_author<Q>(
        &self,
        key: &Q,
    ) -> impl ::std::iter::Iterator<Item = ::rel::Ref<'_, Post>>
    where
        Id<User>: ::std::borrow::Borrow<Q>,
        Q: ::std::hash::Hash + ::std::cmp::Eq + ?Sized;
}
impl PostQueries for ::rel::Relation<Post> {
    fn by_published(
        &self,
        range: impl ::std::ops::RangeBounds<Option<u64>>,
    ) -> impl ::std::iter::Iterator<Item = ::rel::Ref<'_, Post>> {
        let range = (range.start_bound().cloned(), range.end_bound().cloned());
        self.indexes().published.range(range).map(|id| self.indexed(id))
    }
    fn by_author<Q>(
        &self,
        key: &Q,
    ) -> impl ::std::iter::Iterator<Item = ::rel::Ref<'_, Post>>
    where
        Id<User>: ::std::borrow::Borrow<Q>,
        Q: ::std::hash::Hash + ::std::cmp::Eq + ?Sized,
    {
        self.indexes().author.get(key).map(|id| self.indexed(id))
    }
}
