use rel::{Database, Error, Id, Join, Relation};

#[derive(Relation, Clone, Debug)]
struct User {
    #[index(unique)]
    email: String,
    name: String,
}

#[derive(Relation, Clone, Debug)]
struct Post {
    #[references(User, on_delete = cascade)]
    author: Id<User>,
    title: String,
    #[index(ordered)]
    published: Option<u64>,
}

#[derive(Relation, Clone, Debug)]
struct Comment {
    #[references(Post, on_delete = cascade)]
    post: Id<Post>,
    #[references(User, on_delete = restrict)]
    author: Id<User>,
    body: String,
}

#[derive(Relation, Clone, Debug)]
#[index(unique, columns(post, user))]
struct Like {
    #[references(Post, on_delete = cascade)]
    post: Id<Post>,
    #[references(User, on_delete = cascade)]
    user: Id<User>,
}

#[derive(Database, Default)]
struct Blog {
    users: Relation<User>,
    posts: Relation<Post>,
    comments: Relation<Comment>,
    likes: Relation<Like>,
}

fn user(db: &mut Blog, name: &str) -> Id<User> {
    db.insert(User { email: format!("{name}@x"), name: name.into() }).unwrap()
}

#[test]
fn draft_example() -> rel::Result<()> {
    let mut db = Blog::default();

    let alice = user(&mut db, "alice");
    let bob = user(&mut db, "bob");
    let post = db.insert(Post { author: alice, title: "Hello".into(), published: Some(10) })?;
    db.insert(Comment { post, author: bob, body: "hi".into() })?;
    db.insert(Like { post, user: bob })?;

    assert_eq!(
        db.insert(Like { post, user: bob }).unwrap_err(),
        Error::Unique { relation: "Like", index: "post_and_user" }
    );

    assert_eq!(db.users[alice].name, "alice");
    assert_eq!(db.users.by_email("alice@x").map(|u| u.id()), Some(alice));
    assert_eq!(db.posts.by_published(Some(5)..).count(), 1);
    assert_eq!(db.posts.by_published(Some(11)..).count(), 0);
    assert!(db.likes.by_post_and_user(&post, &bob).is_some());

    db.update(post, |p| p.title = "Hello, world".into())?;
    assert_eq!(db.posts[post].title, "Hello, world");

    assert_eq!(db.posts.find(post::author, alice).count(), 1);
    assert_eq!(db.posts.find(post::title, "Hello, world".to_string()).count(), 1);

    let joined: Vec<_> = db
        .posts
        .by_author(&alice)
        .join(&db.comments, comment::post)
        .map(|(post, comment)| {
            (post.title.clone(), db.users[comment.author].name.clone(), comment.body.clone())
        })
        .collect();
    assert_eq!(joined, [("Hello, world".to_string(), "bob".to_string(), "hi".to_string())]);

    assert_eq!(
        db.delete(bob).unwrap_err(),
        Error::Restrict { relation: "Comment", column: "author", references: "User" }
    );
    assert!(db.users.contains(bob), "a refused delete removes nothing");
    assert_eq!(db.likes.len(), 1, "a refused delete removes nothing");

    db.delete(alice)?;
    assert!(db.posts.is_empty());
    assert!(db.comments.is_empty());
    assert!(db.likes.is_empty());
    assert_eq!(db.users.len(), 1);
    Ok(())
}

#[test]
fn foreign_key_must_exist() {
    let mut db = Blog::default();
    let alice = user(&mut db, "alice");
    db.delete(alice).unwrap();
    assert_eq!(
        db.insert(Post { author: alice, title: "x".into(), published: None }).unwrap_err(),
        Error::ForeignKey { relation: "Post", column: "author", references: "User" }
    );
}

#[test]
fn stale_id_does_not_alias_a_new_row() {
    let mut db = Blog::default();
    let alice = user(&mut db, "alice");
    db.delete(alice).unwrap();
    let bob = user(&mut db, "bob");
    assert_eq!(alice.raw().index(), bob.raw().index(), "the slot is reused");
    assert!(db.users.get(alice).is_none());
    assert_eq!(
        db.update(alice, |u| u.name = "x".into()),
        Err(Error::NotFound { relation: "User" })
    );
}

#[test]
fn failed_update_leaves_row_and_indexes_unchanged() {
    let mut db = Blog::default();
    let alice = user(&mut db, "alice");
    user(&mut db, "bob");

    let err = db.update(alice, |u| {
        u.email = "bob@x".into();
        u.name = "changed".into();
    });
    assert_eq!(err, Err(Error::Unique { relation: "User", index: "email" }));
    assert_eq!(db.users[alice].name, "alice");
    assert_eq!(db.users.by_email("alice@x").map(|u| u.id()), Some(alice));
}

#[test]
fn update_moves_index_entries() {
    let mut db = Blog::default();
    let alice = user(&mut db, "alice");
    let bob = user(&mut db, "bob");
    let post = db.insert(Post { author: alice, title: "x".into(), published: Some(1) }).unwrap();

    db.update(post, |p| {
        p.author = bob;
        p.published = Some(7);
    })
    .unwrap();
    assert_eq!(db.posts.by_author(&alice).count(), 0);
    assert_eq!(db.posts.by_author(&bob).count(), 1);
    assert_eq!(db.posts.by_published(Some(1)..Some(2)).count(), 0);
    assert_eq!(db.posts.by_published(Some(7)..=Some(7)).count(), 1);

    db.delete(alice).unwrap();
    assert!(
        db.posts.contains(post),
        "the post moved to bob, so deleting alice doesn't cascade to it"
    );
}

