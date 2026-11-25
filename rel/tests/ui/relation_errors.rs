use rel::{Id, Relation};

#[derive(Relation)]
struct User {
    name: String,
}

#[derive(Relation)]
struct UnknownIndexOption {
    #[index(sorted)]
    a: u32,
}

#[derive(Relation)]
struct ColumnsOnAField {
    #[index(columns(a))]
    a: u32,
}

#[derive(Relation)]
#[index]
struct StructIndexWithoutColumns {
    a: u32,
}

#[derive(Relation)]
#[index(columns(a))]
struct StructIndexWithOneColumn {
    a: u32,
}

#[derive(Relation)]
#[index(columns(a, nope))]
struct StructIndexWithUnknownColumn {
    a: u32,
}

#[derive(Relation)]
#[index(columns(a::b, c))]
struct StructIndexWithAPath {
    a: u32,
    c: u32,
}

#[derive(Relation)]
#[index(columns = a)]
struct StructIndexColumnsWithoutParens {
    a: u32,
}

#[derive(Relation)]
struct OnDeleteWithoutValue {
    #[references(User, on_delete)]
    user: Id<User>,
}

#[derive(Relation)]
struct OnDeleteAsString {
    #[references(User, on_delete = "cascade")]
    user: Id<User>,
}

#[derive(Relation)]
#[relation(key)]
struct KeyWithoutType {
    a: u32,
}

#[derive(Relation)]
#[relation(key = 1)]
struct KeyNotAType {
    a: u32,
}

#[derive(Relation)]
#[relation(columns)]
struct ColumnsModuleWithoutName {
    a: u32,
}

#[derive(Relation)]
#[relation(columns = "cols")]
struct ColumnsModuleAsString {
    a: u32,
}

#[derive(Relation)]
struct UnknownOnDelete {
    #[references(User, on_delete = sometimes)]
    user: Id<User>,
}

#[derive(Relation)]
struct TwoParents {
    #[references(User, Post)]
    user: Id<User>,
}

#[derive(Relation)]
struct NoParent {
    #[references(on_delete = cascade)]
    user: Id<User>,
}

#[derive(Relation)]
#[relation(table = users)]
struct UnknownRelationOption {
    a: u32,
}

#[derive(Relation)]
struct Generic<T> {
    a: T,
}

#[derive(Relation)]
enum NotAStruct {
    A,
}

#[derive(Relation)]
struct TupleStruct(u32);

mod bare {
    pub struct Option;
}

mod lifetime {
    pub struct Option<'a>(pub &'a ());
}

// Named `Option`, but not `Option<T>`.
#[derive(Relation)]
struct SetNullOnBareOption {
    #[references(User, on_delete = set_null)]
    user: bare::Option,
}

#[derive(Relation)]
struct SetNullOnATuple {
    #[references(User, on_delete = set_null)]
    user: (Id<User>,),
}

#[derive(Relation)]
struct SetNullOnOptionOfLifetime {
    #[references(User, on_delete = set_null)]
    user: lifetime::Option<'static>,
}

fn main() {}
