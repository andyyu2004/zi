#[macro_export]
macro_rules! hashmap {
    (@single $($x:tt)*) => (());
    (@count $($rest:expr),*) => (<[()]>::len(&[$($crate::hashmap!(@single $rest)),*]));

    ($($key:expr => $value:expr,)+) => { $crate::hashmap!($($key => $value),+) };
    ($($key:expr => $value:expr),*) => {
        {
            let cap = $crate::hashmap!(@count $($key),*);
            let mut map = ::std::collections::HashMap::with_capacity(cap);
            $(
                let _ = map.insert($key, $value);
            )*
            map
        }
    };
}

#[macro_export]
macro_rules! trie {
    (@trie $value:path) => {
        $crate::keymap::TrieNode::Value($value as _)
    };

    (
        @trie { $($($key:literal)|+ => $value:tt,)+ }
    ) => {
        {
            let cap = hashmap!(@count $($($key),+),*);
            let mut map = ::std::collections::HashMap::with_capacity(cap);
            $(
                $(
                    let key = $key.parse::<$crate::input::KeyEvent>().expect("failed to parse key event");
                    let duplicate = map.insert(
                        key.clone(),
                        trie!(@trie $value)
                    );
                    assert!(duplicate.is_none(), "duplicate key found: {key}");
                )+
            )*
            let trie = $crate::keymap::Trie::new(map);
            $crate::keymap::TrieNode::Trie(trie)
        }
    };

    (
        { $($($key:literal)|+ => $value:tt,)+ }
    ) => {
        trie!(@trie { $($($key)|+ => $value,)+}).into_trie()
    };
}
