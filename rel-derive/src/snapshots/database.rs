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
impl ::rel::Database for Blog {
    fn visit_children(
        &self,
        parent_type: ::std::any::TypeId,
        parent: ::rel::RawId,
        f: &mut dyn ::std::ops::FnMut(::rel::Child),
    ) {
        self.users.visit_children(parent_type, parent, f);
        self.posts.visit_children(parent_type, parent, f);
    }
    fn remove_checked(&mut self, row_type: ::std::any::TypeId, id: ::rel::RawId) {
        if row_type == ::std::any::TypeId::of::<User>() {
            self.users.remove_checked(id);
            return;
        }
        if row_type == ::std::any::TypeId::of::<Post>() {
            self.posts.remove_checked(id);
            return;
        }
        ::std::unreachable!("deleted rows come from this database's relations")
    }
    fn set_null_checked(
        &mut self,
        row_type: ::std::any::TypeId,
        id: ::rel::RawId,
        reference: usize,
    ) {
        if row_type == ::std::any::TypeId::of::<User>() {
            self.users.set_null_checked(id, reference);
            return;
        }
        if row_type == ::std::any::TypeId::of::<Post>() {
            self.posts.set_null_checked(id, reference);
            return;
        }
        ::std::unreachable!("nulled rows come from this database's relations")
    }
}
impl ::rel::KeyIn<Blog> for <User as ::rel::Row>::Key {
    type Row = User;
}
impl ::rel::KeyIn<Blog> for <Post as ::rel::Row>::Key {
    type Row = Post;
}
impl ::rel::HasRelation<User> for Blog {
    fn relation(&self) -> &::rel::Relation<User> {
        &self.users
    }
    fn relation_mut(&mut self) -> &mut ::rel::Relation<User> {
        &mut self.users
    }
}
impl ::rel::HasRelation<Post> for Blog {
    fn relation(&self) -> &::rel::Relation<Post> {
        &self.posts
    }
    fn relation_mut(&mut self) -> &mut ::rel::Relation<Post> {
        &mut self.posts
    }
}
