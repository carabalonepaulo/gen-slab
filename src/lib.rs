#![allow(unused)]

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    idx: u32,
    generation: u32,
}

#[derive(Debug)]
struct Slot<T> {
    value: Option<T>,
    generation: u32,
}

#[derive(Debug)]
pub struct GenSlab<T> {
    slots: Vec<Slot<T>>,
    free: Vec<usize>,
}

impl<T> GenSlab<T> {
    pub fn new() -> Self {
        Self {
            slots: vec![],
            free: vec![],
        }
    }

    pub fn with_capacity(n: usize) -> Self {
        Self {
            slots: Vec::with_capacity(n),
            free: Vec::with_capacity(n),
        }
    }

    pub fn insert(&mut self, value: T) -> Key {
        let idx = {
            if let Some(i) = self.free.pop() {
                i
            } else {
                self.slots.push(Slot {
                    value: None,
                    generation: 0,
                });
                self.slots.len() - 1
            }
        };

        let slot = &mut self.slots[idx];
        slot.value = Some(value);

        Key {
            idx: idx as _,
            generation: slot.generation,
        }
    }

    pub fn remove(&mut self, key: Key) -> Option<T> {
        let key_idx = key.idx as usize;
        let slot = self.slots.get_mut(key_idx)?;
        if slot.generation != key.generation {
            return None;
        }

        let value = slot.value.take()?;
        self.slots[key_idx].generation += 1;
        self.free.push(key_idx);
        Some(value)
    }

    pub fn get(&self, key: Key) -> Option<&T> {
        let slot = self.slots.get(key.idx as usize)?;
        if slot.generation != key.generation {
            return None;
        }
        slot.value.as_ref()
    }

    pub fn get_mut(&mut self, key: Key) -> Option<&mut T> {
        let slot = self.slots.get_mut(key.idx as usize)?;
        if slot.generation != key.generation {
            return None;
        }
        slot.value.as_mut()
    }

    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            inner: self.slots.iter().enumerate(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut {
            inner: self.slots.iter_mut().enumerate(),
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
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Key, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        for (idx, slot) in self.inner.by_ref() {
            if let Some(ref value) = slot.value {
                return Some((
                    Key {
                        idx: idx as u32,
                        generation: slot.generation,
                    },
                    value,
                ));
            }
        }
        None
    }
}

pub struct IterMut<'a, T> {
    inner: std::iter::Enumerate<std::slice::IterMut<'a, Slot<T>>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Key, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        for (idx, slot) in self.inner.by_ref() {
            if let Some(ref mut value) = slot.value {
                return Some((
                    Key {
                        idx: idx as u32,
                        generation: slot.generation,
                    },
                    value,
                ));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
