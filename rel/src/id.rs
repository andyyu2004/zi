use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

/// An untyped row id: a slot index and the generation of the row in that slot.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RawId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl RawId {
    pub fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// A row id type. [`Id<T>`] is the default; implement this for your own id type
/// and pick it with `#[relation(key = MyId)]`. The conversions must round-trip.
pub trait Key: Copy + Eq + Ord + Hash + fmt::Debug + 'static {
    fn from_raw(raw: RawId) -> Self;
    fn raw(self) -> RawId;
}

/// Define id types that implement [`Key`], for `#[relation(key = ...)]`.
/// Useful where the row type can't be named, e.g. in a crate the rows' crate depends on.
///
/// ```
/// rel::key! {
///     pub struct BufferId;
/// }
/// ```
#[macro_export]
macro_rules! key {
    ($($(#[$attr:meta])* $vis:vis struct $name:ident;)*) => {$(
        $(#[$attr])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        $vis struct $name($crate::RawId);

        impl $crate::Key for $name {
            fn from_raw(raw: $crate::RawId) -> Self {
                Self(raw)
            }

            fn raw(self) -> $crate::RawId {
                self.0
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                ::std::write!(f, "{}({}v{})", ::std::stringify!($name), self.0.index(), self.0.generation())
            }
        }
    )*};
}

impl<T: 'static> Key for Id<T> {
    fn from_raw(raw: RawId) -> Self {
        Id::from_raw(raw)
    }

    fn raw(self) -> RawId {
        self.raw
    }
}

/// The id of a row of `T`. Ids are generational: once a row is deleted, its id
/// never refers to another row.
pub struct Id<T> {
    raw: RawId,
    _marker: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    pub fn from_raw(raw: RawId) -> Self {
        Self { raw, _marker: PhantomData }
    }

    pub fn raw(self) -> RawId {
        self.raw
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.raw.cmp(&other.raw)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = std::any::type_name::<T>();
        let name = name.rsplit("::").next().unwrap_or(name);
        write!(f, "{name}#{}v{}", self.raw.index, self.raw.generation)
    }
}
