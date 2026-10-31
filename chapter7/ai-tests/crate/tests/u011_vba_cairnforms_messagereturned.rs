// Tests for vba:CairnForms.MessageReturned (CairnForms L748-752).
//
// The VBA procedure takes a Form bound to a PhoneMessages record and:
//   (1) sets the form's MessageStatus field to the literal "Returned"  (L749)
//   (2) stamps ReturnedAt with Now ONLY if ReturnedAt is currently Null;
//       an existing value is left unchanged                              (L750)
//   (3) commits the record by clearing the Dirty flag                    (L751)
//
// Schema (phonemessages): messagestatus VARCHAR(20) nullable,
// returnedat TIMESTAMP nullable -> the null-check in (2) is meaningful.
//
// `message_returned` is assumed to exist (provided by the system under test);
// we call it but do not implement it. `returned_at` uses i64 as a stand-in
// for a TIMESTAMP so the "preserve existing value" rule is checkable by
// equality; the exact "now" value is not a business rule, so we only assert
// that a null stamp becomes Some.

#[derive(Clone, Debug, PartialEq)]
struct PhoneMessageForm {
    message_status: Option<String>, // PhoneMessages.messagestatus VARCHAR(20)
    returned_at: Option<i64>,       // PhoneMessages.returnedat TIMESTAMP (nullable)
    dirty: bool,                    // form.Dirty
}

// Assumed to exist: mirrors CairnForms.MessageReturned(frm As Form).
// fn message_returned(frm: &mut PhoneMessageForm);

#[test]
fn message_returned_sets_status_to_returned() {
    // L749: frm!MessageStatus = "Returned"  (unconditional literal)
    let mut frm = PhoneMessageForm {
        message_status: Some("Pending".to_string()),
        returned_at: None,
        dirty: true,
    };
    message_returned(&mut frm);
    assert_eq!(
        frm.message_status.as_deref(),
        Some("Returned"),
        "MessageStatus must be set to the literal \"Returned\""
    );
}

#[test]
fn message_returned_stamps_returned_at_when_null() {
    // L750: If IsNull(frm!ReturnedAt) Then frm!ReturnedAt = Now
    let mut frm = PhoneMessageForm {
        message_status: None,
        returned_at: None, // currently Null
        dirty: true,
    };
    message_returned(&mut frm);
    assert!(
        frm.returned_at.is_some(),
        "ReturnedAt must be stamped with the current time when it was Null"
    );
}

#[test]
fn message_returned_preserves_existing_returned_at() {
    // L750: the assignment is guarded by IsNull, so a pre-existing value
    // must be left unchanged.
    let original = 1_700_000_000i64;
    let mut frm = PhoneMessageForm {
        message_status: Some("Pending".to_string()),
        returned_at: Some(original), // already set
        dirty: true,
    };
    message_returned(&mut frm);
    assert_eq!(
        frm.returned_at,
        Some(original),
        "an existing ReturnedAt must be left unchanged (not re-stamped)"
    );
}

#[test]
fn message_returned_clears_dirty_flag() {
    // L751: frm.Dirty = False  (commits the record)
    let mut frm = PhoneMessageForm {
        message_status: None,
        returned_at: None,
        dirty: true,
    };
    message_returned(&mut frm);
    assert!(
        !frm.dirty,
        "the record must be committed by clearing the Dirty flag"
    );
}
