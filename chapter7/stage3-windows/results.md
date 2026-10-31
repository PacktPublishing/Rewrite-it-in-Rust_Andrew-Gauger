# Stage 3 Windows results

- Date run: 2026-10-28
- Access version: Microsoft Access for Microsoft 365, 64-bit, build 16.0.20326.20232. This was read from MSACCESS.EXE (`C:\Program Files\Microsoft Office\Root\Office16\`) and from `Application.Version` / `.Build` (16.0 / 20326), not from the File, Account, About Access screen. The "Version 2xxx" label on that screen wasn't recorded.
- Database MD5: matches (8F37C3E11B8C061010DEB6C14E48000B). Checked before and after the run. The original was never opened; all work used copies in `%TEMP%`.

How the checks were run: each statement was saved as a query (`check_1a` … `check_1d`, plus `tmpRecentTouches` and `tmpQualifiedLeads`) in a copy at `%TEMP%\cairn-check.accdb`. Each query was then opened as a datasheet through Access automation, not pasted into SQL View by hand. The values below were read back from the same queries through DAO, so they're what Access returns. The screenshots are full Access windows, taken with the harness's own window capture.

## 1a Routes — match
| Route | N |
|---|---|
| Manufacturer Repair | 441 |
| Customer Pays Repair | 104 |
| Advance Replacement | 103 |
| Carrier Claim | 64 |
| Scrap - Out of Warranty | 44 |
| Account Manager | 16 |

Total 772. No `Bench Test` row. (check-1a.png)

## 1b Orphan routes — match
| Route | N |
|---|---|
| Customer Pays Repair | 18 |
| Carrier Claim | 6 |
| Account Manager | 3 |
| Advance Replacement | 2 |

Total 29. (check-1b.png)

## 1c Leads as of 2026-10-09 — match
| Leads | ByQuote |
|---|---|
| 39 | 13 |

(check-1c.png)

qryQualifiedLeads today (2026-10-28, which is also `Date()` inside Access): 36 rows. (check-1c-today.png)

The saved `qryQualifiedLeads` SQL is the same as the handoff's `tmpQualifiedLeads` apart from Access's own line breaks and spacing, except that it joins `qryRecentTouches` (`WHERE ActivityDate >= Date() - 90`).

## 1d Round — SQL: 2, 4, 0.12, 0.38, -2, 1   VBA: 2, 4, 0.12, 0.38, -2, 1   same rule: yes
| a | b | c | d | e | f |
|---|---|---|---|---|---|
| 2 | 4 | 0.12 | 0.38 | -2 | 1 |

Both match the banker's-rounding prediction. (check-1d-sql.png, check-1d-vba.png)

VBA note: no keystrokes are sent, so the `?` line wasn't typed into the Immediate window. Instead, a scratch module in the copy (`modRoundCheck`) runs `Debug.Print` with the same six `Round` calls, and the Immediate window shows its output. The screenshot shows that code and the output together. A second function in the module joined the same six results into a string, and it returned `2 | 4 | 0.12 | 0.38 | -2 | 1`.

## Screens
All five are in `screens/`, uncropped, with the run's `manifest.json` and `index.html`. They come from harness run `2026-10-28_174200`, using `-HideNavigationPane`.

| File | Scenario | Ok | Note |
|---|---|---|---|
| 01-commission-sql.png | commission-sql | yes | Full SQL of `qryCommissionDetail` (23 lines) in the SQL View editor |
| 02-row-source.png | row-source | yes | Property Sheet on the **Data** tab, with Row Source showing in full: `SELECT CustomerID, CompanyName FROM Customers ORDER BY CompanyName;`. Two changes from the handoff's method, both without keystrokes: the tab was switched by messaging the sheet's tab control (the sheet opened on All, where Row Source is off-screen), and the sheet was widened for the shot, then put back to its old width. |
| 03-vba-doccustomerchanged.png | vba-doccustomerchanged | yes | CairnForms at `DocCustomerChanged`. The empty Immediate window is docked at the bottom, because the 1d check turned it on and the VBA editor remembers that. |
| 04-vba-quotetoorder.png | vba-quotetoorder | yes | CairnForms at `QuoteToOrder`. Same Immediate window note. |
| 05-payment-terms.png | payment-terms | yes | Customer 194 is Oakridge Logistics. Its saved terms are **Net 60**, and the list is Due on Receipt; Net 15; Net 30; Net 45; Net 60. |

## Harness changes
File: `part 2/ScreenshotHarness/Capture-CairnScreens.ps1`. The existing scenarios are unchanged and keep their order. The new ones are a new group, **Where the rules live**, placed after the invoice scenario and before the failure group:
- `commission-sql`: opens `qryCommissionDetail` in Design View, switches to SQL View (184), and waits until the editor has drawn text (`Wait-SqlText`) before the snap. The first run with a fixed 3.5 s wait came out blank.
- `row-source`: opens `frmQuotes` in Design View, selects `CustomerID`, opens the Property Sheet (287), and switches it to the Data tab (`Select-PropertyTab`, which sends TCM_SETCURFOCUS to the sheet's tab control). It widens the sheet to 720 through `CommandBars("Property Sheet").Width` and restores the previous width afterwards.
- `vba-doccustomerchanged` and `vba-quotetoorder`: `DoCmd.OpenModule('CairnForms', …)`, then capture the maximized VBA editor. Both go through a new `Snap-Module` helper; the existing `behind-the-scenes` step is untouched.
- `payment-terms`: `Open-Form 'frmCustomers' 'CustomerID=194'`, then drops down `PaymentTerms`. The caption records the company name, saved terms and the list.

Also changed: `part 2/ScreenshotHarness/README.md` gained a row for the new group and notes on the three helpers.

Side effect on this machine's Access settings: during testing, the Property Sheet's remembered width was briefly 720. It's back to 270, its original value. The VBA editor's Immediate window is now left open.
