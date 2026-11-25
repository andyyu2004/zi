use rel::Database;

#[derive(Database)]
enum NotAStruct {
    A,
}

#[derive(Database)]
struct TupleStruct(u32);

fn main() {}
