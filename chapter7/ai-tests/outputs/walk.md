# Breadth-first walk

| # | Level | Unit | Reached from | Rules | Tests | Note |
|---|---|---|---|---|---|---|
| 0 | 0 | overview |  |  |  |  |
| 1 | 1 | vba:CairnForms.NextNumber | overview | yes | 10 |  |
| 2 | 1 | vba:CairnForms.QuoteToOrder | overview | yes |  | 4 replies without a tool call |
| 3 | 1 | vba:CairnForms.LoginCheck | overview | yes |  | 4 replies without a tool call |
| 4 | 1 | vba:CairnForms.ActivityDefaults | overview | yes | 5 |  |
| 5 | 1 | vba:CairnForms.RMADefaults | overview | yes | 7 |  |
| 6 | 1 | vba:CairnForms.LineProductChanged | overview | yes | 7 |  |
| 7 | 1 | vba:CairnForms.DocLoad | overview | yes | 5 |  |
| 8 | 1 | vba:CairnForms.DocBeforeInsert | overview | yes |  | 4 replies without a tool call |
| 9 | 1 | vba:CairnForms.DocCustomerChanged | overview | yes |  | 4 replies without a tool call |
| 10 | 1 | vba:CairnForms.MessageCustomerChanged | overview | yes | 5 |  |
| 11 | 1 | vba:CairnForms.MessageReturned | overview | yes | 4 |  |
| 12 | 1 | vba:CairnForms.MfrRefs | overview | yes | 6 |  |
| 13 | 1 | vba:CairnReports.CommissionCriteria | overview | yes | 5 |  |
| 14 | 1 | vba:CairnReports.CommissionPeriod | overview | yes | 3 |  |
| 15 | 1 | vba:CairnReports.ReceiptPaidToDate | overview | yes | 6 |  |
| 16 | 1 | query:qryQuoteTotals | overview | yes |  | 4 replies without a tool call |
| 17 | 1 | query:qryCommissionDetail | overview | yes |  | ran out of context 2 times |
| 18 | 1 | query:qryOrderBalances | overview | yes |  | 4 replies without a tool call |
| 19 | 1 | query:qryOpenReceivables | overview | yes |  | 4 replies without a tool call |
| 20 | 1 | query:qryOpenQuoteValue | overview | yes | 6 |  |
| 21 | 1 | query:qryRMARouting | overview | yes | 14 |  |
| 22 | 1 | query:qryMonthEnd_Orders | overview | yes |  | ran out of context 2 times |
| 23 | 1 | query:qryMonthEnd_SalesByRep | overview | yes |  | 4 replies without a tool call |
| 24 | 1 | query:qryMonthEnd_SalesByProduct | overview | yes |  | 4 replies without a tool call |
| 25 | 1 | query:qryMonthEnd_CashReceived | overview | yes |  | 4 replies without a tool call |
| 26 | 1 | query:qryMonthEnd_QuotePipeline | overview | yes |  | 4 replies without a tool call |
| 27 | 2 | table:Quotes | vba:CairnForms.NextNumber | yes | 15 |  |
| 28 | 2 | table:Orders | vba:CairnForms.NextNumber | yes | 9 |  |
| 29 | 2 | table:Activities | vba:CairnForms.ActivityDefaults | yes | 15 |  |
| 30 | 2 | table:Products | vba:CairnForms.LineProductChanged | yes |  | 4 replies without a tool call |
| 31 | 2 | table:Customers | vba:CairnForms.DocCustomerChanged | yes | 7 |  |
| 32 | 2 | table:CompanyInfo | vba:CairnForms.DocCustomerChanged | yes | 10 |  |
| 33 | 2 | table:Contacts | vba:CairnForms.DocCustomerChanged | yes | 8 |  |
| 34 | 2 | vba:CairnReports.SqlDate | vba:CairnReports.CommissionCriteria | yes | 5 |  |
| 35 | 2 | query:qryQuoteLines | query:qryQuoteTotals | yes |  | 4 replies without a tool call |
| 36 | 2 | table:SalesReps | query:qryCommissionDetail | yes | 4 |  |
| 37 | 2 | query:qryOrderPayments | query:qryOrderBalances | yes | 8 |  |
| 38 | 2 | query:qryOrderLines | query:qryMonthEnd_SalesByProduct | yes | 6 |  |
| 39 | 2 | table:Payments | query:qryMonthEnd_CashReceived | yes |  | 4 replies without a tool call |
| 40 | 3 | table:QuoteLines | query:qryQuoteLines | yes |  | 4 replies without a tool call |
| 41 | 3 | table:OrderLines | query:qryOrderLines | yes | 11 |  |

## Skipped

- vba:CairnForms.SendManufacturerOrder: ran out of context 2 times
- query:qryOrderTotals: ran out of context 2 times
- query:qryQualifiedLeads: 4 replies without a tool call
- query:qryFollowUpsDue: ran out of context 2 times
- table:RMAs: ran out of context 2 times

## Never reached (141 of 187 units)

