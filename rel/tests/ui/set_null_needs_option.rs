use rel::{Id, Relation};

#[derive(Relation)]
struct Group {
    url: String,
}

#[derive(Relation)]
struct View {
    #[references(Group, on_delete = set_null)]
    group: Id<Group>,
}

fn main() {}
