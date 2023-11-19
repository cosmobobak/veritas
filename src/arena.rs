
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
    pub const fn is_null(&self) -> bool {
        self.0 == u32::MAX
    }

    /// Returns the index of this handle.
    pub const fn index(&self) -> u32 {
        self.0
    }
}

/// An arena for storing objects.
pub struct Arena<T> {
    /// The objects in the arena.
    objects: Vec<T>,
    /// The free list.
    free_list: Vec<u32>,
}

impl<T> Arena<T> {
    /// Creates a new arena.
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            free_list: Vec::new(),
        }
    }

    /// Allocates an object in the arena.
    pub fn alloc(&mut self, object: T) -> Handle {
        if let Some(index) = self.free_list.pop() {
            self.objects[index as usize] = object;
            Handle(index)
        } else {
            self.objects.push(object);
            Handle(<_ as TryInto<u32>>::try_into(self.objects.len()).unwrap() - 1)
        }
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

    /// Deallocates an object in the arena.
    pub fn dealloc(&mut self, handle: Handle) {
        self.free_list.push(handle.index());
    }
}