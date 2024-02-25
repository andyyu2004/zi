#[macro_export]
macro_rules! hashmap {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$(hashmap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { hashmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let cap = hashmap!(@count $($key),*);
            let mut map = ::std::collections::HashMap::with_capacity_and_hasher(cap, ::std::hash::BuildHasherDefault::<::rustc_hash::FxHasher>::default());
            $(
                let _ = map.insert($key, $value);
            )*
            map
        }
    };
}

#[macro_export]
macro_rules! trie {
    (@trie $value:ident) => {
        $crate::keymap::TrieNode::Value($value)
    };

    (@trie
        { $($($key:literal)|+ => $value:tt,)+ }
    ) => {
        trie!({ $($($key)|+ => $value,)+ })
    };

    (
        { $($($key:literal)|+ => $value:tt,)+ }
    ) => {
        {
            let cap = hashmap!(@count $($($key),+),*);
            let mut map = ::std::collections::HashMap::with_capacity_and_hasher(cap, ::std::hash::BuildHasherDefault::<rustc_hash::FxHasher>::default());
            $(
                $(
                    let key = $key.parse::<$crate::event::KeyEvent>().unwrap();
                    let duplicate = map.insert(
                        key,
                        trie!(@trie $value)
                    );
                    assert!(duplicate.is_none(), "Duplicate key found");
                )+
            )*
            let trie = $crate::keymap::Trie::new(map);
            $crate::keymap::TrieNode::Trie(trie)
        }
    };
}
