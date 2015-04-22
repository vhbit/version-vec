#![feature(collections, core, convert)]
#![allow(dead_code)]

use std::cmp;
use std::fmt;
use std::ops::{Deref};
use std::num::Int;

#[derive(Copy, Clone, Eq, Debug, PartialEq)]
/// Describes relations between two version vectors
pub enum Ordering {
    Less,
    Equal,
    Greater,
    /// vectors have at least 1 concurrent update
    Concurrent
}

impl Ordering {
    #[inline]
    fn eat(&mut self, order: cmp::Ordering) {
        match (order, *self) {
            (cmp::Ordering::Less, Ordering::Equal) => *self = Ordering::Less,
            (cmp::Ordering::Greater, Ordering::Equal) => *self = Ordering::Greater,
            (cmp::Ordering::Greater, Ordering::Less) |
            (cmp::Ordering::Less, Ordering::Greater) => *self = Ordering::Concurrent,
            _ => ()
        }
    }
}

/// Represents version vector.
///
/// Currently inner implementation is a sorted vector
pub struct VersionVec<I, T> {
    inner: Vec<(I, T)>
}

impl<I: fmt::Debug, T: fmt::Debug> fmt::Debug for VersionVec<I, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&format!("Versions: {:?}", self.inner))
    }
}

impl<I: Clone, T: Clone> Clone for VersionVec<I, T> {
    fn clone(&self) -> VersionVec<I, T> {
        VersionVec {
            inner: self.inner.clone()
        }
    }
}

impl<I, T> VersionVec<I, T> where I: Ord + Copy + Clone, T: Ord + Copy + Clone + Int {
    /// Creates a new empty version vector
    pub fn new() -> VersionVec<I, T> {
        VersionVec {
            inner: vec![]
        }
    }

    /// Constructs version vector from tuples (id, version)
    pub fn from_vec(v: Vec<(I, T)>) -> VersionVec<I, T> {
        let mut v = v;
        v.sort_by(|a, b| a.0.cmp(&b.0));
        VersionVec {
            inner: v
        }
    }

    /// Creates a new copy of self, merges other into that copy and returns it
    pub fn merged(&self, other: &VersionVec<I, T>) -> VersionVec<I, T> {
        let mut result = self.clone();
        result.merge(other);
        result
    }

    /// Returns the value of counter with id if it exists
    pub fn get(&self, id: I) -> Option<T> {
        for i in &self.inner {
            if i.0 == id {
                return Some(i.1)
            } else if i.0 > id {
                return None
            }
        }

        None
    }

    /// Bump (increase) counter for specified id.
    /// If id is missing, adds a new and sets value to 1
    pub fn bump_for(&mut self, id: I) {
        let idx = self.inner.iter().position(|value| value.0 >= id);
        match idx {
            None => self.inner.push((id, T::one())),
            Some(idx) => {
                if self.inner[idx].0 == id {
                    self.inner[idx].1 = self.inner[idx].1 + T::one()
                } else {
                    self.inner.insert(idx, (id, T::one()))
                }
            }
        }
    }

    /// Merge in-place
    pub fn merge(&mut self, other: &VersionVec<I, T>) {
        let mut self_idx = 0;
        let mut other_idx = 0;

        loop {
            if self_idx >= self.inner.len() {
                self.inner.push_all(&other.inner[other_idx..]);
                break
            }

            if other_idx >= other.inner.len() {
                break
            }

            let left = self.inner[self_idx];
            let right = other.inner[other_idx];

            if left.0 == right.0 {
                self.inner[self_idx].1 = cmp::max(left.1, right.1);
                self_idx += 1;
                other_idx += 1;
            } else {
                if left.0 < right.0 {
                    self_idx += 1
                } else {
                    self.inner.insert(self_idx, right);
                    self_idx += 1;
                    other_idx += 1;
                }
            }
        }
    }

    /// Compares 2 version vectors
    pub fn cmp(&self, other: &VersionVec<I, T>) -> Ordering {
        let mut self_idx = 0;
        let mut other_idx = 0;
        let mut result = Ordering::Equal;

        loop {
            if self_idx >= self.inner.len() {
                if other_idx == other.inner.len() {
                    // both exhausted
                    return result
                } else {
                    // other is not exhausted, so self is less if there is at least 1 non-zero
                    if other.inner[other_idx..].iter().position(|v| v.1 > T::zero()).is_some() {
                        result.eat(cmp::Ordering::Less);
                    }
                    return result
                }
            }

            if other_idx >= other.inner.len() {
                // since we've got here self is not exhausted yet
                // => self is greater if there is at least 1 non-zero
                if self.inner[self_idx..].iter().position(|v| v.1 > T::zero()).is_some() {
                    result.eat(cmp::Ordering::Greater);
                }
                return result
            }

            let left = self.inner[self_idx];
            let right = other.inner[other_idx];

            let id_cmp = left.0.cmp(&right.0);
            let deltas = match id_cmp {
                cmp::Ordering::Less => (1, 0, if left.1 != T::zero() {cmp::Ordering::Greater} else {cmp::Ordering::Equal}),
                cmp::Ordering::Greater => (0, 1, if right.1 != T::zero() {cmp::Ordering::Less} else {cmp::Ordering::Equal}),
                cmp::Ordering::Equal => (1, 1, left.1.cmp(&right.1))
            };

            self_idx += deltas.0;
            other_idx += deltas.1;
            if deltas.2 != cmp::Ordering::Equal {
                result.eat(deltas.2);
            }

            // Ouch, there is a conflict, nothing to catch here
            if result == Ordering::Concurrent {
                return result;
            }
        }
    }
}

