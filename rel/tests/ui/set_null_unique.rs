use rel::{Id, Relation};

#[derive(Relation)]
struct Group {
    url: String,
}

#[derive(Relation)]
struct View {
    #[index(unique)]
    #[references(Group, on_delete = set_null)]
    group: Option<Id<Group>>,
}

fn main() {}
