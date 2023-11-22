
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
        self.0 as usize
    }
}

/// An arena for storing objects.
pub struct Arena<T> {
    /// The objects in the arena.
    objects: Vec<T>,
}

impl<T> Arena<T> {
    /// Creates a new arena.
    pub const fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    /// Allocates an object in the arena.
    pub fn alloc(&mut self, object: T) -> Handle {
        #![allow(clippy::cast_possible_truncation)]
        let index = self.objects.len();
        self.objects.push(object);
        Handle(index as u32)
    }

    /// Returns a reference to an object in the arena.
    pub fn get(&self, handle: Handle) -> &T {
        &self.objects[handle.index() as usize]
    }

    /// Returns a mutable reference to an object in the arena.
    pub fn get_mut(&mut self, handle: Handle) -> &mut T {
        &mut self.objects[handle.index() as usize]
    }

    /// Returns a reference to an object in the arena, if it exists.
    pub fn try_get(&self, handle: Handle) -> Option<&T> {
        self.objects.get(handle.index() as usize)
    }

    /// Returns a mutable reference to an object in the arena, if it exists.
    pub fn try_get_mut(&mut self, handle: Handle) -> Option<&mut T> {
        self.objects.get_mut(handle.index() as usize)
    }
}