vba:CairnBuild, vba:CairnBuild.BuildCairnDatabase, vba:CairnBuild.AddOperationsTables, vba:CairnBuild.CreateOperationsQueries, vba:CairnBuild.SetColWidth, vba:CairnBuild.AddUser, vba:CairnBuild.AddRMA, vba:CairnBuild.FieldExists, vba:CairnBuild.CreateTables, vba:CairnBuild.CreateIndexes, vba:CairnBuild.RepairTextDefaults, vba:CairnBuild.SetFieldFormats, vba:CairnBuild.LoadSampleData, vba:CairnBuild.RebuildQueries, vba:CairnBuild.CreateQueries, vba:CairnBuild.ClearSampleData, vba:CairnBuild.x, vba:CairnBuild.q, vba:CairnBuild.TableExists, vba:CairnBuild.SetFmt, vba:CairnBuild.Sq, vba:CairnBuild.Dt, vba:CairnBuild.nm, vba:CairnBuild.AddProduct, vba:CairnBuild.AddQuote, vba:CairnBuild.AddLine, vba:CairnBuild.AddActivity, vba:CairnBuild.AddMsg, vba:CairnForms, vba:CairnForms.BuildCairnFormsQuiet, vba:CairnForms.BuildCairnForms, vba:CairnForms.BuildSfrmContacts, vba:CairnForms.BuildSfrmCustomerActivities, vba:CairnForms.BuildSfrmCustomerQuotes, vba:CairnForms.BuildSfrmCustomerOrders, vba:CairnForms.BuildSfrmLines, vba:CairnForms.BuildSfrmPayments, vba:CairnForms.BuildSfrmOpenMessages, vba:CairnForms.BuildSfrmFollowUps, vba:CairnForms.BuildFrmCompanyInfo, vba:CairnForms.BuildFrmSalesReps, vba:CairnForms.BuildFrmProducts, vba:CairnForms.BuildFrmActivities, vba:CairnForms.BuildFrmPhoneMessages, vba:CairnForms.BuildFrmCustomers, vba:CairnForms.BuildFrmQuotes, vba:CairnForms.BuildFrmOrders, vba:CairnForms.BuildFrmLogin, vba:CairnForms.BuildFrmUsers, vba:CairnForms.BuildFrmRMAs, vba:CairnForms.BuildFrmMainMenu, vba:CairnForms.OpenFrm, vba:CairnForms.OpenDatasheet, vba:CairnForms.OpenNew, vba:CairnForms.OpenQry, vba:CairnForms.CloseMe, vba:CairnForms.OpenRec, vba:CairnForms.FindRec, vba:CairnForms.RequeryControls, vba:CairnForms.RecalcParent, vba:CairnForms.NewDocForCustomer, vba:CairnForms.ActivityCustomerChanged, vba:CairnForms.SignOut, vba:CairnForms.QuitApp, vba:CairnForms.MenuLoad, vba:CairnForms.MenuTimer, vba:CairnForms.JsonText, vba:CairnForms.I2T, vba:CairnForms.R, vba:CairnForms.Fn, vba:CairnForms.NewForm, vba:CairnForms.SaveForm, vba:CairnForms.DiscardTemp, vba:CairnForms.DropAllForms, vba:CairnForms.TableExists, vba:CairnForms.FormExists, vba:CairnForms.SetDbProp, vba:CairnForms.Title, vba:CairnForms.Lbl, vba:CairnForms.AttachLabel, vba:CairnForms.Txt, vba:CairnForms.Cbo, vba:CairnForms.SetupList, vba:CairnForms.Chk, vba:CairnForms.Btn, vba:CairnForms.TotalBox, vba:CairnForms.SubF, vba:CairnForms.FindBox, vba:CairnForms.DsTxt, vba:CairnForms.DsCbo, vba:CairnForms.DsChk, vba:CairnForms.ReadOnly, vba:CairnForms.DblClickOpens, vba:CairnReports, vba:CairnReports.BuildCairnReportsQuiet, vba:CairnReports.BuildCairnReports, vba:CairnReports.BuildRptQuote, vba:CairnReports.BuildRptReceipt, vba:CairnReports.BuildRptCommission, vba:CairnReports.BuildRptInvoiceLines, vba:CairnReports.BuildRptInvoice, vba:CairnReports.BuildRptRMASummary, vba:CairnReports.PrintInvoice, vba:CairnReports.PrintRMASummary, vba:CairnReports.PrintQuote, vba:CairnReports.PrintReceipt, vba:CairnReports.PrintCommission, vba:CairnReports.ClearCommissionPeriod, vba:CairnReports.I2T, vba:CairnReports.NewReport, vba:CairnReports.SaveReport, vba:CairnReports.DiscardTemp, vba:CairnReports.DropAllReports, vba:CairnReports.QueryExists, vba:CairnReports.AddReportFooter, vba:CairnReports.tb, vba:CairnReports.Lb, vba:CairnReports.Caps, vba:CairnReports.HLine, vba:CairnReports.Letterhead, vba:CairnReports.InfoRow, vba:CairnReports.AddressBlock, vba:CairnReports.HeadCell, vba:CairnReports.LineHeadings, vba:CairnReports.LineDetail, vba:CairnReports.TotalRow, vba:CairnReports.SumCell, vba:CairnReports.PageFooter, query:qryCustomerActivityLog, query:qryInvoiceBase, query:qryInvoicePrint, query:qryManufacturerLog, query:qryOpenPhoneMessages, query:qryQuotePrint, query:qryQuotePrintBase, query:qryReceiptPrint, query:qryReceiptPrintBase, query:qryRecentTouches, table:ManufacturerOrders, table:PhoneMessages, table:Users
