use rel::{Id, Relation};

#[derive(Relation)]
struct User {
    #[index(unique)]
    name: String,
}

#[derive(Relation)]
struct Post {
    #[references(User, on_delete = cascade)]
    author: Id<User>,
}

fn main() {
    let mut users = Relation::<User>::default();
    let mut posts = Relation::<Post>::default();
    // A unique index can be broken, and so can a foreign key.
    let author = users.insert(User { name: "a".into() });
    posts.insert(Post { author });
}
