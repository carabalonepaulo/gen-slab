#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Key(u64);

impl Key {
    const INDEX_BITS: u64 = 32;
    const INDEX_MASK: u64 = (1u64 << Self::INDEX_BITS) - 1;

    #[inline]
    fn pack(idx: u32, generation: u32, salt: u64) -> Self {
        let raw = ((generation as u64) << Self::INDEX_BITS) | (idx as u64 & Self::INDEX_MASK);
        Self(raw ^ salt)
    }

    #[inline]
    fn unpack(self, salt: u64) -> (u32, u32) {
        let raw = self.0 ^ salt;
        let idx = (raw & Self::INDEX_MASK) as u32;
        let generation = (raw >> Self::INDEX_BITS) as u32;
        (idx, generation)
    }

    #[inline]
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

#[derive(Debug)]
struct Slot<T> {
    value: Option<T>,
    generation: u32,
}

#[derive(Debug)]
pub struct GenSlab<T> {
    slots: Vec<Slot<T>>,
    free: Vec<u32>,
    salt: u64,
}

impl<T> GenSlab<T> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(n: usize) -> Self {
        let salt = {
            let mut salt = 0u64;
            while salt == 0 {
                salt = rand::random();
            }
            salt
        };

        Self {
            slots: Vec::with_capacity(n),
            free: Vec::with_capacity(n),
            salt,
        }
    }

    pub fn insert(&mut self, value: T) -> Key {
        let idx = {
            if let Some(i) = self.free.pop() {
                i
            } else {
                assert!(self.slots.len() < u32::MAX as usize);
                self.slots.push(Slot {
                    value: None,
                    generation: 0,
                });
                (self.slots.len() - 1) as u32
            }
        };

        let slot = &mut self.slots[idx as usize];
        slot.value = Some(value);

        Key::pack(idx as u32, slot.generation, self.salt)
    }

    pub fn remove(&mut self, key: Key) -> Option<T> {
        let (idx, generation) = key.unpack(self.salt);
        let idx_usize = idx as usize;

        let slot = self.slots.get_mut(idx_usize)?;
        if slot.generation != generation {
            return None;
        }

        let value = slot.value.take()?;
        slot.generation = slot.generation.wrapping_add(1);
        self.free.push(idx);
        Some(value)
    }

    pub fn get(&self, key: Key) -> Option<&T> {
        let (idx, generation) = key.unpack(self.salt);
        let slot = self.slots.get(idx as usize)?;
        if slot.generation != generation {
            return None;
        }
        slot.value.as_ref()
    }

    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        let (idx, generation) = key.unpack(self.salt);
        let slot = self.slots.get_mut(idx as usize)?;
        if slot.generation != generation {
            return None;
        }
        slot.value.as_mut()
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.slots.iter().enumerate(),
            salt: self.salt,
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            inner: self.slots.iter_mut().enumerate(),
            salt: self.salt,
        }
    }
}

impl<T> Default for GenSlab<T> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Iter<'a, T> {
    inner: std::iter::Enumerate<std::slice::Iter<'a, Slot<T>>>,
    salt: u64,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Key, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        for (idx, slot) in self.inner.by_ref() {
            if let Some(ref value) = slot.value {
                return Some((Key::pack(idx as u32, slot.generation, self.salt), value));
            }
        }
        None
    }
}

pub struct IterMut<'a, T> {
    inner: std::iter::Enumerate<std::slice::IterMut<'a, Slot<T>>>,
    salt: u64,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Key, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        for (idx, slot) in self.inner.by_ref() {
            if let Some(ref mut value) = slot.value {
                return Some((Key::pack(idx as u32, slot.generation, self.salt), value));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_get() {
        let mut slab = GenSlab::new();

        let key = slab.insert(42);
        assert_eq!(slab.get(key), Some(&42));

        let key2 = slab.insert(100);
        assert_eq!(slab.get(key2), Some(&100));
    }

    #[test]
    fn remove_invalidates_key() {
        let mut slab = GenSlab::new();

        let key = slab.insert(10);
        assert_eq!(slab.remove(key), Some(10));

        assert_eq!(slab.get(key), None);
        assert_eq!(slab.remove(key), None);
    }

    #[test]
    fn slot_reuse_changes_generation() {
        let mut slab = GenSlab::new();

        let key1 = slab.insert(1);
        slab.remove(key1);

        let key2 = slab.insert(2);

        assert_ne!(key1.as_u64(), key2.as_u64());

        assert_eq!(slab.get(key1), None);
        assert_eq!(slab.get(key2), Some(&2));
    }

    #[test]
    fn cross_slab_key_is_invalid() {
        let mut slab1 = GenSlab::new();
        let mut slab2 = GenSlab::<usize>::new();

        let key = slab1.insert(55);

        assert_eq!(slab2.get(key), None);
        assert_eq!(slab2.remove(key), None);
    }

    #[test]
    fn iter_returns_all_live_values() {
        let mut slab = GenSlab::new();

        let k1 = slab.insert(1);
        let k2 = slab.insert(2);
        let k3 = slab.insert(3);

        slab.remove(k2);

        let mut collected: Vec<_> = slab.iter().map(|(_, v)| *v).collect();
        collected.sort();

        assert_eq!(collected, vec![1, 3]);

        assert_eq!(slab.get(k1), Some(&1));
        assert_eq!(slab.get(k2), None);
        assert_eq!(slab.get(k3), Some(&3));
    }

    #[test]
    fn iter_mut_allows_modification() {
        let mut slab = GenSlab::new();

        slab.insert(5);
        slab.insert(10);

        for (_, v) in slab.iter_mut() {
            *v *= 2;
        }

        let mut values: Vec<_> = slab.iter().map(|(_, v)| *v).collect();
        values.sort();

        assert_eq!(values, vec![10, 20]);
    }

    #[test]
    fn multiple_removes_do_not_corrupt() {
        let mut slab = GenSlab::new();

        let k1 = slab.insert(1);
        let k2 = slab.insert(2);
        let k3 = slab.insert(3);

        slab.remove(k2);
        slab.remove(k1);
        slab.remove(k3);

        assert!(slab.iter().next().is_none());

        let k4 = slab.insert(100);
        assert_eq!(slab.get(k4), Some(&100));
    }

    #[test]
    fn generation_wrap_invalidates_old_keys() {
        let mut slab = GenSlab::new();

        let key1 = slab.insert(123);
        let (idx, _) = key1.unpack(slab.salt);

        slab.remove(key1);

        let slot = &mut slab.slots[idx as usize];
        slot.generation = u32::MAX;

        let key2 = slab.insert(456);

        slab.remove(key2);

        assert_eq!(slab.slots[idx as usize].generation, 0);
        assert_eq!(slab.get(key1), None);
    }

    #[test]
    fn capacity_growth_works() {
        let mut slab = GenSlab::with_capacity(2);

        let k1 = slab.insert(1);
        let k2 = slab.insert(2);
        let k3 = slab.insert(3);

        assert_eq!(slab.get(k1), Some(&1));
        assert_eq!(slab.get(k2), Some(&2));
        assert_eq!(slab.get(k3), Some(&3));
    }
}
