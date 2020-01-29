use bytes::*;
use skiplist::*;
use std::str;
use std::sync::atomic::*;
use std::sync::*;
use std::time::Duration;
use yatp::task::callback::Handle;

const ARENA_SIZE: u32 = 1 << 20;

fn new_value(v: usize) -> Bytes {
    Bytes::from(format!("{:05}", v))
}

#[test]
fn test_empty() {
    let key = b"aaa".to_vec();
    let list = Skiplist::with_capacity(ARENA_SIZE);
    let v = list.get(&key);
    assert!(v.is_none());

    let mut iter = list.iter_ref();
    assert!(!iter.valid());
    iter.seek_to_first();
    assert!(!iter.valid());
    iter.seek_to_last();
    assert!(!iter.valid());
    iter.seek(&key);
    assert!(!iter.valid());
    assert!(list.is_empty());
}

#[test]
fn test_basic() {
    let list = Skiplist::with_capacity(ARENA_SIZE);
    let table = vec![
        (b"key1" as &'static [u8], new_value(42)),
        (b"key2", new_value(52)),
        (b"key3", new_value(62)),
        (b"key5", Bytes::from(format!("{:0102400}", 1))),
        (b"key4", new_value(72)),
    ];

    for (key, value) in &table {
        list.put(Bytes::from_static(key), value.clone());
    }

    assert_eq!(list.get(b"key"), None);
    assert_eq!(list.len(), 5);
    assert!(!list.is_empty());
    for (key, value) in &table {
        let tag = unsafe { str::from_utf8_unchecked(key) };
        assert_eq!(list.get(key), Some(value), "{}", tag);
    }
}

fn test_concurrent_basic(n: usize, cap: u32, value_len: usize) {
    let pool = yatp::Builder::new("concurrent_basic").build_callback_pool();
    let list = Skiplist::with_capacity(cap);
    let kvs: Vec<_> = (0..n)
        .map(|i| {
            (
                Bytes::from(format!("{:05}", i)),
                Bytes::from(format!("{1:00$}", value_len, i)),
            )
        })
        .collect();
    let (tx, rx) = mpsc::channel();
    for (k, v) in kvs.clone() {
        let tx = tx.clone();
        let list = list.clone();
        pool.spawn(move |_: &mut Handle<'_>| {
            list.put(k, v);
            tx.send(()).unwrap();
        })
    }
    for _ in 0..n {
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
    }
    for (k, v) in kvs {
        let tx = tx.clone();
        let list = list.clone();
        pool.spawn(move |_: &mut Handle<'_>| {
            let val = list.get(&k);
            assert_eq!(val, Some(&v), "{:?}", k);
            tx.send(()).unwrap();
        });
    }
    for _ in 0..n {
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
    }
    assert_eq!(list.len(), n);
}

#[test]
fn test_concurrent_basic_small_value() {
    test_concurrent_basic(1000, ARENA_SIZE, 5);
}

#[test]
fn test_concurrent_basic_big_value() {
    test_concurrent_basic(100, 120 << 20, 1048576);
}

#[test]
fn test_one_key() {
    let n = 100;
    let write_pool = yatp::Builder::new("one_key").build_callback_pool();
    let read_pool = yatp::Builder::new("one_key").build_callback_pool();
    let list = Skiplist::with_capacity(ARENA_SIZE);
    let key = b"thekey";
    let (tx, rx) = mpsc::channel();
    for i in 0..n {
        let tx = tx.clone();
        let list = list.clone();
        let key = Bytes::from_static(key);
        let value = new_value(i);
        write_pool.spawn(move |_: &mut Handle<'_>| {
            list.put(key, value);
            tx.send(()).unwrap();
        })
    }
    let mark = Arc::new(AtomicBool::new(false));
    for _ in 0..n {
        let tx = tx.clone();
        let list = list.clone();
        let mark = mark.clone();
        read_pool.spawn(move |_: &mut Handle<'_>| {
            let val = list.get(key);
            if val.is_none() {
                return;
            }
            let s = unsafe { str::from_utf8_unchecked(val.unwrap()) };
            let val: usize = s.parse().unwrap();
            assert!(val < n);
            mark.store(true, Ordering::SeqCst);
            tx.send(()).unwrap();
        });
    }
    for _ in 0..n {
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
        rx.recv_timeout(Duration::from_secs(3)).unwrap();
    }
    assert_eq!(list.len(), 1);
    assert!(mark.load(Ordering::SeqCst));
}

#[test]
fn test_iterator_next() {
    let n = 100;
    let list = Skiplist::with_capacity(ARENA_SIZE);
    let mut iter_ref = list.iter_ref();
    assert!(!iter_ref.valid());
    iter_ref.seek_to_first();
    assert!(!iter_ref.valid());
    for i in (0..n).rev() {
        let key = Bytes::from(format!("{:05}", i));
        list.put(key, new_value(i));
    }
    iter_ref.seek_to_first();
    for i in 0..n {
        assert!(iter_ref.valid());
        let v = iter_ref.value();
        assert_eq!(*v, new_value(i));
        iter_ref.next();
    }
    assert!(!iter_ref.valid());
}

#[test]
fn test_iterator_prev() {
    let n = 100;
    let list = Skiplist::with_capacity(ARENA_SIZE);
    let mut iter_ref = list.iter_ref();
    assert!(!iter_ref.valid());
    iter_ref.seek_to_last();
    assert!(!iter_ref.valid());
    for i in (0..n).rev() {
        let key = Bytes::from(format!("{:05}", i));
        list.put(key, new_value(i));
    }
    iter_ref.seek_to_last();
    for i in (0..n).rev() {
        assert!(iter_ref.valid());
        let v = iter_ref.value();
        assert_eq!(*v, new_value(i));
        iter_ref.prev();
    }
    assert!(!iter_ref.valid());
}

#[test]
fn test_iterator_seek() {
    let n = 100;
    let list = Skiplist::with_capacity(ARENA_SIZE);
    let mut iter_ref = list.iter_ref();
    assert!(!iter_ref.valid());
    iter_ref.seek_to_first();
    assert!(!iter_ref.valid());
    for i in (0..n).rev() {
        let v = i * 10 + 1000;
        let key = Bytes::from(format!("{:05}", v));
        list.put(key, new_value(v));
    }
    iter_ref.seek_to_first();
    assert!(iter_ref.valid());
    assert_eq!(iter_ref.value(), b"01000" as &[u8]);

    let cases = vec![
        (b"00000", Some(b"01000"), None),
        (b"01000", Some(b"01000"), Some(b"01000")),
        (b"01005", Some(b"01010"), Some(b"01000")),
        (b"01010", Some(b"01010"), Some(b"01010")),
        (b"99999", None, Some(b"01990")),
    ];
    for (key, seek_expect, for_prev_expect) in cases {
        iter_ref.seek(key);
        assert_eq!(iter_ref.valid(), seek_expect.is_some());
        if let Some(v) = seek_expect {
            assert_eq!(iter_ref.value(), &v[..]);
        }
        iter_ref.seek_for_prev(key);
        assert_eq!(iter_ref.valid(), for_prev_expect.is_some());
        if let Some(v) = for_prev_expect {
            assert_eq!(iter_ref.value(), &v[..]);
        }
    }
}