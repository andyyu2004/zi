use super::Keymap;
use crate::Mode;

#[test]
fn test_keymap() {
    let mut keymap = Keymap::<Mode, char, u32>::default();

    assert!(keymap.insert(Mode::Normal, ['i'], 1).is_none());
    assert_eq!(keymap.on_key(Mode::Normal, 'i'), Some(&1));

    // Pressing random key should be an noop and shouldn't affect the next keypress.
    assert!(keymap.on_key(Mode::Normal, 'x').is_none());
    assert_eq!(keymap.on_key(Mode::Normal, 'i'), Some(&1));

    // overwrite the previous key
    assert_eq!(keymap.insert(Mode::Normal, ['i'], 2), Some(1));
    assert_eq!(keymap.on_key(Mode::Normal, 'i'), Some(&2));

    // multiple keys
    assert!(keymap.insert(Mode::Normal, ['f', 'd'], 3).is_none());
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), Some(&3));

    // The second 'ff' doesn't exist so the second 'f' should become the beginning of a new sequence
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), Some(&3));

    // If we hit a key that doesn't exist, the buffer should reset
    assert_eq!(keymap.on_key(Mode::Normal, 'x'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), Some(&3));

    // Partial overlapping binding
    assert!(keymap.insert(Mode::Normal, ['f', 'f'], 4).is_none());
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), Some(&4));
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), Some(&3));

    // Overwrite with longer key sequence
    assert_eq!(keymap.insert(Mode::Normal, ['f', 'd', 'd'], 5), Some(3));
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), None);
    assert_eq!(keymap.on_key(Mode::Normal, 'd'), Some(&5));

    // Different mode
    assert_eq!(keymap.insert(Mode::Insert, ['f', 'd'], 6), None);
    assert_eq!(keymap.on_key(Mode::Insert, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Insert, 'd'), Some(&6));

    // Mode changes reset the buffer
    assert_eq!(keymap.on_key(Mode::Normal, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Insert, 'f'), None);
    assert_eq!(keymap.on_key(Mode::Insert, 'd'), Some(&6));
}
