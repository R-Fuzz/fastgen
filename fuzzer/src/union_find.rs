/// reused from https://github.com/tov/disjoint-sets-rs/blob/master/src/array.rs
/// augmented with links to retrieve all elements in a set


use std::cell::Cell;
use std::fmt::{self, Debug};

/// A type that can be used as a [`UnionFind`](struct.UnionFind.html)
/// element.
///
/// It must be safely convertible to and from `usize`.
///
/// The two methods must be well-behaved partial inverses as follows:
///
/// -  For all `n: usize`, if `Self::from_usize(n)` = `Some(t)` then
///    `t.to_usize()` = `n`.
/// -  For all `t: Self`, if `t.to_usize()` = `n` then
///    `Self::from_usize(n)` = `Some(t)`.
/// -  For all `n: usize`, if `Self::from_usize(n)` = `None` then for all
///    `m: usize` such that `m > n`, `Self::from_usize(m)` = `None`.
///
/// In other words, `ElementType` sets up a bijection between the first
/// *k* `usize` values and some *k* values of the `Self` type.
pub trait ElementType : Copy + Debug + Eq {
    /// Converts from `usize` to the element type.
    ///
    /// Returns `None` if the argument won’t fit in `Self`.
    fn from_usize(n: usize) -> Option<Self>;

    /// Converts from the element type to `usize`.
    fn to_usize(self) -> usize;
}

impl ElementType for usize {
    #[inline]
    fn from_usize(n: usize) -> Option<usize> { Some(n) }
    #[inline]
    fn to_usize(self) -> usize { self }
}

macro_rules! element_type_impl {
    ($type_:ident) => {
        impl ElementType for $type_ {
            #[inline]
            fn from_usize(u: usize) -> Option<Self> {
                let result = u as $type_;
                if result as usize == u { Some(result) } else { None }
            }

            #[inline]
            fn to_usize(self) -> usize {
                self as usize
            }
        }
  }
}

element_type_impl!(u8);
element_type_impl!(u16);
element_type_impl!(u32);

/// Vector-based union-find representing a set of disjoint sets.
///
/// If configured with Cargo feature `"serde"`, impls for `Serialize`
/// and `Deserialize` will be defined.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct UnionFind<Element: ElementType = usize> {
    elements: Vec<Cell<Element>>,
    links: Vec<Cell<Element>>,
    ranks: Vec<u8>,
}
// Invariant: self.elements.len() == self.ranks.len()

impl<Element: Debug + ElementType> Debug for UnionFind<Element> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "UnionFind({:?})", self.elements)
    }
}

impl<Element: ElementType> Default for UnionFind<Element> {
    fn default() -> Self {
        UnionFind::new(0)
    }
}

impl<Element: ElementType> UnionFind<Element> {
    /// Creates a new union-find of `size` elements.
    ///
    /// # Panics
    ///
    /// If `size` elements would overflow the element type `Element`.
    pub fn new(size: usize) -> Self {
        UnionFind {
            elements: (0..size).map(|i| {
                let e = Element::from_usize(i).expect("UnionFind::new: overflow");
                Cell::new(e)
            }).collect(),
            links: (0..size).map(|i| {
                let e = Element::from_usize(i).expect("UnionFind::new: overflow");
                Cell::new(e)
            }).collect(),
            ranks: vec![0; size],
        }
    }

    /// The number of elements in all the sets.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Is the union-find devoid of elements?
    ///
    /// It is possible to create an empty `UnionFind` and then add
    /// elements with [`alloc`](#method.alloc).
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Creates a new element in a singleton set.
    ///
    /// # Panics
    ///
    /// If allocating another element would overflow the element type
    /// `Element`.
    pub fn alloc(&mut self) -> Element {
        let result = Element::from_usize(self.elements.len())
                       .expect("UnionFind::alloc: overflow");
        self.elements.push(Cell::new(result));
        self.ranks.push(0);
        result
    }

