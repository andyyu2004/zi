use expect_test::expect_file;
use syn::{File, parse_quote};

use super::{database, relation};

#[test]
fn relation_expansion() {
    let post = parse_quote! {
        struct Post {
            #[references(User, on_delete = cascade)]
            author: Id<User>,
            title: String,
            #[index(ordered)]
            published: Option<u64>,
        }
    };

    let expanded: File = syn::parse2(relation(&post, true).unwrap()).unwrap();
    expect_file!["snapshots/relation.rs"].assert_eq(&prettyplease::unparse(&expanded));
}

#[test]
fn database_expansion() {
    let blog = parse_quote! {
        struct Blog {
            users: Relation<User>,
            posts: Relation<Post>,
        }
    };

    let expanded: File = syn::parse2(database(&blog, true).unwrap()).unwrap();
    expect_file!["snapshots/database.rs"].assert_eq(&prettyplease::unparse(&expanded));
}
