// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Persistent ordered AVL index with subtree maximum augmentation.
//!
//! Updates allocate only the search/rotation paths. Each node retains its own
//! metadata charge; shared subtrees remain alive for snapshot readers. All
//! fallible work happens before the caller publishes a replacement root.

use crate::mm::FallibleArc;

/// Per-node accounting supplied by the caller. The charge lives with the node,
/// including while old snapshots retain it. No global policy is assumed.
pub trait NodeAccount {
    type Charge;
    type Error;
    fn try_charge(&self, bytes: usize) -> core::result::Result<Self::Charge, Self::Error>;
}

/// Entries must keep keys and weights stable while stored, including through
/// interior mutability. Clone must preserve both and should be cheap and
/// allocation-free: fallible node allocation cannot catch a panicking clone. Weight's default is its
/// least value; use `()` for an ordinary ordered map without augmentation.
pub trait Entry: Clone {
    type Key: Copy + Ord;
    type Weight: Copy + Ord + Default;
    fn key(&self) -> Self::Key;
    fn weight(&self) -> Self::Weight {
        Self::Weight::default()
    }
}

#[derive(Debug)]
pub enum Error<E> {
    Account(E),
    Allocation,
}

type Link<T, A> = Option<FallibleArc<Node<T, A>>>;
type Result<T, A> = core::result::Result<T, Error<<A as NodeAccount>::Error>>;

struct Node<T: Entry, A: NodeAccount> {
    value: T,
    left: Link<T, A>,
    right: Link<T, A>,
    height: u32,
    maximum: T::Weight,
    _charge: A::Charge,
}

pub struct PersistentAvl<T: Entry, A: NodeAccount> {
    root: Link<T, A>,
}

impl<T: Entry, A: NodeAccount> Clone for PersistentAvl<T, A> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
        }
    }
}

fn height<T: Entry, A: NodeAccount>(root: &Link<T, A>) -> u32 {
    root.as_ref().map_or(0, |node| node.height)
}
fn maximum<T: Entry, A: NodeAccount>(root: &Link<T, A>) -> T::Weight {
    root.as_ref()
        .map_or(T::Weight::default(), |node| node.maximum)
}

impl<T: Entry, A: NodeAccount> Node<T, A> {
    fn make(
        value: T,
        left: Link<T, A>,
        right: Link<T, A>,
        account: &A,
    ) -> Result<FallibleArc<Self>, A> {
        let charge = account
            .try_charge(FallibleArc::<Self>::allocation_size())
            .map_err(Error::Account)?;
        FallibleArc::try_new(Self {
            height: 1 + height(&left).max(height(&right)),
            maximum: value.weight().max(maximum(&left)).max(maximum(&right)),
            value,
            left,
            right,
            _charge: charge,
        })
        .map_err(|_| Error::Allocation)
    }

    fn balanced(
        value: T,
        left: Link<T, A>,
        right: Link<T, A>,
        account: &A,
    ) -> Result<FallibleArc<Self>, A> {
        if height(&left) > height(&right) + 1 {
            let Some(l) = left.as_ref() else {
                tree_invariant_failure();
            };
            if height(&l.left) >= height(&l.right) {
                let right = Self::make(value, l.right.clone(), right, account)?;
                return Self::make(l.value.clone(), l.left.clone(), Some(right), account);
            }
            let Some(middle) = l.right.as_ref() else {
                tree_invariant_failure();
            };
            let new_left = Self::make(
                l.value.clone(),
                l.left.clone(),
                middle.left.clone(),
                account,
            )?;
            let new_right = Self::make(value, middle.right.clone(), right, account)?;
            return Self::make(
                middle.value.clone(),
                Some(new_left),
                Some(new_right),
                account,
            );
        }
        if height(&right) > height(&left) + 1 {
            let Some(r) = right.as_ref() else {
                tree_invariant_failure();
            };
            if height(&r.right) >= height(&r.left) {
                let left = Self::make(value, left, r.left.clone(), account)?;
                return Self::make(r.value.clone(), Some(left), r.right.clone(), account);
            }
            let Some(middle) = r.left.as_ref() else {
                tree_invariant_failure();
            };
            let new_left = Self::make(value, left, middle.left.clone(), account)?;
            let new_right = Self::make(
                r.value.clone(),
                middle.right.clone(),
                r.right.clone(),
                account,
            )?;
            return Self::make(
                middle.value.clone(),
                Some(new_left),
                Some(new_right),
                account,
            );
        }
        Self::make(value, left, right, account)
    }

