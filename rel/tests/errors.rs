use rel::{Database, Error, Id, Relation};

#[derive(Relation, Clone)]
struct User {
    #[index(unique)]
    name: String,
}

#[derive(Relation, Clone)]
struct Post {
    #[references(User, on_delete = restrict)]
    author: Id<User>,
}

#[derive(Database, Default)]
struct Blog {
    users: Relation<User>,
    posts: Relation<Post>,
}

#[test]
fn errors_display() {
    let cases = [
        (Error::NotFound { relation: "User" }, "no such User"),
        (
            Error::Unique { relation: "User", index: "name" },
            "unique index `name` on User already has this key",
        ),
        (
            Error::ForeignKey { relation: "Post", column: "author", references: "User" },
            "Post.author refers to a User that does not exist",
        ),
        (
            Error::Restrict { relation: "Post", column: "author", references: "User" },
            "cannot delete User: Post.author still refers to it",
        ),
    ];
    for (error, message) in cases {
        assert_eq!(error.to_string(), message);
        let _: &dyn std::error::Error = &error;
    }
}

#[test]
fn a_deleted_row_is_not_found() {
    let mut db = Blog::default();
    let alice = db.insert(User { name: "alice".into() }).unwrap();
    db.delete(alice).unwrap();

    let not_found = Error::NotFound { relation: "User" };
    assert!(db.get(alice).is_none());
    assert_eq!(db.delete(alice), Err(not_found.clone()));
    assert_eq!(db.update(alice, |u| u.name = "bob".into()), Err(not_found.clone()));
    assert_eq!(db.set(alice, user::name, "bob".into()), Err(not_found));
}

#[test]
fn update_checks_unique_indexes_and_foreign_keys() {
    let mut db = Blog::default();
    let alice = db.insert(User { name: "alice".into() }).unwrap();
    let bob = db.insert(User { name: "bob".into() }).unwrap();
    let post = db.insert(Post { author: alice }).unwrap();

    assert_eq!(
        db.update(bob, |u| u.name = "alice".into()),
        Err(Error::Unique { relation: "User", index: "name" })
    );
    assert_eq!(db.users[bob].name, "bob");

    db.delete(bob).unwrap();
    assert_eq!(
        db.update(post, |p| p.author = bob),
        Err(Error::ForeignKey { relation: "Post", column: "author", references: "User" })
    );
    assert_eq!(db.posts[post].author, alice);
}

#[test]
fn a_refused_delete_changes_nothing() {
    let mut db = Blog::default();
    let alice = db.insert(User { name: "alice".into() }).unwrap();
    db.insert(Post { author: alice }).unwrap();

    assert_eq!(
        db.delete(alice),
        Err(Error::Restrict { relation: "Post", column: "author", references: "User" })
    );
    assert_eq!(db.users.len(), 1);
    assert_eq!(db.posts.len(), 1);
}
