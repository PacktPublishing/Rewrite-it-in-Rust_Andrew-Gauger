// Tests for vba:CairnReports.CommissionPeriod (CairnReports L498-500)
//
// VBA source:
//   Public Function CommissionPeriod() As String
//       CommissionPeriod = IIf(Len(mPeriod), mPeriod, "All dates")
//   End Function
//
// mPeriod is a private module variable (L31, "month shown on the commission
// statement"). It is set to Format(d1, "mmmm yyyy") (e.g. "March 2025") in
// PrintCommission (L486) and the RMA summary (L444) just before opening the
// report, and reset to "" by ClearCommissionPeriod (L494-496), which is wired
// to every report's OnClose (L528). Report headers/footers display the value
// via =CommissionPeriod() (L208, L261, L279, L381, L409).
//
// Assumed pure function: commission_period(m_period: &str) -> String

#[test]
fn empty_period_returns_all_dates() {
    // After ClearCommissionPeriod (L494-496) sets mPeriod = "", a report
    // opened without a month must show the literal "All dates".
    assert_eq!(commission_period(""), "All dates");
}

#[test]
fn month_label_returned_verbatim() {
    // PrintCommission (L486) sets mPeriod = Format(d1, "mmmm yyyy");
    // the function must return it unchanged.
    assert_eq!(commission_period("March 2025"), "March 2025");
}

#[test]
fn single_space_is_nonempty_and_returned_verbatim() {
    // The branch is chosen by Len(mPeriod): VBA Len(" ") = 1 (truthy), so a
    // blank-but-non-empty label takes the mPeriod branch, not "All dates".
    assert_eq!(commission_period(" "), " ");
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
fn commission_period(_m_period: &str) -> String { todo!() }
