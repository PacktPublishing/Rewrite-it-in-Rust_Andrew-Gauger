// Tests for vba:CairnForms.RMADefaults (CairnForms L843-846).
//
// Rules pinned down:
//  1. If frm!RMANumber Is Null, it is set to NextNumber("RMA") (L844).
//     NextNumber (L671-681) builds stem = "RMA-" & Year(Date) & "-",
//     takes DMax of Val(Mid(RMANumber, Len(stem)+1)) over rows where
//     RMANumber Like stem & "*", and returns stem & Format(Nz(max,0)+1, "0000").
//     So the number is "RMA-YYYY-NNNN": 4-digit zero-padded sequence,
//     1 when no rows exist for the current year; only current-year rows count.
//  2. If frm!OpenedByRepID Is Null, it is set to TempVars("RepID") (L845).
//  3. Neither field is ever overwritten when it already holds a value.
//
// Assumed function (not implemented here):
//   fn rma_defaults(
//       rma_number: Option<&str>,        // frm!RMANumber (None = Null)
//       opened_by_rep_id: Option<i32>,   // frm!OpenedByRepID (None = Null)
//       rep_id: i32,                     // TempVars("RepID")
//       existing_numbers: &[&str],       // RMANumber values in table RMAs
//       year: i32,                       // Year(Date)
//   ) -> (String, i32)                   // resulting (RMANumber, OpenedByRepID)

#[test]
fn rma_number_defaults_to_first_of_year_when_null_and_no_rows() {
    // No RMAs rows for 2026 -> Nz(max,0)+1 = 1 -> "RMA-2026-0001".
    let (num, _) = rma_defaults(None, None, 42, &[], 2026);
    assert_eq!(num, "RMA-2026-0001");
}

#[test]
fn rma_number_not_overwritten_when_already_set() {
    // L844 guards with IsNull: an existing value is left untouched.
    let (num, _) = rma_defaults(
        Some("RMA-2026-0007"),
        None,
        42,
        &["RMA-2026-0007"],
        2026,
    );
    assert_eq!(num, "RMA-2026-0007");
}

#[test]
fn next_rma_number_is_max_plus_one_zero_padded_for_current_year() {
    // DMax of Val(Mid(...)) over "RMA-2026-*" rows: max(7, 12) = 12 -> 13.
    let (num, _) = rma_defaults(
        None,
        None,
        42,
        &["RMA-2026-0007", "RMA-2026-0012"],
        2026,
    );
    assert_eq!(num, "RMA-2026-0013");
}

#[test]
fn rows_from_other_years_do_not_affect_sequence() {
    // The Like filter is "RMA-2026-*": a 2025 row is ignored, so the
    // sequence restarts at 0001 for the new year.
    let (num, _) = rma_defaults(None, None, 42, &["RMA-2025-0099"], 2026);
    assert_eq!(num, "RMA-2026-0001");
}

#[test]
fn opened_by_rep_defaults_to_login_rep_when_null() {
    // L845: Null OpenedByRepID takes TempVars("RepID") = 42.
    let (_, rep) = rma_defaults(None, None, 42, &[], 2026);
    assert_eq!(rep, 42);
}

#[test]
fn opened_by_rep_not_overwritten_when_already_set() {
    // L845 guards with IsNull: an existing rep is left untouched.
    let (_, rep) = rma_defaults(None, Some(7), 42, &[], 2026);
    assert_eq!(rep, 7);
}

#[test]
fn both_defaults_applied_together_on_fresh_form() {
    // A brand-new RMA form (both fields Null) gets the next number
    // and the logged-in rep in one call.
    let (num, rep) = rma_defaults(None, None, 42, &["RMA-2026-0001"], 2026);
    assert_eq!(num, "RMA-2026-0002");
    assert_eq!(rep, 42);
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
fn rma_defaults(_rma_number: Option<&str>, _opened_by_rep_id: Option<i32>, _rep_id: i32, _existing_numbers: &[&str], _year: i32) -> (String, i32) { todo!() }
