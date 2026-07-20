use std::sync::Arc;

use super::*;

#[test]
fn replacement_refreshes_recency_and_one_over_evicts_the_lru_key() {
    let mut cache = DeterministicLruCache::new(3);
    assert_eq!(cache.insert(2_u16, "two"), CacheInsertOutcome::Inserted);
    assert_eq!(cache.insert(1, "one"), CacheInsertOutcome::Inserted);
    assert_eq!(cache.insert(3, "three"), CacheInsertOutcome::Inserted);

    assert_eq!(
        cache.insert(2, "two-replaced"),
        CacheInsertOutcome::Replaced
    );
    assert_eq!(
        cache.insert(4, "four"),
        CacheInsertOutcome::InsertedAfterEviction
    );

    assert!(!cache.entries.contains_key(&1));
    assert_eq!(cache.get(&2), Some("two-replaced"));
    assert_eq!(cache.entries.len(), 3);
    assert_eq!(cache.snapshot().replacements, 1);
    assert_eq!(cache.snapshot().evictions, 1);
}

#[test]
fn equal_recency_evicts_the_lexicographically_smallest_key() {
    let mut cache = DeterministicLruCache::new(2);
    cache.insert(2_u16, "two");
    cache.insert(1, "one");
    cache.entries.get_mut(&1).expect("first entry").last_access = 1;
    cache.entries.get_mut(&2).expect("second entry").last_access = 1;

    assert_eq!(
        cache.insert(3, "three"),
        CacheInsertOutcome::InsertedAfterEviction
    );

    assert!(!cache.entries.contains_key(&1));
    assert!(cache.entries.contains_key(&2));
    assert!(cache.entries.contains_key(&3));
}

#[test]
fn production_capacity_is_inclusive_and_access_preserves_the_recent_entry() {
    let mut cache = DeterministicLruCache::new(SIGNATURE_CACHE_CAPACITY);
    for key in 0..SIGNATURE_CACHE_CAPACITY {
        cache.insert(key, key);
    }
    assert_eq!(cache.entries.len(), SIGNATURE_CACHE_CAPACITY);
    assert_eq!(cache.get(&0), Some(0));

    assert_eq!(
        cache.insert(SIGNATURE_CACHE_CAPACITY, SIGNATURE_CACHE_CAPACITY),
        CacheInsertOutcome::InsertedAfterEviction
    );

    assert_eq!(cache.entries.len(), SIGNATURE_CACHE_CAPACITY);
    assert!(cache.entries.contains_key(&0));
    assert!(!cache.entries.contains_key(&1));
}

#[test]
fn access_clock_overflow_clears_and_restarts_at_one() {
    let mut cache = DeterministicLruCache::new(2);
    cache.insert(1_u16, "one");
    cache.access_clock = u64::MAX;

    assert_eq!(
        cache.insert(2, "two"),
        CacheInsertOutcome::InsertedAfterClockReset
    );

    assert_eq!(cache.entries.len(), 1);
    assert_eq!(cache.get(&2), Some("two"));
    let snapshot = cache.snapshot();
    assert_eq!(snapshot.clock_resets, 1);
    assert_eq!(snapshot.access_clock, 2);
}

#[test]
fn overflow_during_hit_becomes_a_miss_and_discards_old_entries() {
    let mut cache = DeterministicLruCache::new(2);
    cache.insert(1_u16, "one");
    cache.access_clock = u64::MAX;

    assert_eq!(cache.get(&1), None);

    let snapshot = cache.snapshot();
    assert_eq!(snapshot.entries, 0);
    assert_eq!(snapshot.misses, 1);
    assert_eq!(snapshot.clock_resets, 1);
}

#[test]
fn predicate_eviction_removes_only_matching_typed_keys() {
    let mut cache = DeterministicLruCache::new(4);
    for key in 0_u16..4 {
        cache.insert(key, key);
    }

    assert_eq!(cache.remove_where(|key| key % 2 == 0), 2);

    assert_eq!(
        cache.entries.keys().copied().collect::<Vec<_>>(),
        vec![1, 3]
    );
}

#[test]
fn poisoned_mutex_clears_once_and_accepts_future_operations() {
    let caches = Arc::new(ProfileSemanticCaches::default());
    let poisoned = Arc::clone(&caches);
    assert!(
        std::thread::spawn(move || {
            let _guard = poisoned.signature_help.lock().expect("initial cache lock");
            panic!("poison signature cache");
        })
        .join()
        .is_err()
    );

    let first = caches.signature_snapshot_for_test();
    let second = caches.signature_snapshot_for_test();

    assert_eq!(first.entries, 0);
    assert_eq!(first.poison_recoveries, 1);
    assert_eq!(second.poison_recoveries, 1);
}
