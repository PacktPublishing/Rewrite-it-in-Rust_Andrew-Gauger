// Tests for vba:CairnForms.ActivityDefaults (CairnForms L730-734).
//
//   730: ' Activities tab on frmCustomers: default the rep to the customer's rep
//   731: Public Function ActivityDefaults(frm As Form)
//   732:     On Error Resume Next
//   733:     If IsNull(frm!repID) Then frm!repID = frm.Parent!AssignedRepID
//   734: End Function
//
// Runs for the Activities tab/subform on frmCustomers. The assumed function
// mirrors the VBA: it takes the activity form (by reference, as VBA object
// arguments are) and applies the default in place.
//
// Schema context: activities.repid is INTEGER NOT NULL (FK to SalesReps),
// Customers.assignedrepid is nullable — a customer with no assigned rep
// yields a null default; the user must pick a rep before saving.

/// Stand-in for the parent customer form's AssignedRepID control.
#[derive(Clone, Copy, Debug, PartialEq)]
enum ParentState {
    /// frm.Parent is a customer form with the control present.
    Customer { assigned_rep_id: Option<i32> },
    /// frm.Parent is Nothing (parent form unavailable).
    NoParent,
    /// Parent form exists but has no AssignedRepID control.
    MissingControl,
}

/// Stand-in for the Activities subform (the `frm` argument).
#[derive(Debug)]
struct ActivityForm {
    rep_id: Option<i32>,
    parent: ParentState,
}

impl ActivityForm {
    fn new(rep_id: Option<i32>, parent: ParentState) -> Self {
        ActivityForm { rep_id, parent }
    }
}

// Assumed: fn activity_defaults(frm: &mut ActivityForm)
// Mirrors L732-733: On Error Resume Next; if repID is null, set it to the
// parent's AssignedRepID. Never raises.

#[test]
fn null_rep_defaults_to_parent_assigned_rep() {
    // L733: IsNull(frm!repID) is true -> frm!repID = frm.Parent!AssignedRepID.
    // Parent customer's AssignedRepID is 7 -> repID becomes 7.
    let mut frm = ActivityForm::new(None, ParentState::Customer { assigned_rep_id: Some(7) });
    activity_defaults(&mut frm);
    assert_eq!(frm.rep_id, Some(7));
}

#[test]
fn non_null_rep_is_left_unchanged() {
    // L733 guard: repID is already 3 and the parent's rep is 7 -> stays 3.
    let mut frm = ActivityForm::new(Some(3), ParentState::Customer { assigned_rep_id: Some(7) });
    activity_defaults(&mut frm);
    assert_eq!(frm.rep_id, Some(3));
}

#[test]
fn null_parent_rep_yields_null_rep() {
    // Customer with no assigned rep: repID is set to null (no effective
    // change); the user must pick a rep before saving (activities.repid NOT NULL).
    let mut frm = ActivityForm::new(None, ParentState::Customer { assigned_rep_id: None });
    activity_defaults(&mut frm);
    assert_eq!(frm.rep_id, None);
}

#[test]
fn unavailable_parent_is_swallowed_and_rep_unchanged() {
    // L732 On Error Resume Next: frm.Parent is Nothing, so evaluating the
    // RHS raises; the error is swallowed, the assignment never happens, and
    // repID stays as it was. The function must not raise.
    let mut frm = ActivityForm::new(None, ParentState::NoParent);
    activity_defaults(&mut frm);
    assert_eq!(frm.rep_id, None);
}

#[test]
fn missing_parent_control_is_swallowed_and_rep_unchanged() {
    // L732: the parent form lacks the AssignedRepID control, so the RHS
    // raises; the error is swallowed and repID stays as it was. Must not raise.
    let mut frm = ActivityForm::new(None, ParentState::MissingControl);
    activity_defaults(&mut frm);
    assert_eq!(frm.rep_id, None);
}
