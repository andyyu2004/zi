use rel::{Id, Relation};

#[derive(Relation)]
struct User {
    name: String,
}

#[derive(Relation)]
struct Post {
    #[references(User, on_delete = cascade)]
    author: Id<User>,
}

#[derive(rel::Database, Default)]
struct Db {
    posts: Relation<Post>,
}

fn main() {}
