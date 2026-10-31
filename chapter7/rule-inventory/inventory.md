| Rule | Kind | Source | Evidence |
|------|------|--------|----------|
| qryCommissionDetail | Saved query | CairnSystems_History.accdb (CairnBuild.bas:661) | SELECT b.RepID,r.FirstName & ' ' & r.LastName,r.Email,r.CommissionRate,b.OrderID,b.OrderNumber,b.OrderDate,b.OrderStatus,c.CompanyName,b.Subtotal,b.Subtotal - b.ProductCost,CCur(Round(b.Subtotal * r.CommissionRate, 2)),b.AmountPaid,b.BalanceDue FROM [qryOrderBalances],[SalesReps],[Customers] WHERE b.OrderStatus <> 'Cancelled' |
| qryCustomerActivityLog | Saved query | CairnSystems_History.accdb (CairnBuild.bas:689) | SELECT c.CompanyName,a.ActivityDate,a.ActivityType,a.Subject,a.Details,r.FirstName & ' ' & r.LastName,ct.FirstName & ' ' & ct.LastName,a.FollowUpDate,a.Completed FROM [Activities],[Customers],[SalesReps],[Contacts] ORDER BY c.CompanyName |
| qryFollowUpsDue | Saved query | CairnSystems_History.accdb (CairnBuild.bas:681) | SELECT a.ActivityID,a.FollowUpDate,r.FirstName & ' ' & r.LastName,c.CompanyName,ct.FirstName & ' ' & ct.LastName,ct.Phone,a.ActivityType,a.Subject,a.Details FROM [Activities],[Customers],[SalesReps],[Contacts] WHERE a.Completed = False AND a.FollowUpDate Is Not Null AND a.FollowUpDate <= Date() + 7 ORDER BY a.FollowUpDate |
| qryInvoiceBase | Saved query | CairnSystems_History.accdb (CairnBuild.bas:586) | SELECT b.OrderID,b.OrderNumber,b.OrderDate,b.OrderStatus,b.Subtotal,b.TaxRate,b.TaxAmount,b.ShippingCost,b.OrderTotal,b.AmountPaid,b.BalanceDue,o.CustomerPO,o.ShipDate,o.ShipVia,o.TrackingNumber,c.CompanyName,c.BillAddress,c.BillCity,c.BillState,c.BillZip,c.PaymentTerms,r.FirstName & ' ' & r.LastName FROM [qryOrderBalances],[Orders],[Customers],[SalesReps] |
| qryInvoicePrint | Saved query | CairnSystems_History.accdb (CairnBuild.bas:591) | SELECT ib.*,ci.CompanyName,ci.Address1,ci.City & ', ' & ci.State & ' ' & ci.Zip,ci.Phone,ci.Website FROM [qryInvoiceBase],[CompanyInfo] |
| qryManufacturerLog | Saved query | CairnSystems_History.accdb (CairnBuild.bas:172) | SELECT m.MfrRequestID,o.OrderNumber,m.Attempt,m.SentAt,u.UserName,m.HttpStatus,m.ManufacturerRef,m.ErrorText,m.RequestBody,m.ResponseBody,m.OrderID FROM [ManufacturerOrders],[Orders],[Users] ORDER BY m.SentAt |
| qryMonthEnd_CashReceived | Saved query | CairnSystems_History.accdb (CairnBuild.bas:636) | SELECT p.PaymentDate,o.OrderNumber,c.CompanyName,p.PaymentMethod,p.PaymentRef,p.Amount FROM [Payments],[Orders],[Customers] WHERE p.PaymentDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) AND p.PaymentDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) ORDER BY p.PaymentDate |
| qryMonthEnd_Orders | Saved query | CairnSystems_History.accdb (CairnBuild.bas:604) | SELECT b.OrderNumber,b.OrderDate,b.OrderStatus,c.CompanyName,r.FirstName & ' ' & r.LastName,b.Subtotal,b.TaxAmount,b.ShippingCost,b.OrderTotal,b.ProductCost,b.Subtotal - b.ProductCost,b.AmountPaid,b.BalanceDue FROM [qryOrderBalances],[Customers],[SalesReps] WHERE b.OrderStatus <> 'Cancelled' AND b.OrderDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) AND b.OrderDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) ORDER BY b.OrderDate |
| qryMonthEnd_QuotePipeline | Saved query | CairnSystems_History.accdb (CairnBuild.bas:644) | SELECT t.QuoteStatus,Count(t.QuoteID),Sum(t.QuoteTotal) FROM [qryQuoteTotals] WHERE t.QuoteDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) AND t.QuoteDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) |
| qryMonthEnd_SalesByProduct | Saved query | CairnSystems_History.accdb (CairnBuild.bas:627) | SELECT l.Category,l.SKU,l.ItemDescription,Sum(l.Quantity),Sum(l.LineTotal),Sum(l.LineCost),Sum(l.LineTotal) - Sum(l.LineCost) FROM [qryOrderLines],[Orders] WHERE o.OrderStatus <> 'Cancelled' AND o.OrderDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) AND o.OrderDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) ORDER BY l.Category |
| qryMonthEnd_SalesByRep | Saved query | CairnSystems_History.accdb (CairnBuild.bas:616) | SELECT r.RepID,r.FirstName & ' ' & r.LastName,Count(b.OrderID),Sum(b.Subtotal),Sum(b.OrderTotal),Sum(b.Subtotal - b.ProductCost),r.CommissionRate,Round(Sum(b.Subtotal)*r.CommissionRate,2) FROM [qryOrderTotals],[SalesReps] WHERE b.OrderStatus <> 'Cancelled' AND b.OrderDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) AND b.OrderDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) ORDER BY Sum(b.Subtotal) |
| qryOpenPhoneMessages | Saved query | CairnSystems_History.accdb (CairnBuild.bas:672) | SELECT m.MessageID,m.ReceivedAt,fr.FirstName & ' ' & fr.LastName,tb.FirstName & ' ' & tb.LastName,m.CallerName,Nz(c.CompanyName, m.CallerCompany),m.CallerPhone,m.Message,m.Urgent,m.PleaseCallBack,m.WillCallAgain,m.MessageStatus FROM [PhoneMessages],[SalesReps],[SalesReps],[Customers] WHERE m.MessageStatus NOT IN ('Returned', 'Closed') ORDER BY m.Urgent |
| qryOpenQuoteValue | Saved query | CairnSystems_History.accdb (CairnBuild.bas:152) | SELECT CustomerID,Max(QuoteTotal) FROM [qryQuoteTotals] WHERE QuoteStatus = 'Sent' |
| qryOpenReceivables | Saved query | CairnSystems_History.accdb (CairnBuild.bas:651) | SELECT b.OrderNumber,c.CompanyName,b.OrderDate,b.OrderStatus,b.OrderTotal,b.AmountPaid,b.BalanceDue,Date() - b.OrderDate,c.PaymentTerms FROM [qryOrderBalances],[Customers] WHERE b.BalanceDue > 0.005 AND b.OrderStatus <> 'Cancelled' ORDER BY b.OrderDate |
| qryOrderBalances | Saved query | CairnSystems_History.accdb (CairnBuild.bas:555) | SELECT t.*,CCur(Nz(p.AmountPaid,0)),CCur(t.OrderTotal - Nz(p.AmountPaid,0)),p.LastPaymentDate FROM [qryOrderTotals],[qryOrderPayments] |
| qryOrderLines | Saved query | CairnSystems_History.accdb (CairnBuild.bas:540) | SELECT ol.OrderLineID,ol.OrderID,ol.ProductID,p.SKU,p.Category,Nz(ol.LineDescription, p.ProductName),ol.Quantity,ol.UnitPrice,ol.DiscountPct,Round(ol.Quantity*ol.UnitPrice*(1-ol.DiscountPct),2),ol.Quantity*p.UnitCost,ol.SerialNumbers FROM [OrderLines],[Products] |
| qryOrderPayments | Saved query | CairnSystems_History.accdb (CairnBuild.bas:551) | SELECT OrderID,Sum(Amount),Max(PaymentDate) FROM [Payments] |
| qryOrderTotals | Saved query | CairnSystems_History.accdb (CairnBuild.bas:548) | SELECT o.OrderID,o.OrderNumber,o.CustomerID,o.RepID,o.OrderDate,o.OrderStatus,CCur(Nz(Sum(l.LineTotal),0)),o.TaxRate,CCur(Round(Nz(Sum(l.LineTotal),0)*o.TaxRate,2)),o.ShippingCost,CCur(Nz(Sum(l.LineTotal),0) + Round(Nz(Sum(l.LineTotal),0)*o.TaxRate,2) + o.ShippingCost),CCur(Nz(Sum(l.LineCost),0)) FROM [Orders],[qryOrderLines] |
| qryQualifiedLeads | Saved query | CairnSystems_History.accdb (CairnBuild.bas:165) | SELECT c.CustomerID,c.CompanyName,c.Industry,r.FirstName & ' ' & r.LastName,Nz(t.Touches, 0),q.OpenQuote FROM [Customers],[SalesReps],[qryRecentTouches],[qryOpenQuoteValue] WHERE c.CustomerStatus = 'Prospect' AND (q.OpenQuote >= 5000 OR (Nz(t.Touches, 0) >= 2 AND c.Industry IN ('Utilities', 'Food & Beverage', 'Healthcare'))) AND EXISTS (SELECT 1 FROM Contacts AS k WHERE k.CustomerID = c.CustomerID AND (k.Title Like '*Director*' OR k.Title Like '*Manager*')) ORDER BY q.OpenQuote |
| qryQuoteLines | Saved query | CairnSystems_History.accdb (CairnBuild.bas:504) | SELECT ql.QuoteLineID,ql.QuoteID,ql.ProductID,p.SKU,Nz(ql.LineDescription, p.ProductName),ql.Quantity,ql.UnitPrice,ql.DiscountPct,Round(ql.Quantity*ql.UnitPrice*(1-ql.DiscountPct),2) FROM [QuoteLines],[Products] |
| qryQuotePrint | Saved query | CairnSystems_History.accdb (CairnBuild.bas:533) | SELECT b.*,Nz(b.Terms, ci.QuoteTerms),ci.CompanyName,ci.Address1,ci.City & ', ' & ci.State & ' ' & ci.Zip,ci.Phone,ci.Email,ci.Website FROM [qryQuotePrintBase],[CompanyInfo] |
| qryQuotePrintBase | Saved query | CairnSystems_History.accdb (CairnBuild.bas:526) | SELECT q.QuoteID,q.QuoteNumber,q.QuoteDate,q.ValidUntil,q.QuoteStatus,q.Terms,q.Notes,c.CompanyName,c.BillAddress,c.BillCity,c.BillState,c.BillZip,c.BillCountry,ct.FirstName & ' ' & ct.LastName,ct.Email,ct.Phone,r.FirstName & ' ' & r.LastName,r.Email,r.Phone,l.QuoteLineID,l.SKU,l.ItemDescription,l.Quantity,l.UnitPrice,l.DiscountPct,l.LineTotal,t.Subtotal,t.TaxRate,t.TaxAmount,t.ShippingCost,t.QuoteTotal FROM [Quotes],[Customers],[SalesReps],[qryQuoteTotals],[Contacts],[qryQuoteLines] |
| qryQuoteTotals | Saved query | CairnSystems_History.accdb (CairnBuild.bas:511) | SELECT q.QuoteID,q.QuoteNumber,q.CustomerID,q.RepID,q.QuoteDate,q.QuoteStatus,CCur(Nz(Sum(l.LineTotal),0)),q.TaxRate,CCur(Round(Nz(Sum(l.LineTotal),0)*q.TaxRate,2)),q.ShippingCost,CCur(Nz(Sum(l.LineTotal),0) + Round(Nz(Sum(l.LineTotal),0)*q.TaxRate,2) + q.ShippingCost) FROM [Quotes],[qryQuoteLines] |
| qryRMARouting | Saved query | CairnSystems_History.accdb (CairnBuild.bas:148) | SELECT r.RMAID,r.RMANumber,r.OpenedDate,r.RMAStatus,r.Reason,r.SerialNumber,r.ExtendedWarranty,c.CompanyName,c.TaxExempt,p.SKU,p.Category,o.OrderNumber,Nz(o.ShipDate, o.OrderDate),r.OpenedDate - Nz(o.ShipDate, o.OrderDate),IIf(r.Reason='Damaged in Shipping','Carrier Claim',IIf(r.Reason='Dead on Arrival' And [DaysInService]<=30,'Advance Replacement',IIf(p.Category='Service','Account Manager',IIf([DaysInService]<=365 Or r.ExtendedWarranty,'Manufacturer Repair',IIf(r.Reason='Dead on Arrival','Advance Replacement',IIf([DaysInService]<=90,'Bench Test',IIf(c.TaxExempt,'Account Manager',IIf([DaysInService]>730,'Scrap - Out of Warranty','Customer Pays Repair')))))))) FROM [RMAs],[Customers],[Products],[Orders] |
| qryReceiptPrint | Saved query | CairnSystems_History.accdb (CairnBuild.bas:576) | SELECT rb.*,ci.CompanyName,ci.Address1,ci.City & ', ' & ci.State & ' ' & ci.Zip,ci.Phone,ci.Website,ci.ReceiptFooter FROM [qryReceiptPrintBase],[CompanyInfo] |
| qryReceiptPrintBase | Saved query | CairnSystems_History.accdb (CairnBuild.bas:569) | SELECT pay.PaymentID,pay.OrderID,pay.PaymentDate,pay.Amount,pay.PaymentMethod,pay.PaymentRef,o.OrderNumber,o.OrderDate,o.CustomerPO,c.CompanyName,c.BillAddress,c.BillCity,c.BillState,c.BillZip,c.BillCountry,r.FirstName & ' ' & r.LastName,l.OrderLineID,l.SKU,l.ItemDescription,l.Quantity,l.UnitPrice,l.DiscountPct,l.LineTotal,b.Subtotal,b.TaxAmount,b.ShippingCost,b.OrderTotal,b.AmountPaid,b.BalanceDue FROM [Payments],[Orders],[Customers],[SalesReps],[qryOrderBalances],[qryOrderLines] |
| qryRecentTouches | Saved query | CairnSystems_History.accdb (CairnBuild.bas:154) | SELECT CustomerID,Count(*) FROM [Activities] WHERE ActivityDate >= Date() - 90 |
| BuildCairnDatabase | VBA routine | CairnBuild.bas:31 |  |
| AddOperationsTables | VBA routine | CairnBuild.bas:63 | ------------------------------------------------------------------------------ |
| CreateOperationsQueries | VBA routine | CairnBuild.bas:130 |  |
| SetColWidth | VBA routine | CairnBuild.bas:180 | Datasheet column width saved with a table or query field |
| AddUser | VBA routine | CairnBuild.bas:192 |  |
| AddRMA | VBA routine | CairnBuild.bas:197 |  |
| FieldExists | VBA routine | CairnBuild.bas:204 |  |
| CreateTables | VBA routine | CairnBuild.bas:215 | ------------------------------------------------------------------------------ |
| CreateIndexes | VBA routine | CairnBuild.bas:333 |  |
| RepairTextDefaults | VBA routine | CairnBuild.bas:346 | Re-set text defaults in Access's own "x" form and clean any rows already saved with quotes. Safe to run again. |
| SetFieldFormats | VBA routine | CairnBuild.bas:361 |  |
| LoadSampleData | VBA routine | CairnBuild.bas:376 | ------------------------------------------------------------------------------ |
| RebuildQueries | VBA routine | CairnBuild.bas:490 | Re-create just the saved queries (safe to run again; existing ones are replaced) |
| CreateQueries | VBA routine | CairnBuild.bas:497 |  |
| ClearSampleData | VBA routine | CairnBuild.bas:695 | ------------------------------------------------------------------------------ |
| X | VBA routine | CairnBuild.bas:714 | ------------------------------------------------------------------------------ |
| Q | VBA routine | CairnBuild.bas:723 |  |
| TableExists | VBA routine | CairnBuild.bas:735 |  |
| SetFmt | VBA routine | CairnBuild.bas:742 |  |
| Sq | VBA routine | CairnBuild.bas:753 |  |
| Dt | VBA routine | CairnBuild.bas:761 |  |
| Nm | VBA routine | CairnBuild.bas:765 |  |
| AddProduct | VBA routine | CairnBuild.bas:769 |  |
| AddQuote | VBA routine | CairnBuild.bas:775 |  |
| AddLine | VBA routine | CairnBuild.bas:782 |  |
| AddActivity | VBA routine | CairnBuild.bas:788 |  |
| AddMsg | VBA routine | CairnBuild.bas:795 |  |
| BuildCairnFormsQuiet | VBA routine | CairnForms.bas:61 | No message boxes; errors go to CairnForms_error.txt (for automated builds) |
| BuildCairnForms | VBA routine | CairnForms.bas:65 |  |
| BuildSfrmContacts | VBA routine | CairnForms.bas:124 | ------------------------------------------------------------------------------ |
| BuildSfrmCustomerActivities | VBA routine | CairnForms.bas:138 |  |
| BuildSfrmCustomerQuotes | VBA routine | CairnForms.bas:155 |  |
| BuildSfrmCustomerOrders | VBA routine | CairnForms.bas:168 |  |
| BuildSfrmLines | VBA routine | CairnForms.bas:183 |  |
| BuildSfrmPayments | VBA routine | CairnForms.bas:200 |  |
| BuildSfrmOpenMessages | VBA routine | CairnForms.bas:214 |  |
| BuildSfrmFollowUps | VBA routine | CairnForms.bas:231 |  |
| BuildFrmCompanyInfo | VBA routine | CairnForms.bas:250 | ------------------------------------------------------------------------------ |
| BuildFrmSalesReps | VBA routine | CairnForms.bas:272 |  |
| BuildFrmProducts | VBA routine | CairnForms.bas:290 |  |
| BuildFrmActivities | VBA routine | CairnForms.bas:308 |  |
| BuildFrmPhoneMessages | VBA routine | CairnForms.bas:330 |  |
| BuildFrmCustomers | VBA routine | CairnForms.bas:358 | ------------------------------------------------------------------------------ |
| BuildFrmQuotes | VBA routine | CairnForms.bas:405 |  |
| BuildFrmOrders | VBA routine | CairnForms.bas:444 |  |
| BuildFrmLogin | VBA routine | CairnForms.bas:499 | ------------------------------------------------------------------------------ |
| BuildFrmUsers | VBA routine | CairnForms.bas:512 |  |
| BuildFrmRMAs | VBA routine | CairnForms.bas:525 |  |
| BuildFrmMainMenu | VBA routine | CairnForms.bas:566 | ------------------------------------------------------------------------------ |
| OpenFrm | VBA routine | CairnForms.bas:618 | ============================================================================== |
| OpenDatasheet | VBA routine | CairnForms.bas:622 |  |
| OpenNew | VBA routine | CairnForms.bas:626 |  |
| OpenQry | VBA routine | CairnForms.bas:630 |  |
| CloseMe | VBA routine | CairnForms.bas:635 |  |
| OpenRec | VBA routine | CairnForms.bas:641 | Double-click in a list: open the record in its own form |
| FindRec | VBA routine | CairnForms.bas:647 | "Go to" combo at the top of a form |
| RequeryControls | VBA routine | CairnForms.bas:658 | Requery one or more controls (names separated by ;) |
| RecalcParent | VBA routine | CairnForms.bas:665 |  |
| NextNumber | VBA routine | CairnForms.bas:671 | Next number like Q-2026-0105, SO-2026-0205 or RMA-2026-0012 |
| NewDocForCustomer | VBA routine | CairnForms.bas:684 | From frmCustomers: open a new quote / order / activity for this customer |
| DocLoad | VBA routine | CairnForms.bas:694 | Quote / order opened from a customer: preset the customer |
| DocBeforeInsert | VBA routine | CairnForms.bas:700 |  |
| DocCustomerChanged | VBA routine | CairnForms.bas:706 | Customer picked on a quote/order: tax rate, rep and primary contact |
| ActivityCustomerChanged | VBA routine | CairnForms.bas:722 | frmActivities: customer picked (or preset from frmCustomers) |
| ActivityDefaults | VBA routine | CairnForms.bas:731 | Activities tab on frmCustomers: default the rep to the customer's rep |
| LineProductChanged | VBA routine | CairnForms.bas:736 |  |
| MessageCustomerChanged | VBA routine | CairnForms.bas:741 |  |
| MessageReturned | VBA routine | CairnForms.bas:748 |  |
| QuoteToOrder | VBA routine | CairnForms.bas:755 | Copy a quote and its lines into a new sales order |
| LoginCheck | VBA routine | CairnForms.bas:793 | ------------------------------------------------------------------------------ |
| SignOut | VBA routine | CairnForms.bas:814 |  |
| QuitApp | VBA routine | CairnForms.bas:820 |  |
| MenuLoad | VBA routine | CairnForms.bas:826 | because a form can't close itself while it is still loading) |
| MenuTimer | VBA routine | CairnForms.bas:834 |  |
| RMADefaults | VBA routine | CairnForms.bas:843 | ------------------------------------------------------------------------------ |
| SendManufacturerOrder | VBA routine | CairnForms.bas:854 | ------------------------------------------------------------------------------ |
| MfrRefs | VBA routine | CairnForms.bas:927 | Manufacturer order numbers recorded for an order, e.g. "MFR-100231, MFR-100232" |
| JsonText | VBA routine | CairnForms.bas:944 | Value of "key":"value" in a JSON reply (enough for the manufacturer's answer) |
| I2T | VBA routine | CairnForms.bas:957 | ============================================================================== |
| R | VBA routine | CairnForms.bas:962 | Top of layout row n (inches) |
| Fn | VBA routine | CairnForms.bas:967 | Event property that calls one of the runtime functions: =Fn([Form],"a","b") |
| NewForm | VBA routine | CairnForms.bas:976 |  |
| SaveForm | VBA routine | CairnForms.bas:993 |  |
| DiscardTemp | VBA routine | CairnForms.bas:1000 |  |
| DropAllForms | VBA routine | CairnForms.bas:1009 |  |
| TableExists | VBA routine | CairnForms.bas:1019 |  |
| FormExists | VBA routine | CairnForms.bas:1026 |  |
| SetDbProp | VBA routine | CairnForms.bas:1033 |  |
| Title | VBA routine | CairnForms.bas:1043 |  |
| Lbl | VBA routine | CairnForms.bas:1050 |  |
| AttachLabel | VBA routine | CairnForms.bas:1060 | Label to the left of a control (attached, so it moves/hides with it) |
| Txt | VBA routine | CairnForms.bas:1069 | Bound text box (src = field) or calculated one (src starts with "=") |
| Cbo | VBA routine | CairnForms.bas:1081 |  |
| SetupList | VBA routine | CairnForms.bas:1092 |  |
| Chk | VBA routine | CairnForms.bas:1112 |  |
| Btn | VBA routine | CairnForms.bas:1121 |  |
| TotalBox | VBA routine | CairnForms.bas:1131 | Read-only total pulled from a totals query for the current record |
| SubF | VBA routine | CairnForms.bas:1141 |  |
| FindBox | VBA routine | CairnForms.bas:1154 |  |
| DsTxt | VBA routine | CairnForms.bas:1165 | ---- datasheet helpers (position only sets column order) ---- |
| DsCbo | VBA routine | CairnForms.bas:1175 |  |
| DsChk | VBA routine | CairnForms.bas:1185 |  |
| ReadOnly | VBA routine | CairnForms.bas:1190 |  |
| DblClickOpens | VBA routine | CairnForms.bas:1194 |  |
| GenerateHistoryQuiet | VBA routine | CairnHistory.bas:78 |  |
| GenerateHistory | VBA routine | CairnHistory.bas:82 |  |
| AddOperationsHistoryQuiet | VBA routine | CairnHistory.bas:154 | ============================================================================== |
| AddOperationsHistory | VBA routine | CairnHistory.bas:158 |  |
| AddOperationsData | VBA routine | CairnHistory.bas:177 |  |
| OpsUsers | VBA routine | CairnHistory.bas:193 |  |
| OpsRMAs | VBA routine | CairnHistory.bas:238 | Returns against shipped hardware, plus some that point at orders that don't exist |
| AddRMARow | VBA routine | CairnHistory.bas:272 |  |
| RMAText | VBA routine | CairnHistory.bas:328 |  |
| OpsMfrOrders | VBA routine | CairnHistory.bas:342 | order, so the retry ordered the hardware twice; Access only recorded one. |
| MfrReply | VBA routine | CairnHistory.bas:400 |  |
| MfrRow | VBA routine | CairnHistory.bas:404 |  |
| OpsDuplicateCustomers | VBA routine | CairnHistory.bas:421 | recent order or two booked against the new record |
| NameVariant | VBA routine | CairnHistory.bas:467 |  |
| OpsRetiredStatuses | VBA routine | CairnHistory.bas:483 | Status values nobody uses any more, and rows saved by the old text-default bug |
| OpsOrphanQuoteLines | VBA routine | CairnHistory.bas:497 | their lines stayed behind |
| OpsPricingCodes | VBA routine | CairnHistory.bas:512 | Pricing arrangements typed into Notes as codes, each rep their own way |
| OpsSummary | VBA routine | CairnHistory.bas:533 |  |
| Count1 | VBA routine | CairnHistory.bas:546 |  |
| SqlText | VBA routine | CairnHistory.bas:553 |  |
| HasTable | VBA routine | CairnHistory.bas:557 |  |
| IndNames | VBA routine | CairnHistory.bas:568 | ------------------------------------------------------------------------------ |
| IndWeights | VBA routine | CairnHistory.bas:573 |  |
| IndSuffixes | VBA routine | CairnHistory.bas:577 |  |
| IndSensors | VBA routine | CairnHistory.bas:591 | Sensors each industry buys ("TH" = whichever temperature sensor was current) |
| MonthWeights | VBA routine | CairnHistory.bas:604 |  |
| NamePrefixes | VBA routine | CairnHistory.bas:608 |  |
| FirstNames | VBA routine | CairnHistory.bas:619 |  |
| LastNames | VBA routine | CairnHistory.bas:627 |  |
| SetupProducts | VBA routine | CairnHistory.bas:638 | ------------------------------------------------------------------------------ |
| SetupReps | VBA routine | CairnHistory.bas:683 |  |
| RepActive | VBA routine | CairnHistory.bas:731 |  |
| PickRepAt | VBA routine | CairnHistory.bas:736 | A rep (ID) working on date d, weighted by their share of accounts |
| RepAt | VBA routine | CairnHistory.bas:755 | The customer's rep on date d (a stand-in covers if that rep wasn't there yet / had left) |
| ReassignReps | VBA routine | CairnHistory.bas:760 | Accounts whose rep has left move to a current rep; a share of the rest are rebalanced each year |
| SetupCustomers | VBA routine | CairnHistory.bas:770 | ------------------------------------------------------------------------------ |
| NewCustomer | VBA routine | CairnHistory.bas:827 |  |
| PickPlace | VBA routine | CairnHistory.bas:901 |  |
| TermsToDays | VBA routine | CairnHistory.bas:916 |  |
| ContactFor | VBA routine | CairnHistory.bas:920 |  |
| PickCustomer | VBA routine | CairnHistory.bas:927 | Random customer who was buying on date d (key accounts more often) |
| PickProspect | VBA routine | CairnHistory.bas:940 |  |
| YearTarget | VBA routine | CairnHistory.bas:952 | ------------------------------------------------------------------------------ |
| SeasonFraction | VBA routine | CairnHistory.bas:962 | Share of a year's (seasonal) sales that falls on or before d |
| OrderSize | VBA routine | CairnHistory.bas:973 |  |
| GenerateYear | VBA routine | CairnHistory.bas:985 |  |
| AgeInstalledBase | VBA routine | CairnHistory.bas:1032 | Older devices drop out of subscription renewals (replaced, retired or renewed on multi-year terms) |
| GrowStage | VBA routine | CairnHistory.bas:1039 |  |
| BuildLines | VBA routine | CairnHistory.bas:1046 | Line items for an order of roughly S dollars. Returns "productID\|qty\|price\|disc;..." |
| AddLine | VBA routine | CairnHistory.bas:1122 |  |
| YearPrice | VBA routine | CairnHistory.bas:1129 | List price in a given year (about 3% a year lower going back) |
| Avail | VBA routine | CairnHistory.bas:1138 |  |
| SensorFor | VBA routine | CairnHistory.bas:1142 |  |
| ShipCost | VBA routine | CairnHistory.bas:1155 |  |
| ProcessOrder | VBA routine | CairnHistory.bas:1161 |  |
| PayDue | VBA routine | CairnHistory.bas:1255 |  |
| AddQuote | VBA routine | CairnHistory.bas:1263 |  |
| AddLines | VBA routine | CairnHistory.bas:1281 |  |
| AddPayment | VBA routine | CairnHistory.bas:1297 |  |
| LostQuotes | VBA routine | CairnHistory.bas:1325 | ------------------------------------------------------------------------------ |
| CheckIns | VBA routine | CairnHistory.bas:1361 |  |
| AddAct | VBA routine | CairnHistory.bas:1388 |  |
| GenerateMessages | VBA routine | CairnHistory.bas:1411 |  |
| Renumber | VBA routine | CairnHistory.bas:1484 | Number quotes / orders in date order within each year: Q-2016-0001, SO-2016-0001 ... |
| FinishCustomers | VBA routine | CairnHistory.bas:1499 |  |
| Summarize | VBA routine | CairnHistory.bas:1515 |  |
| OpenRecordsets | VBA routine | CairnHistory.bas:1533 |  |
| CloseRecordsets | VBA routine | CairnHistory.bas:1545 |  |
| WriteLog | VBA routine | CairnHistory.bas:1550 |  |
| RB | VBA routine | CairnHistory.bas:1560 | ------------------------------------------------------------------------------ |
| Pick | VBA routine | CairnHistory.bas:1564 |  |
| WPick | VBA routine | CairnHistory.bas:1568 |  |
| LogNorm | VBA routine | CairnHistory.bas:1579 |  |
| Phone555 | VBA routine | CairnHistory.bas:1585 |  |
| RandChars | VBA routine | CairnHistory.bas:1589 |  |
| BizDay | VBA routine | CairnHistory.bas:1597 |  |
| PrevBizDay | VBA routine | CairnHistory.bas:1605 |  |
| RandomBizDate | VBA routine | CairnHistory.bas:1614 | Weekday in year y (no later than lastDay), weighted by season |
| SortStage | VBA routine | CairnHistory.bas:1628 |  |
| QSort | VBA routine | CairnHistory.bas:1635 |  |
| BuildCairnReportsQuiet | VBA routine | CairnReports.bas:50 | No message boxes; errors go to CairnReports_error.txt (for automated builds) |
| BuildCairnReports | VBA routine | CairnReports.bas:54 |  |
| BuildRptQuote | VBA routine | CairnReports.bas:83 | ------------------------------------------------------------------------------ |
| BuildRptReceipt | VBA routine | CairnReports.bas:140 | ------------------------------------------------------------------------------ |
| BuildRptCommission | VBA routine | CairnReports.bas:194 | ------------------------------------------------------------------------------ |
| BuildRptInvoiceLines | VBA routine | CairnReports.bas:286 | ------------------------------------------------------------------------------ |
| BuildRptInvoice | VBA routine | CairnReports.bas:302 |  |
| BuildRptRMASummary | VBA routine | CairnReports.bas:366 | ------------------------------------------------------------------------------ |
| PrintInvoice | VBA routine | CairnReports.bas:418 | Sales Orders form: preview the invoice for this order |
| PrintRMASummary | VBA routine | CairnReports.bas:428 | Main menu: ask for the month, then preview the RMA summary |
| PrintQuote | VBA routine | CairnReports.bas:449 | Quotes form: preview the current quote |
| PrintReceipt | VBA routine | CairnReports.bas:459 | Sales Orders form: preview a receipt for the payment selected in the Payments list |
| PrintCommission | VBA routine | CairnReports.bas:470 | Main menu: ask for the month, then preview the commission statements |
| CommissionCriteria | VBA routine | CairnReports.bas:490 |  |
| ClearCommissionPeriod | VBA routine | CairnReports.bas:494 |  |
| CommissionPeriod | VBA routine | CairnReports.bas:498 |  |
| ReceiptPaidToDate | VBA routine | CairnReports.bas:503 | Total paid on the order up to and including this payment |
| SqlDate | VBA routine | CairnReports.bas:512 |  |
| I2T | VBA routine | CairnReports.bas:519 | ============================================================================== |
| NewReport | VBA routine | CairnReports.bas:523 |  |
| SaveReport | VBA routine | CairnReports.bas:539 |  |
| DiscardTemp | VBA routine | CairnReports.bas:552 |  |
| DropAllReports | VBA routine | CairnReports.bas:561 |  |
| QueryExists | VBA routine | CairnReports.bas:574 |  |
| AddReportFooter | VBA routine | CairnReports.bas:581 |  |
| Tb | VBA routine | CairnReports.bas:595 | Text box: field name or "=expression" |
| Lb | VBA routine | CairnReports.bas:609 |  |
| Caps | VBA routine | CairnReports.bas:622 | Small grey heading such as PREPARED FOR |
| HLine | VBA routine | CairnReports.bas:628 |  |
| Letterhead | VBA routine | CairnReports.bas:635 | Seller name/address on the left, big document title on the right |
| InfoRow | VBA routine | CairnReports.bas:648 | "Label: value" pair in the top-right block |
| AddressBlock | VBA routine | CairnReports.bas:657 |  |
| HeadCell | VBA routine | CairnReports.bas:663 |  |
| LineHeadings | VBA routine | CairnReports.bas:673 | Column headings and detail row shared by quote and receipt |
| LineDetail | VBA routine | CairnReports.bas:682 |  |
| TotalRow | VBA routine | CairnReports.bas:694 |  |
| SumCell | VBA routine | CairnReports.bas:706 |  |
| PageFooter | VBA routine | CairnReports.bas:713 |  |