#[test]
fn restrict_is_ignored_for_rows_the_cascade_removes() {
    let mut db = Blog::default();
    let alice = user(&mut db, "alice");
    let post = db.insert(Post { author: alice, title: "x".into(), published: None }).unwrap();
    // Alice commented on her own post: the comment restricts deleting alice, but
    // deleting alice cascades to the post, and from there to the comment.
    db.insert(Comment { post, author: alice, body: "me".into() }).unwrap();

    db.delete(alice).unwrap();
    assert!(db.comments.is_empty());
}

#[test]
fn find_range_without_an_ordered_index_scans() {
    let mut db = Blog::default();
    for name in ["a", "b", "c"] {
        user(&mut db, name);
    }
    let names: Vec<_> =
        db.users.find_range(user::name, "b".to_string()..).map(|u| u.name.clone()).collect();
    assert_eq!(names, ["b", "c"]);
}

#[cfg(feature = "rusqlite")]
#[test]
fn sqlite_snapshot_round_trip() -> rel::sqlite::Result<()> {
    use rel::sqlite;
    use rel::sqlite::rusqlite::Connection;

    let mut conn = Connection::open_in_memory()?;
    let schema = sqlite::schema::<Blog>();
    let serialized = serde_json::to_string(&schema).unwrap();
    assert_eq!(serde_json::from_str::<rel::sqlite::Schema>(&serialized).unwrap(), schema);
    conn.execute_batch(&schema.initial_migration())?;

    let mut db = Blog::default();
    let alice = user(&mut db, "alice");
    let stale = user(&mut db, "stale");
    let last = user(&mut db, "last");
    let post =
        db.insert(Post { author: alice, title: "hello".into(), published: Some(i64::MAX as u64) })?;
    let like = db.insert(Like { post, user: alice })?;
    db.delete(last)?;
    db.delete(stale)?;
    db.save(&mut conn)?;

    let mut loaded = Blog::load(&mut conn)?;
    assert_eq!(loaded.users.by_email("alice@x").unwrap().id(), alice);
    assert_eq!(loaded.posts.by_published(Some(i64::MAX as u64)..).next().unwrap().id(), post);
    assert_eq!(loaded.likes.by_post_and_user(&post, &alice).unwrap().id(), like);
    let reopened = user(&mut loaded, "new");
    assert_eq!(reopened, user(&mut db, "new"));
    assert!(loaded.get(stale).is_none());
    assert!(loaded.get(last).is_none());
    loaded.save(&mut conn)?;
    loaded.delete(alice)?;
    assert!(loaded.posts.is_empty());
    assert!(loaded.likes.is_empty());
    loaded.save(&mut conn)?;
    let latest = Blog::load(&mut conn)?;
    assert!(latest.get(post).is_none());
    assert_eq!(latest.get(reopened).unwrap().name, "new");
    Ok(())
}

#[cfg(feature = "rusqlite")]
#[test]
fn sqlite_generates_additive_migration() -> rel::sqlite::Result<()> {
    use rel::sqlite::rusqlite::Connection;
    use rel::sqlite::{self, Column, Index, Table};

    let mut conn = Connection::open_in_memory()?;
    let original = sqlite::schema::<Blog>();
    conn.execute_batch(&original.initial_migration())?;
    let mut db = Blog::default();
    let alice = user(&mut db, "alice");
    db.save(&mut conn)?;

    let mut updated = original.clone();
    let user = updated.tables.iter_mut().find(|table| table.name == "User").unwrap();
    user.columns.push(Column { name: "bio".into(), sql_type: "TEXT".into(), nullable: true });
    user.indexes.push(Index { columns: vec!["bio".into()], unique: false });
    updated.tables.push(Table {
        name: "Tag".into(),
        columns: Vec::new(),
        foreign_keys: Vec::new(),
        indexes: Vec::new(),
    });
    let tx = conn.transaction()?;
    tx.execute_batch(&updated.migration_from(&original)?)?;
    tx.commit()?;
    assert_eq!(
        conn.query_row("SELECT bio FROM User", [], |row| row.get::<_, Option<String>>(0))?,
        None
    );
    assert_eq!(conn.query_row("SELECT count(*) FROM Tag", [], |row| row.get::<_, i64>(0))?, 0);
    assert_eq!(Blog::load(&mut conn)?.get(alice).unwrap().name, "alice");
    assert!(original.migration_from(&updated).is_err());
    Ok(())
}

#[cfg(feature = "rusqlite")]
#[test]
fn sqlite_failed_save_preserves_previous_snapshot() -> rel::sqlite::Result<()> {
    use rel::sqlite;
    use rel::sqlite::rusqlite::Connection;

    let mut conn = Connection::open_in_memory()?;
    conn.execute_batch(&sqlite::schema::<Blog>().initial_migration())?;
    let mut db = Blog::default();
    let alice = user(&mut db, "alice");
    db.save(&mut conn)?;
    let bob = user(&mut db, "bob");
    db.insert(Post { author: bob, title: "hello".into(), published: None })?;
    conn.execute_batch(
        "CREATE TRIGGER reject_post BEFORE INSERT ON Post
         BEGIN SELECT RAISE(ABORT, 'post rejected'); END;",
    )?;
    assert!(db.save(&mut conn).is_err());
    let loaded = Blog::load(&mut conn)?;
    assert_eq!(loaded.get(alice).unwrap().name, "alice");
    assert!(loaded.get(bob).is_none());
    assert!(loaded.posts.is_empty());
    Ok(())
}
