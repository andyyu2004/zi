use expect_test::expect_file;
use syn::{File, Item, parse_quote};

use super::{database, relation};

#[test]
fn sqlite_expansion() {
    let post = parse_quote! {
        struct Post {
            #[references(User, on_delete = cascade)]
            author: Id<User>,
            title: String,
            #[index(ordered)]
            published: Option<u64>,
        }
    };
    let blog = parse_quote! {
        struct Blog {
            users: Relation<User>,
            posts: Relation<Post>,
        }
    };

    let mut expanded: File = syn::parse2(relation(&post, true).unwrap()).unwrap();
    expanded.items.retain(|item| {
        let Item::Impl(item) = item else { return false };
        item.trait_.as_ref().is_some_and(|(_, path, _)| {
            path.segments.iter().nth(1).is_some_and(|segment| segment.ident == "sqlite")
        })
    });

    let expanded_database: File = syn::parse2(database(&blog, true).unwrap()).unwrap();
    // The database derive emits its snapshot impls alongside the foreign-key checks.
    expanded
        .items
        .extend(expanded_database.items.into_iter().filter(|item| matches!(item, Item::Const(_))));

    expect_file!["snapshots/sqlite.rs"].assert_eq(&prettyplease::unparse(&expanded));
}
