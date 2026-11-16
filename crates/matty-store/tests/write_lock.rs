//! Two connections writing one database wait for each other instead of
//! losing a batch (#929).
//!
//! A closed main window left its sync engine running, so a reopened window
//! had two engines writing the same file. Their deferred transactions read
//! first and asked for the write lock later; SQLite answers that with
//! "database is locked" at once once the other has written, and every
//! server event in the batch was dropped.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Barrier};
use std::time::Duration;

use matty_store::Store;

fn count(store: &Store) -> i64 {
    store
        .conn()
        .query_row("SELECT count(*) FROM probe", [], |row| row.get(0))
        .unwrap()
}

#[test]
fn a_second_writer_waits_for_the_first_and_both_land() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("account.sqlite");
    let mut first = Store::open(&path).unwrap();
    first
        .conn()
        .execute_batch("CREATE TABLE probe (n INTEGER)")
        .unwrap();
    let mut second = Store::open(&path).unwrap();
    let started = Arc::new(Barrier::new(2));
    let other = std::thread::spawn({
        let started = started.clone();
        move || {
            started.wait();
            // Begins while the first holds the lock; must wait, not fail.
            let tx = second.write().unwrap();
            let seen: i64 = tx
                .query_row("SELECT count(*) FROM probe", [], |row| row.get(0))
                .unwrap();
            tx.execute("INSERT INTO probe (n) VALUES (2)", []).unwrap();
            tx.commit().unwrap();
            seen
        }
    });
    let tx = first.write().unwrap();
    tx.query_row("SELECT count(*) FROM probe", [], |row| row.get::<_, i64>(0))
        .unwrap();
    started.wait();
    std::thread::sleep(Duration::from_millis(200));
    tx.execute("INSERT INTO probe (n) VALUES (1)", []).unwrap();
    tx.commit().unwrap();
    // The second saw the first's row: it began only after the first ended.
    assert_eq!(other.join().unwrap(), 1);
    assert_eq!(count(&first), 2);
}

#[test]
fn a_deferred_transaction_loses_its_batch_which_is_why_writes_take_the_lock_first() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("account.sqlite");
    let mut first = Store::open(&path).unwrap();
    first
        .conn()
        .execute_batch("CREATE TABLE probe (n INTEGER)")
        .unwrap();
    let second = Store::open(&path).unwrap();
    let tx = first.conn_mut().transaction().unwrap();
    tx.query_row("SELECT count(*) FROM probe", [], |row| row.get::<_, i64>(0))
        .unwrap();
    second
        .conn()
        .execute("INSERT INTO probe (n) VALUES (2)", [])
        .unwrap();
    let started = std::time::Instant::now();
    let error = tx
        .execute("INSERT INTO probe (n) VALUES (1)", [])
        .unwrap_err();
    assert!(error.to_string().contains("locked"), "{error}");
    // At once, not after the busy timeout.
    assert!(started.elapsed() < Duration::from_secs(1));
}
