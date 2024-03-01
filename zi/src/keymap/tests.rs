use super::Keymap;
use crate::keymap::TrieResult::*;
use crate::Mode;

#[test]
fn test_keymap() {
    let mut keymap = Keymap::<Mode, char, u32>::default();

    assert!(keymap.insert(Mode::Normal, ['i'], 1).is_none());
    assert_eq!(keymap.on_key(Mode::Normal, 'i'), (Found(1), vec![]));

    // Pressing random key should be an noop and shouldn't affect the next keypress.
    assert_eq!(keymap.on_key(Mode::Normal, 'x'), (Nothing, vec!['x']));
    assert_eq!(keymap.on_key(Mode::Normal, 'i'), (Found(1), vec![]));

    // overwrite the previous key
    assert_eq!(keymap.insert(Mode::Normal, ['i'], 2), Some(1));
    assert_eq!(keymap.on_key(Mode::Normal, 'i'), (Found(2), vec![]));

    // multiple keys
    assert!(keymap.insert(Mode::Normal, ['f', 'd'], 3).is_none());
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), (Found(3), vec![]));

    // The second 'ff' doesn't exist so the second 'f' should become the beginning of a new sequence
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Partial, vec![]));
    // It should return `Partial` because we've started a new sequence with the second `f`.
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Partial, vec!['f']));
    // Since an `f` is buffered, the next `d` should be the end of the `fd` sequence
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), (Found(3), vec![]));

    // If we hit a key that doesn't exist, the buffer should reset
    assert_eq!(keymap.on_key(Mode::Normal, 'x'), (Nothing, vec!['x']));
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), (Found(3), vec![]));

    // (Partial, vec![]) overlapping binding
    assert!(keymap.insert(Mode::Normal, ['f', 'f'], 4).is_none());
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Found(4), vec![]));
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), (Found(3), vec![]));

    // Overwrite with longer key sequence
    assert_eq!(keymap.insert(Mode::Normal, ['f', 'd', 'd'], 5), Some(3));
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), (Found(5), vec![]));

    // Different mode
    assert_eq!(keymap.insert(Mode::Insert, ['f', 'd'], 6), None);
    assert_eq!(keymap.on_key(Mode::Insert, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Insert, 'd'), (Found(6), vec![]));

    // Mode changes reset the buffer
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Insert, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Insert, 'd'), (Found(6), vec![]));

    // mode with no bindings should return `Nothing`
    assert_eq!(keymap.on_key(Mode::Visual, 'i'), (Nothing, vec!['i']));

    // `fx` should execute the `x` command and pass the buffered `f` to the caller
    assert!(keymap.insert(Mode::Insert, ['x'], 42).is_none());
    assert_eq!(keymap.on_key(Mode::Insert, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Insert, 'x'), (Found(42), vec!['f']));

    assert_eq!(keymap.on_key(Mode::Insert, 'f'), (Partial, vec![]));
    assert_eq!(keymap.on_key(Mode::Insert, 'f'), (Partial, vec!['f']));
    assert_eq!(keymap.on_key(Mode::Insert, 'f'), (Partial, vec!['f']));
    assert_eq!(keymap.on_key(Mode::Insert, 'f'), (Partial, vec!['f']));
    assert_eq!(keymap.on_key(Mode::Insert, 'd'), (Found(6), vec![]));
}