    /// Joins the sets of the two given elements.
    ///
    /// Returns whether anything changed. That is, if the sets were
    /// different, it returns `true`, but if they were already the same
    /// then it returns `false`.
    pub fn union(&mut self, a: Element, b: Element) -> bool {
        let a = self.find(a);
        let b = self.find(b);

        if a == b { return false; }

        let temp = self.link(b);
        self.set_link(b, self.link(a));
        self.set_link(a, temp);

        let rank_a = self.rank(a);
        let rank_b = self.rank(b);

        if rank_a > rank_b {
            self.set_parent(b, a);
        } else if rank_b > rank_a {
            self.set_parent(a, b);
        } else {
            self.set_parent(a, b);
            self.increment_rank(b);
        }

        true
    }

    /// Finds the representative element for the given element’s set.
    pub fn find(&self, mut element: Element) -> Element {
        let mut parent = self.parent(element);

        while element != parent {
            let grandparent = self.parent(parent);
            self.set_parent(element, grandparent);
            element = parent;
            parent = grandparent;
        }

        element
    }

    /// Determines whether two elements are in the same set.
    pub fn equiv(&self, a: Element, b: Element) -> bool {
        self.find(a) == self.find(b)
    }

    /// Forces all laziness, so that each element points directly to its
    /// set’s representative.
    pub fn force(&self) {
        for i in 0 .. self.len() {
            let element = Element::from_usize(i).unwrap();
            let root = self.find(element);
            self.set_parent(element, root);
        }
    }

    /// Returns a vector of set representatives.
    pub fn to_vec(&self) -> Vec<Element> {
        self.force();
        self.elements.iter().map(Cell::get).collect()
    }

    /// Returns a vector of the elements in the sameset  for a given element 
    pub fn get_set(&self, a: Element) -> Vec<Element> {
      let mut ret: Vec<Element>  = Vec::new();
      let root  = a;
      let mut p = a;
      ret.push(a);
      while self.link(p) != root {
        p = self.link(p);
        ret.push(p);
      }
      ret
    }

    // HELPERS

    fn rank(&self, element: Element) -> u8 {
        self.ranks[element.to_usize()]
    }

    fn increment_rank(&mut self, element: Element) {
        let i = element.to_usize();
        self.ranks[i] = self.ranks[i].saturating_add(1);
    }

    fn parent(&self, element: Element) -> Element {
        self.elements[element.to_usize()].get()
    }

    fn link(&self, element: Element) -> Element {
        self.links[element.to_usize()].get()
    }

    fn set_parent(&self, element: Element, parent: Element) {
        self.elements[element.to_usize()].set(parent);
    }

    fn set_link(&self, element: Element, link: Element) {
        self.links[element.to_usize()].set(link);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn len() {
        assert_eq!(5, UnionFind::<u32>::new(5).len());
    }

    #[test]
    fn union() {
        let mut uf = UnionFind::<u32>::new(8);
        assert!(!uf.equiv(0, 1));
        uf.union(0, 1);
        assert!(uf.equiv(0, 1));
    }

    #[test]
    fn unions() {
        let mut uf = UnionFind::<usize>::new(8);
        assert!(uf.union(0, 1));
        assert!(uf.union(1, 2));
        assert!(uf.union(4, 3));
        assert!(uf.union(3, 2));
        assert!(! uf.union(0, 3));

        assert!(uf.equiv(0, 1));
        assert!(uf.equiv(0, 2));
        assert!(uf.equiv(0, 3));
        assert!(uf.equiv(0, 4));
        assert!(!uf.equiv(0, 5));

        uf.union(5, 3);
        assert!(uf.equiv(0, 5));

        uf.union(6, 7);
        assert!(uf.equiv(6, 7));
        assert!(!uf.equiv(5, 7));

        uf.union(0, 7);
        assert!(uf.equiv(5, 7));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_round_trip() {
        extern crate serde_json;

        let mut uf0: UnionFind<usize> = UnionFind::new(8);
        uf0.union(0, 1);
        uf0.union(2, 3);
        assert!( uf0.equiv(0, 1));
        assert!(!uf0.equiv(1, 2));
        assert!( uf0.equiv(2, 3));

        let json = serde_json::to_string(&uf0).unwrap();
        let uf1: UnionFind<usize> = serde_json::from_str(&json).unwrap();
        assert!( uf1.equiv(0, 1));
        assert!(!uf1.equiv(1, 2));
        assert!( uf1.equiv(2, 3));
    }
}
