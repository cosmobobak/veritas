use std::fmt::Debug;


/// A handle to an object in the arena.
/// 
/// TODO: Make this non-null, with a separate nullable handle.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Handle(u32);

impl Handle {
    /// Returns the null handle.
    pub const fn null() -> Self {
        Self(u32::MAX)
    }

    /// Returns true if this is the null handle.
    pub const fn is_null(self) -> bool {
        self.0 == u32::MAX
    }

    /// Returns the index of this handle.
    pub const fn index(self) -> usize {
        assert!(!self.is_null());
        self.0 as usize
    }

    /// Returns the handle with the given index.
    pub const fn from_index<T>(index: usize, memory: &[T]) -> Self {
        assert!(index < memory.len());
        Self(index as u32)
    }
}

impl Debug for Handle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_null() {
            write!(f, "Handle::null()")
        } else {
            write!(f, "Handle({})", self.index())
        }
    }
}