/*
impl<I, T> Index<RangeFull> for VersionVec<I, T> {
    type Output = [(I, T)];

    fn index<'a>(&'a self, _index: &RangeFull) -> &'a [(I, T)] {
        &self.inner
    }
}
*/

/*
impl<I, T> AsSlice<(I, T)> for VersionVec<I, T> {
    fn as_slice(&self) -> &[(I, T)] {
        &self.inner
    }
}
*/

// FIXME: it actually should be convert::AsRef but since I'm stick to
// an old version, Deref works much better for now
impl<I, T> Deref for VersionVec<I, T> {
    type Target = [(I, T)];

    fn deref<'a>(&'a self) -> &'a [(I, T)] {
        &self.inner
    }
}


#[cfg(test)]
mod test {
    use super::{Ordering, VersionVec};

    type VecTemplate = Vec<(usize, usize)>;

    #[test]
    fn get_counter() {
        let v = VersionVec::from_vec(vec![(1, 10), (2, 20), (3, 30)]);

        assert_eq!(v.get(1), Some(10));
        assert_eq!(v.get(5), None);
        assert_eq!(v.get(2), Some(20));
        assert_eq!(v.get(3), Some(30));
        assert_eq!(v.get(6), None);
    }

    #[test]
    fn bump() {
        let mut v = VersionVec::from_vec(vec![(1, 10), (2, 20), (3, 30)]);

        v.bump_for(1);
        assert_eq!(&*v, [(1, 11), (2, 20), (3, 30)]);

        v.bump_for(0);
        assert_eq!(&*v, [(0, 1), (1, 11), (2, 20), (3, 30)]);

        v.bump_for(10);
        assert_eq!(&*v, [(0, 1), (1, 11), (2, 20), (3, 30), (10, 1)]);
    }

    #[test]
    fn comparisons() {
        // Taken from synching test cases, except concurrent and nil cases
        // https://github.com/syncthing/protocol/blob/master/vector_compare_test.go
        //
        // Original version vectors do have a single concurrent state
        use Ordering::*;

        let test_cases: Vec<(Ordering, VecTemplate, VecTemplate)> = vec![
            (Equal, vec![], vec![]),
            (Equal, vec![], vec![(10, 0)]),
            (Equal, vec![(10, 0)], vec![]),
            (Equal, vec![(10, 0)], vec![(20, 0)]),
            (Equal, vec![(10, 1), (20, 2)], vec![(10, 1), (20, 2)]),
            (Greater, vec![(1, 10)], vec![]),
            (Greater, vec![(1, 10)], vec![(1, 0)]),
            (Greater, vec![(1, 10)], vec![(1, 8)]),
            (Greater, vec![(1, 20), (20, 50)], vec![(1, 10), (20, 20)]),
            (Greater, vec![(1, 10), (20, 50)], vec![(1, 10), (20, 20)]),
            (Less, vec![], vec![(1, 10)]),
            (Less, vec![(1, 0)], vec![(1, 10)]),
            (Less, vec![(1, 8)], vec![(1, 10)]),
            (Less, vec![(1, 8), (2, 20)], vec![(1, 10), (2, 20)]),
            (Less, vec![(1, 8), (2, 20)], vec![(1, 8), (2, 50)]),
            (Concurrent, vec![(1, 10)], vec![(2, 22)]),
            (Concurrent, vec![(1, 10), (2, 20)], vec![(1, 8), (2, 22)]),
            ];

        for case in test_cases {
            let v1 = VersionVec::from_vec(case.1);
            let v2 = VersionVec::from_vec(case.2);

            let res = v1.cmp(&v2);
            assert!(res == case.0, "expected: {:?}, got {:?}, left {:?}, right {:?}", case.0, res, v1, v2);
        }
    }

    #[test]
    fn merge() {
        let test_cases: Vec<(VecTemplate, VecTemplate, VecTemplate)> = vec![
            (vec![], vec![], vec![]),
            (vec![(1, 10), (2, 20)], vec![(1, 10), (2, 20)], vec![(1, 10), (2, 20)]),
            (vec![], vec![(1, 10)], vec![(1, 10)]),
            (vec![(1, 10)], vec![(1, 10), (2, 20)], vec![(1, 10), (2, 20)]),
            (vec![(1, 10)], vec![(2, 20)], vec![(1, 10), (2, 20)]),
            (vec![(1, 10), (4, 40)], vec![(1, 10), (2, 20), (4, 40)], vec![(1, 10), (2, 20), (4, 40)]),
            (vec![(1, 10), (2, 40)], vec![(1, 20), (2, 20)], vec![(1, 20), (2, 40)]),
            (vec![(10, 1), (20, 2), (30, 1)], vec![(5, 1), (10, 2), (15, 1), (20, 1), (25, 1), (35, 1)],
             vec![(5, 1), (10, 2), (15, 1), (20, 2), (25, 1), (30, 1), (35, 1)])
            ];

        for case in test_cases {
            let v1 = VersionVec::from_vec(case.0);
            let v2 = VersionVec::from_vec(case.1);
            let merged = v1.merged(&v2);

            assert_eq!(&*merged, case.2.as_slice());
        }
    }
}