    fn insert(root: &Link<T, A>, value: T, account: &A) -> Result<FallibleArc<Self>, A> {
        let Some(node) = root else {
            return Self::make(value, None, None, account);
        };
        match value.key().cmp(&node.value.key()) {
            core::cmp::Ordering::Less => Self::balanced(
                node.value.clone(),
                Some(Self::insert(&node.left, value, account)?),
                node.right.clone(),
                account,
            ),
            core::cmp::Ordering::Greater => Self::balanced(
                node.value.clone(),
                node.left.clone(),
                Some(Self::insert(&node.right, value, account)?),
                account,
            ),
            core::cmp::Ordering::Equal => {
                Self::make(value, node.left.clone(), node.right.clone(), account)
            }
        }
    }

    fn remove(root: &Link<T, A>, key: T::Key, account: &A) -> Result<Link<T, A>, A> {
        let Some(node) = root else { return Ok(None) };
        let replacement = match key.cmp(&node.value.key()) {
            core::cmp::Ordering::Less => Self::balanced(
                node.value.clone(),
                Self::remove(&node.left, key, account)?,
                node.right.clone(),
                account,
            )?,
            core::cmp::Ordering::Greater => Self::balanced(
                node.value.clone(),
                node.left.clone(),
                Self::remove(&node.right, key, account)?,
                account,
            )?,
            core::cmp::Ordering::Equal => match (&node.left, &node.right) {
                (None, _) => return Ok(node.right.clone()),
                (_, None) => return Ok(node.left.clone()),
                (_, Some(right)) => {
                    let mut successor = &**right;
                    while let Some(left) = &successor.left {
                        successor = left;
                    }
                    Self::balanced(
                        successor.value.clone(),
                        node.left.clone(),
                        Self::remove(&node.right, successor.value.key(), account)?,
                        account,
                    )?
                }
            },
        };
        Ok(Some(replacement))
    }

    // The key bound admits one boundary path. Entire out-of-bound or too-small
    // subtrees are pruned, and the first qualifying result ends the search.
    fn first(root: &Link<T, A>, lower: T::Key, weight: T::Weight) -> Option<&T> {
        let node = root.as_ref()?;
        if node.maximum < weight {
            return None;
        }
        if node.value.key() < lower {
            return Self::first(&node.right, lower, weight);
        }
        Self::first(&node.left, lower, weight)
            .or_else(|| (node.value.weight() >= weight).then_some(&node.value))
            .or_else(|| Self::first(&node.right, lower, weight))
    }

    fn last(root: &Link<T, A>, upper: T::Key, weight: T::Weight) -> Option<&T> {
        let node = root.as_ref()?;
        if node.maximum < weight {
            return None;
        }
        if node.value.key() > upper {
            return Self::last(&node.left, upper, weight);
        }
        Self::last(&node.right, upper, weight)
            .or_else(|| (node.value.weight() >= weight).then_some(&node.value))
            .or_else(|| Self::last(&node.left, upper, weight))
    }
}

