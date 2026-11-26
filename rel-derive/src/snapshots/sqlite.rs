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
const _: () = {
    fn assert_references<T: ::rel::Row + ::rel::References<Blog>>(
        db: &Blog,
        relation: &::rel::Relation<T>,
    ) -> ::rel::Result<()> {
        for row in relation.iter() {
            ::rel::References::check_references(row.row(), db)?;
        }
        ::std::result::Result::Ok(())
    }
    fn assert_all(db: &Blog) -> ::rel::Result<()> {
        let _ = db;
        assert_references::<User>(db, &db.users)?;
        assert_references::<Post>(db, &db.posts)?;
        ::std::result::Result::Ok(())
    }
    impl<__RelSqlite> ::rel::sqlite::SnapshotSchema<__RelSqlite> for Blog
    where
        User: ::rel::sqlite::RowSchema<__RelSqlite>,
        Post: ::rel::sqlite::RowSchema<__RelSqlite>,
    {
        fn tables() -> ::std::vec::Vec<::rel::sqlite::Table> {
            ::std::vec![
                < User as ::rel::sqlite::RowSchema < __RelSqlite >> ::table(), < Post as
                ::rel::sqlite::RowSchema < __RelSqlite >> ::table()
            ]
        }
    }
    impl<__RelSqlite> ::rel::sqlite::WriteSnapshot<__RelSqlite> for Blog
    where
        User: ::rel::sqlite::WriteRow<__RelSqlite>,
        Post: ::rel::sqlite::WriteRow<__RelSqlite>,
    {
        fn write_snapshot(
            &self,
            tx: &::rel::sqlite::rusqlite::Transaction<'_>,
        ) -> ::rel::sqlite::Result<()> {
            let _ = tx;
            self.users.write_snapshot::<__RelSqlite>(tx)?;
            self.posts.write_snapshot::<__RelSqlite>(tx)?;
            ::std::result::Result::Ok(())
        }
    }
    impl<__RelSqlite> ::rel::sqlite::ReadSnapshot<__RelSqlite> for Blog
    where
        User: ::rel::sqlite::ReadRow<__RelSqlite>,
        Post: ::rel::sqlite::ReadRow<__RelSqlite>,
    {
        fn read_snapshot(
            mut self,
            tx: &::rel::sqlite::rusqlite::Transaction<'_>,
        ) -> ::rel::sqlite::Result<Self> {
            let _ = tx;
            self.users = ::rel::Relation::<User>::read_snapshot::<__RelSqlite>(tx)?;
            self.posts = ::rel::Relation::<Post>::read_snapshot::<__RelSqlite>(tx)?;
            assert_all(&self)?;
            ::std::result::Result::Ok(self)
        }
    }
};
