// Tests for table:Activities — schema rules from the DDL, grounded in observed data.
//
// DDL:
//   activityid  SERIAL  PK (activities_pkey)
//   customerid  INTEGER NOT NULL  FK -> Customers ON DELETE CASCADE
//   contactid   INTEGER  (nullable) FK -> Contacts
//   repid       INTEGER NOT NULL  FK -> SalesReps
//   activitydate  TIMESTAMP (nullable, indexed)
//   activitytype  VARCHAR(20) (nullable)
//   subject       VARCHAR(150) (nullable)
//   details       TEXT (nullable)
//   followupdate  TIMESTAMP (nullable, indexed)
//   completed     BOOLEAN NOT NULL
//
// Observed rows (activityid, customerid, contactid, repid, type, subject, completed, followupdate):
//   1, 1, 1, 1, "Meeting", "Site walk at Reservoir 2", 1, NULL
//   2, 2, 2, 2, "Call",    "Expansion to Dock 4",      1, NULL
//   3, 3, 3, 1, "Call",    "Vibration pilot results",  0, 2026-09-20
//   4, 4, 4, 2, "Meeting", "AgTech expo follow-up",    0, 2026-10-02
//   5, 5, 5, 3, "Email",   "Order confirmation",       1, NULL
//   6, 2, 2, 2, "Note",    "Balance due",              0, 2026-10-21
//
// Confirmed existing FK targets: customers 1,2,3,4,5,100,1000; contacts 1,2,3,4,5,219; reps 1,2,3.
// Confirmed non-existent: customer 999999, rep 999999.

use super::activities::*;

fn act(c: i32, k: Option<i32>, r: i32, t: &str, s: &str, done: bool) -> Activity {
    Activity {
        customerid: c,
        contactid: k,
        repid: r,
        activitydate: None,
        activitytype: t.to_string(),
        subject: s.to_string(),
        details: None,
        followupdate: None,
        completed: done,
    }
}

// 1. A fully-populated row with existing FK targets is accepted (row 1).
#[test]
fn insert_valid_activity_succeeds() {
    let a = act(1, Some(1), 1, "Meeting", "Site walk at Reservoir 2", true);
    assert!(insert_activity(&a).is_ok());
}

// 2. customerid is NOT NULL: a null customer is rejected.
#[test]
fn insert_null_customer_rejected() {
    let mut a = act(1, Some(1), 1, "Call", "x", false);
    a.customerid = 0; // stand-in for NULL
    assert!(insert_activity(&a).is_err());
}

// 3. customerid FK: a non-existent customer (999999 confirmed absent) is rejected.
#[test]
fn insert_nonexistent_customer_rejected() {
    let a = act(999999, Some(1), 1, "Call", "x", false);
    assert!(insert_activity(&a).is_err());
}

// 4. repid is NOT NULL: a null rep is rejected.
#[test]
fn insert_null_rep_rejected() {
    let mut a = act(1, Some(1), 1, "Call", "x", false);
    a.repid = 0; // stand-in for NULL
    assert!(insert_activity(&a).is_err());
}

// 5. repid FK: a non-existent rep (999999 confirmed absent) is rejected.
#[test]
fn insert_nonexistent_rep_rejected() {
    let a = act(1, Some(1), 999999, "Call", "x", false);
    assert!(insert_activity(&a).is_err());
}

// 6. contactid is nullable: NULL is accepted and stored as None.
#[test]
fn insert_null_contact_accepted() {
    let a = act(1, None, 1, "Call", "x", false);
    assert!(insert_activity(&a).is_ok());
}

// 7. contactid FK: a non-existent contact is rejected.
#[test]
fn insert_nonexistent_contact_rejected() {
    let a = act(1, Some(999999), 1, "Call", "x", false);
    assert!(insert_activity(&a).is_err());
}

// 8. activitytype VARCHAR(20): 20 chars accepted, 21 rejected.
#[test]
fn activitytype_length_limit_20() {
    let ok = act(1, None, 1, &"a".repeat(20), "x", false);
    assert!(insert_activity(&ok).is_ok());
    let bad = act(1, None, 1, &"a".repeat(21), "x", false);
    assert!(insert_activity(&bad).is_err());
}

// 9. subject VARCHAR(150): 150 chars accepted, 151 rejected.
#[test]
fn subject_length_limit_150() {
    let ok = act(1, None, 1, "Call", &"s".repeat(150), false);
    assert!(insert_activity(&ok).is_ok());
    let bad = act(1, None, 1, "Call", &"s".repeat(151), false);
    assert!(insert_activity(&bad).is_err());
}

// 10. completed BOOLEAN NOT NULL: both 0 and 1 are accepted (rows show both).
#[test]
fn completed_accepts_both_values() {
    assert!(insert_activity(&act(1, None, 1, "Call", "x", true)).is_ok());
    assert!(insert_activity(&act(1, None, 1, "Call", "x", false)).is_ok());
}

// 11. activitydate nullable: NULL accepted, a timestamp accepted (row 1: NULL; row 3: 2026-09-20).
#[test]
fn activitydate_nullable() {
    let mut a = act(1, None, 1, "Call", "x", false);
    a.activitydate = None;
    assert!(insert_activity(&a).is_ok());
    a.activitydate = Some("2026-09-20 09:00:00".to_string());
    assert!(insert_activity(&a).is_ok());
}

// 12. followupdate nullable: NULL accepted (rows 1,2,5), a timestamp accepted (row 6: 2026-10-21).
#[test]
fn followupdate_nullable() {
    let mut a = act(1, None, 1, "Note", "x", false);
    a.followupdate = None;
    assert!(insert_activity(&a).is_ok());
    a.followupdate = Some("2026-10-21 00:00:00".to_string());
    assert!(insert_activity(&a).is_ok());
}

// 13. activityid is SERIAL: two inserts get distinct, increasing ids.
#[test]
fn activityid_auto_increments() {
    let id1 = insert_activity(&act(1, None, 1, "Call", "a", false)).unwrap();
    let id2 = insert_activity(&act(1, None, 1, "Call", "b", false)).unwrap();
    assert!(id2 > id1);
}

// 14. customerid FK ON DELETE CASCADE: deleting customer 2 removes its
//     activities (2 and 6) but leaves customer 1's (1) and customer 5's (5).
#[test]
fn delete_customer_cascades_to_its_activities() {
    assert!(delete_customer(2).is_ok());
    assert!(get_activity(2).is_none());
    assert!(get_activity(6).is_none());
    assert!(get_activity(1).is_some());
    assert!(get_activity(5).is_some());
}

// 15. Deleting a customer with no activities succeeds (no cascade work).
#[test]
fn delete_customer_with_no_activities_succeeds() {
    assert!(delete_customer(1000).is_ok());
}