impl<T: Entry, A: NodeAccount> Default for PersistentAvl<T, A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Entry, A: NodeAccount> PersistentAvl<T, A> {
    pub const fn new() -> Self {
        Self { root: None }
    }
    pub fn get(&self, key: T::Key) -> Option<&T> {
        let mut root = self.root.as_ref();
        while let Some(node) = root {
            match key.cmp(&node.value.key()) {
                core::cmp::Ordering::Less => root = node.left.as_ref(),
                core::cmp::Ordering::Greater => root = node.right.as_ref(),
                core::cmp::Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }
    /// First key at or above `lower` whose weight is at least `weight`.
    pub fn first(&self, lower: T::Key, weight: T::Weight) -> Option<&T> {
        Node::first(&self.root, lower, weight)
    }
    /// Last key at or below `upper` whose weight is at least `weight`.
    pub fn last(&self, upper: T::Key, weight: T::Weight) -> Option<&T> {
        Node::last(&self.root, upper, weight)
    }
    /// Return a new snapshot, replacing an equal key. Failure preserves self.
    pub fn insert(&self, value: T, account: &A) -> Result<Self, A> {
        Ok(Self {
            root: Some(Node::insert(&self.root, value, account)?),
        })
    }
    /// Return a snapshot without `key`; absent keys leave contents unchanged.
    pub fn remove(&self, key: T::Key, account: &A) -> Result<Self, A> {
        Ok(Self {
            root: Node::remove(&self.root, key, account)?,
        })
    }
}

#[cold]
fn tree_invariant_failure() -> ! {
    crate::debug::invariant_failure("persistent AVL balance invariant")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(Default)]
    struct Stats {
        calls: AtomicUsize,
        fail: AtomicUsize,
        live: AtomicUsize,
        probes: AtomicUsize,
    }
    #[derive(Clone, Default)]
    struct Account(Arc<Stats>);
    struct Charge(Arc<Stats>);
    impl Drop for Charge {
        fn drop(&mut self) {
            self.0.live.fetch_sub(1, Ordering::Relaxed);
        }
    }
    impl NodeAccount for Account {
        type Charge = Charge;
        type Error = ();
        fn try_charge(&self, _: usize) -> core::result::Result<Charge, ()> {
            let call = self.0.calls.fetch_add(1, Ordering::Relaxed) + 1;
            if self.0.fail.load(Ordering::Relaxed) == call {
                return Err(());
            }
            self.0.live.fetch_add(1, Ordering::Relaxed);
            Ok(Charge(self.0.clone()))
        }
    }
    #[derive(Clone)]
    struct Value {
        key: u64,
        size: u64,
        stats: Arc<Stats>,
    }
    impl Entry for Value {
        type Key = u64;
        type Weight = u64;
        fn key(&self) -> u64 {
            self.stats.probes.fetch_add(1, Ordering::Relaxed);
            self.key
        }
        fn weight(&self) -> u64 {
            self.stats.probes.fetch_add(1, Ordering::Relaxed);
            self.size
        }
    }
    fn value(key: u64, size: u64, account: &Account) -> Value {
        Value {
            key,
            size,
            stats: account.0.clone(),
        }
    }
    fn verify(
        root: &Link<Value, Account>,
        lower: Option<u64>,
        upper: Option<u64>,
    ) -> (u32, u64, usize) {
        let Some(node) = root else { return (0, 0, 0) };
        assert!(lower.is_none_or(|bound| node.value.key > bound));
        assert!(upper.is_none_or(|bound| node.value.key < bound));
        let (lh, lm, lc) = verify(&node.left, lower, Some(node.value.key));
        let (rh, rm, rc) = verify(&node.right, Some(node.value.key), upper);
        assert!(lh.abs_diff(rh) <= 1);
        assert_eq!(node.height, 1 + lh.max(rh));
        assert_eq!(node.maximum, node.value.size.max(lm).max(rm));
        (node.height, node.maximum, lc + rc + 1)
    }

    #[test]
    fn custom_ordered_keys_need_no_numeric_weight() {
        #[derive(Clone)]
        struct Named((&'static str, u16));
        impl Entry for Named {
            type Key = (&'static str, u16);
            type Weight = ();
            fn key(&self) -> Self::Key {
                self.0
            }
        }
        let account = Account::default();
        let tree = PersistentAvl::new();
        let tree = crate::require_ok(tree.insert(Named(("beta", 2)), &account));
        let tree = crate::require_ok(tree.insert(Named(("alpha", 1)), &account));
        assert_eq!(tree.first(("", 0), ()).map(Entry::key), Some(("alpha", 1)));
        assert_eq!(tree.last(("z", 0), ()).map(Entry::key), Some(("beta", 2)));
        assert!(tree.get(("beta", 2)).is_some());
    }

    #[test]
    fn persistent_avl_preserves_order_balance_augmentation_and_snapshots() {
        let account = Account::default();
        let mut tree: PersistentAvl<Value, Account> = PersistentAvl::new();
        let mut expected = BTreeMap::new();
        let mut seed = 1_u64;
        for _ in 0..4000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let key = (seed >> 32) % 128;
            let old = tree.clone();
            let before = tree.get(key).map(|v| v.size);
            if seed & 3 == 0 {
                tree = crate::require_ok(tree.remove(key, &account));
                expected.remove(&key);
            } else {
                let size = (seed >> 16) % 33;
                tree = crate::require_ok(tree.insert(value(key, size, &account), &account));
                expected.insert(key, size);
            }
            assert_eq!(old.get(key).map(|v| v.size), before);
            assert_eq!(verify(&tree.root, None, None).2, expected.len());
            let bound = (seed >> 8) % 130;
            let minimum = (seed >> 24) % 35;
            let first = expected
                .range(bound..)
                .find(|(_, size)| **size >= minimum)
                .map(|(&k, _)| k);
            let last = expected
                .range(..=bound)
                .rev()
                .find(|(_, size)| **size >= minimum)
                .map(|(&k, _)| k);
            assert_eq!(tree.first(bound, minimum).map(|v| v.key), first);
            assert_eq!(tree.last(bound, minimum).map(|v| v.key), last);
        }
        // Remove every entry, exercising both successor replacement and
        // rotations while shrinking. Each displaced snapshot is still valid.
        for key in expected.keys().copied().collect::<std::vec::Vec<_>>() {
            tree = crate::require_ok(tree.remove(key, &account));
            verify(&tree.root, None, None);
        }
        assert!(tree.root.is_none());
        assert_eq!(account.0.live.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn index_work_and_path_allocations_stay_logarithmic() {
        let account = Account::default();
        let mut tree: PersistentAvl<Value, Account> = PersistentAvl::new();
        for key in 0..4096 {
            let height = height(&tree.root) as usize;
            let before = account.0.calls.load(Ordering::Relaxed);
            tree = crate::require_ok(tree.insert(value(key, key % 17, &account), &account));
            assert!(account.0.calls.load(Ordering::Relaxed) - before <= 3 * (height + 1));
        }
        let depth = height(&tree.root) as usize;
        assert!(depth <= 16);
        for bound in 0..4098 {
            account.0.probes.store(0, Ordering::Relaxed);
            tree.first(bound, 16);
            tree.last(bound, 16);
            assert!(account.0.probes.load(Ordering::Relaxed) <= 8 * depth);
        }
        for key in 0..4096 {
            let before = account.0.calls.load(Ordering::Relaxed);
            let depth = height(&tree.root) as usize;
            tree = crate::require_ok(tree.remove(key, &account));
            assert!(account.0.calls.load(Ordering::Relaxed) - before <= 3 * depth);
        }
        assert_eq!(account.0.live.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn every_failed_path_copy_releases_charges_and_preserves_old_root() {
        let account = Account::default();
        let mut tree: PersistentAvl<Value, Account> = PersistentAvl::new();
        for key in 0..63 {
            tree = crate::require_ok(tree.insert(value(key, key, &account), &account));
        }
        for remove in [false, true] {
            let baseline = account.0.live.load(Ordering::Relaxed);
            let mut succeeded = false;
            for fail in 1..100 {
                account.0.fail.store(
                    account.0.calls.load(Ordering::Relaxed) + fail,
                    Ordering::Relaxed,
                );
                let result = if remove {
                    tree.remove(31, &account)
                } else {
                    tree.insert(value(64, 100, &account), &account)
                };
                succeeded = result.is_ok();
                drop(result);
                assert_eq!(account.0.live.load(Ordering::Relaxed), baseline);
                assert!(tree.get(31).is_some());
                assert!(tree.get(64).is_none());
                verify(&tree.root, None, None);
                if succeeded {
                    break;
                }
            }
            assert!(succeeded);
            account.0.fail.store(0, Ordering::Relaxed);
        }
        drop(tree);
        assert_eq!(account.0.live.load(Ordering::Relaxed), 0);
    }
}
