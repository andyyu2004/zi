pub trait Sealed {}

impl<T: ?Sized> Sealed for &T {}

// A token that prevents external code from invoking certain methods.
// Primarily for trait methods that are only meant to be called internally.
pub struct Internal(pub(crate) ());
