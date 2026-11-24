Attribute VB_Name = "CairnReports"
'==============================================================================
'  CAIRN SYSTEMS - REPORTS BUILDER
'------------------------------------------------------------------------------
'  Run AFTER BuildCairnDatabase (and RebuildQueries if your database was built
'  with an older CairnBuild). Import this file (Alt+F11 > File > Import File),
'  click inside BuildCairnReports and press F5. Safe to run again.
'
'  REPORTS
'    rptQuote       Printable customer quote, one quote per page
'                   (Print Quote button on the Quotes form)
'    rptReceipt     Payment receipt with the order's items and balance
'                   (Print Receipt button on the Sales Orders form)
'    rptCommission  Month-end commission statement, one page per rep plus a
'                   summary page (Commission Statements button on the menu)
'    rptInvoice     Customer invoice laid out for a window envelope with a
'                   tear-off remittance stub (Print Invoice on Sales Orders).
'                   The line-item box is a fixed size: it holds 6 lines and
'                   anything after that is not printed, though the totals
'                   still include it. Kept that way; see README.
'    rptRMASummary  Returns opened in a month, grouped by route
'                   (RMA Summary on the menu; needs AddOperationsTables)
'
'  Commission = order net sales (before tax and shipping) x the rep's
'  commission rate, for non-cancelled orders dated in the month.
'==============================================================================
Option Compare Database
Option Explicit

Private mTmp As String          ' design-view name of the report being built
Private mPeriod As String       ' month shown on the commission statement

Private Const REPORT_LIST As String = "rptQuote;rptReceipt;rptCommission;rptInvoice;rptInvoiceLines;rptRMASummary"
Private Const INVOICE_LINES As Integer = 6      ' lines that fit in the invoice's line-item box
Private Const PAGE_W As Single = 7.5

' Section numbers
Private Const SEC_DETAIL As Integer = 0
Private Const SEC_RPT_FOOTER As Integer = 2
Private Const SEC_PAGE_HEADER As Integer = 3
Private Const SEC_PAGE_FOOTER As Integer = 4
Private Const SEC_GRP_HEADER As Integer = 5
Private Const SEC_GRP_FOOTER As Integer = 6

Private Const CLR_ACCENT As Long = 7949855      ' RGB(31, 78, 121)
Private Const CLR_MUTED As Long = 8421504       ' RGB(128, 128, 128)
Private Const CLR_BAND As Long = 16380660       ' RGB(244, 242, 249) light row shade

' No message boxes; errors go to CairnReports_error.txt (for automated builds)
Public Sub BuildCairnReportsQuiet()
    BuildCairnReports True
End Sub

Public Sub BuildCairnReports(Optional ByVal quiet As Boolean = False)
    Dim stepName As String
    On Error GoTo Fail
    stepName = "removing old reports": DropAllReports
    stepName = "rptQuote": BuildRptQuote
    stepName = "rptReceipt": BuildRptReceipt
    stepName = "rptCommission": BuildRptCommission
    stepName = "rptInvoiceLines": BuildRptInvoiceLines
    stepName = "rptInvoice": BuildRptInvoice
    If QueryExists("qryRMARouting") Then stepName = "rptRMASummary": BuildRptRMASummary
    Application.RefreshDatabaseWindow
    If Not quiet Then MsgBox "Cairn Systems reports built.", vbInformation, "Cairn Systems"
    Exit Sub
Fail:
    Dim msg As String: msg = "Error " & Err.Number & " while building " & stepName & ": " & Err.Description
    DiscardTemp
    If quiet Then
        Dim fh As Integer: fh = FreeFile
        Open CurrentProject.Path & "\CairnReports_error.txt" For Output As #fh
        Print #fh, msg
        Close #fh
    Else
        MsgBox msg, vbCritical, "Report step failed"
    End If
End Sub

'------------------------------------------------------------------------------
'  QUOTE
'------------------------------------------------------------------------------
Private Sub BuildRptQuote()
    Dim r As Report: Set r = NewReport("qryQuotePrint", "Quote")
    Const H As Integer = SEC_GRP_HEADER, F As Integer = SEC_GRP_FOOTER, D As Integer = SEC_DETAIL
    Dim c As Control

    CreateGroupLevel r.Name, "QuoteNumber", True, True
    CreateGroupLevel r.Name, "QuoteLineID", False, False
    r.Section(H).ForceNewPage = 1
    r.Section(H).KeepTogether = True

    Letterhead r, H, "QUOTE"
    InfoRow r, H, "Quote #", "QuoteNumber", 0.6
    InfoRow r, H, "Date", "QuoteDate", 0.8
    InfoRow r, H, "Valid Until", "ValidUntil", 1
    InfoRow r, H, "Sales Rep", "RepName", 1.2

    HLine r, H, 0, 1.5, PAGE_W
    Caps r, H, "PREPARED FOR", 0, 1.6, 3
    AddressBlock r, H, 1.8
    Tb r, H, "=IIf(Len(Trim(Nz([ContactName],"""")))=0,Null,""Attn: "" & [ContactName])", 0, 2.4, 3.8
    Tb r, H, "=Trim(Nz([ContactEmail],"""") & ""   "" & Nz([ContactPhone],""""))", 0, 2.6, 3.8
    Caps r, H, "YOUR SALES REP", 4.5, 1.6, 3
    Tb(r, H, "RepName", 4.5, 1.8, 3).FontBold = True
    Tb r, H, "RepEmail", 4.5, 2, 3
    Tb r, H, "RepPhone", 4.5, 2.2, 3

    LineHeadings r, H, 3
    r.Section(H).Height = I2T(3.3)

    LineDetail r, D
    r.Section(D).Height = I2T(0.26)

    HLine r, F, 4.5, 0.08, PAGE_W - 4.5
    TotalRow r, F, "Subtotal", "=[Subtotal]", 0.15
    TotalRow r, F, "=""Tax ("" & Format([TaxRate],""0.00%"") & "")""", "=[TaxAmount]", 0.37
    TotalRow r, F, "Shipping", "=[ShippingCost]", 0.59
    Set c = TotalRow(r, F, "Quote Total", "=[QuoteTotal]", 0.85, True)

    Caps r, F, "TERMS", 0, 0.15, 4
    Set c = Tb(r, F, "QuoteTerms", 0, 0.35, 4.2, 0.2)
    c.CanGrow = True: c.FontSize = 9
    Set c = Tb(r, F, "=IIf(IsNull([Notes]),Null,""Notes: "" & [Notes])", 0, 1.25, PAGE_W, 0.2)
    c.CanGrow = True: c.CanShrink = True: c.FontSize = 9
    Lb r, F, "Accepted by: ________________________________     Title: ________________     Date: ____________", 0, 1.7, PAGE_W
    Set c = Tb(r, F, "=""To accept, sign and return this quote or reply to "" & [RepName] & "" ("" & [RepEmail] & "").""", 0, 2.05, PAGE_W)
    c.FontSize = 9: c.ForeColor = CLR_MUTED
    r.Section(F).CanGrow = True
    r.Section(F).KeepTogether = True
    r.Section(F).Height = I2T(2.35)

    PageFooter r, "=""Quote "" & [QuoteNumber] & ""   -   Page "" & [Page]"
    SaveReport r, "rptQuote"
End Sub

'------------------------------------------------------------------------------
'  RECEIPT
'------------------------------------------------------------------------------
Private Sub BuildRptReceipt()
    Dim r As Report: Set r = NewReport("qryReceiptPrint", "Receipt")
    Const H As Integer = SEC_GRP_HEADER, F As Integer = SEC_GRP_FOOTER, D As Integer = SEC_DETAIL
    Dim c As Control

    CreateGroupLevel r.Name, "PaymentID", True, True
    CreateGroupLevel r.Name, "OrderLineID", False, False
    r.Section(H).ForceNewPage = 1
    r.Section(H).KeepTogether = True

    Letterhead r, H, "RECEIPT"
    InfoRow r, H, "Receipt #", "=Format([PaymentID],""00000"")", 0.6
    InfoRow r, H, "Payment Date", "PaymentDate", 0.8
    InfoRow r, H, "Order #", "OrderNumber", 1
    InfoRow r, H, "Customer PO", "CustomerPO", 1.2

    HLine r, H, 0, 1.5, PAGE_W
    Caps r, H, "RECEIVED FROM", 0, 1.6, 3
    AddressBlock r, H, 1.8

    Set c = Tb(r, H, "=""Payment received: "" & Format([PaymentAmount],""Currency"") & "" by "" & Nz([PaymentMethod],""payment"") & IIf(IsNull([PaymentRef]),"""","" (ref "" & [PaymentRef] & "")"")", 0, 2.55, PAGE_W, 0.3)
    c.FontSize = 13: c.FontBold = True: c.ForeColor = CLR_ACCENT
    Set c = Tb(r, H, "=""Items on order "" & [OrderNumber] & "" (ordered "" & Format([OrderDate],""Short Date"") & "")""", 0, 2.95, PAGE_W)
    c.FontSize = 9: c.ForeColor = CLR_MUTED
    LineHeadings r, H, 3.2
    r.Section(H).Height = I2T(3.5)

    LineDetail r, D
    r.Section(D).Height = I2T(0.26)

    HLine r, F, 4.5, 0.08, PAGE_W - 4.5
    TotalRow r, F, "Order Subtotal", "=[Subtotal]", 0.15
    TotalRow r, F, "Tax", "=[TaxAmount]", 0.37
    TotalRow r, F, "Shipping", "=[ShippingCost]", 0.59
    TotalRow r, F, "Order Total", "=[OrderTotal]", 0.81, True
    TotalRow r, F, "This Payment", "=[PaymentAmount]", 1.1
    TotalRow r, F, "Paid to Date", "=ReceiptPaidToDate([PaymentID])", 1.32
    TotalRow r, F, "Balance Remaining", "=[OrderTotal]-ReceiptPaidToDate([PaymentID])", 1.6, True

    Set c = Tb(r, F, "=""Sales rep: "" & [RepName]", 0, 0.15, 4)
    c.FontSize = 9
    Set c = Tb(r, F, "ReceiptFooter", 0, 2.05, PAGE_W, 0.2)
    c.CanGrow = True: c.FontItalic = True: c.TextAlign = 2
    r.Section(F).CanGrow = True
    r.Section(F).KeepTogether = True
    r.Section(F).Height = I2T(2.35)

    PageFooter r, "=""Receipt "" & Format([PaymentID],""00000"") & ""   -   Page "" & [Page]"
    SaveReport r, "rptReceipt"
End Sub

'------------------------------------------------------------------------------
'  COMMISSION STATEMENT
'------------------------------------------------------------------------------
Private Sub BuildRptCommission()
    Dim r As Report: Set r = NewReport("qryCommissionDetail", "Commission Statements")
    Const H As Integer = SEC_GRP_HEADER, F As Integer = SEC_GRP_FOOTER, D As Integer = SEC_DETAIL
    Dim c As Control, i As Integer

    CreateGroupLevel r.Name, "RepName", True, True
    CreateGroupLevel r.Name, "OrderDate", False, False
    r.Section(H).ForceNewPage = 1
    r.Section(H).KeepTogether = True

    Set c = Tb(r, H, "=DLookUp(""CompanyName"",""CompanyInfo"")", 0, 0, 4.5, 0.3)
    c.FontSize = 14: c.FontBold = True
    Set c = Lb(r, H, "COMMISSION STATEMENT", 3.5, 0, 4, 0.4)
    c.FontSize = 18: c.FontBold = True: c.ForeColor = CLR_ACCENT: c.TextAlign = 3
    Set c = Tb(r, H, "=CommissionPeriod()", 3.5, 0.45, 4, 0.3)
    c.FontSize = 13: c.TextAlign = 3

    HLine r, H, 0, 0.9, PAGE_W
    Caps r, H, "SALES REP", 0, 1, 3
    Set c = Tb(r, H, "RepName", 0, 1.2, 4, 0.3)
    c.FontSize = 13: c.FontBold = True
    Tb r, H, "RepEmail", 0, 1.5, 4
    Caps r, H, "COMMISSION RATE", 4.5, 1, 3
    Set c = Tb(r, H, "=Format([CommissionRate],""0.0%"") & "" of net sales""", 4.5, 1.2, 3, 0.3)
    c.FontSize = 13: c.FontBold = True
    Set c = Tb(r, H, "=""Net sales = order lines after discounts, before tax and shipping. Cancelled orders excluded.""", 0, 1.8, PAGE_W)
    c.FontSize = 8: c.ForeColor = CLR_MUTED

    Dim heads, xs, ws, srcs, aligns
    heads = Array("Order #", "Date", "Customer", "Status", "Net Sales", "Balance Due", "Commission")
    srcs = Array("OrderNumber", "OrderDate", "CustomerName", "OrderStatus", "NetSales", "BalanceDue", "Commission")
    xs = Array(0, 1, 1.8, 3.75, 4.7, 5.7, 6.7)
    ws = Array(1, 0.8, 1.95, 0.95, 1, 1, 0.8)
    aligns = Array(1, 1, 1, 1, 3, 3, 3)
    For i = 0 To UBound(heads)
        HeadCell r, H, CStr(heads(i)), CSng(xs(i)), 2.1, CSng(ws(i)), CInt(aligns(i))
        Set c = Tb(r, D, CStr(srcs(i)), CSng(xs(i)), 0.03, CSng(ws(i)) - 0.05)
        c.TextAlign = aligns(i): c.FontSize = 10
        If i >= 4 Then c.Format = "Currency"
        If i = 1 Then c.Format = "Short Date"
        If i = 2 Then c.CanGrow = True
    Next
    r.Section(H).Height = I2T(2.45)
    r.Section(D).Height = I2T(0.26)
    r.Section(D).AlternateBackColor = CLR_BAND

    HLine r, F, 0, 0.06, PAGE_W
    Set c = Tb(r, F, "=Count(*) & IIf(Count(*)=1,"" order"","" orders"")", 0, 0.12, 3)
    c.FontBold = True
    SumCell r, F, "=Sum([NetSales])", 4.7, 0.12, 0.95
    SumCell r, F, "=Sum([BalanceDue])", 5.7, 0.12, 0.95
    SumCell r, F, "=Sum([Commission])", 6.7, 0.12, 0.8
    Set c = Tb(r, F, "=""Commission payable: "" & Format(Sum([Commission]),""Currency"")", 3, 0.55, PAGE_W - 3, 0.32)
    c.FontSize = 14: c.FontBold = True: c.ForeColor = CLR_ACCENT: c.TextAlign = 3
    Set c = Tb(r, F, "=IIf(Sum([BalanceDue])>0.005,""Note: "" & Format(Sum([BalanceDue]),""Currency"") & "" of these orders is still unpaid."",Null)", 0, 1, PAGE_W)
    c.FontSize = 9: c.ForeColor = CLR_MUTED: c.CanShrink = True
    Lb r, F, "Approved by: ________________________________          Date: ________________", 0, 1.5, PAGE_W
    Lb r, F, "Rep signature: ______________________________          Date: ________________", 0, 1.9, PAGE_W
    r.Section(F).KeepTogether = True
    r.Section(F).Height = I2T(2.25)

    ' Summary page for all reps
    If AddReportFooter(r) Then
        Const RF As Integer = SEC_RPT_FOOTER
        r.Section(RF).ForceNewPage = 1
        Set c = Lb(r, RF, "COMMISSION SUMMARY - ALL REPS", 0, 0, 5, 0.4)
        c.FontSize = 16: c.FontBold = True: c.ForeColor = CLR_ACCENT
        Set c = Tb(r, RF, "=CommissionPeriod()", 0, 0.45, 5, 0.3)
        c.FontSize = 13
        HLine r, RF, 0, 0.9, PAGE_W
        Lb r, RF, "Orders", 0, 1.05, 2
        Tb(r, RF, "=Count(*)", 2.2, 1.05, 1.5).TextAlign = 3
        Lb r, RF, "Net sales", 0, 1.3, 2
        SumCell r, RF, "=Sum([NetSales])", 2.2, 1.3, 1.5
        Lb r, RF, "Unpaid balance on these orders", 0, 1.55, 2.2
        SumCell r, RF, "=Sum([BalanceDue])", 2.2, 1.55, 1.5
        Set c = Lb(r, RF, "Total commission payable", 0, 1.85, 2.2)
        c.FontBold = True
        Set c = SumCell(r, RF, "=Sum([Commission])", 2.2, 1.85, 1.5)
        c.FontSize = 12
        Set c = Tb(r, RF, "=""Individual statements for each rep are on the preceding pages.""", 0, 2.3, PAGE_W)
        c.FontSize = 9: c.ForeColor = CLR_MUTED
        r.Section(RF).Height = I2T(2.6)
    End If

    PageFooter r, "=""Commission statements - "" & CommissionPeriod() & ""   -   Page "" & [Page] & "" of "" & [Pages]"
    SaveReport r, "rptCommission"
End Sub

'------------------------------------------------------------------------------
'  INVOICE (fixed layout: window-envelope address, line box, remittance stub)
'------------------------------------------------------------------------------
Private Sub BuildRptInvoiceLines()
    Dim r As Report: Set r = NewReport("SELECT * FROM qryOrderLines ORDER BY OrderLineID", "Invoice Lines")
    Dim c As Control
    LineDetail r, SEC_DETAIL
    For Each c In r.Section(SEC_DETAIL).Controls
        c.CanGrow = False
    Next
    r.Section(SEC_DETAIL).CanGrow = False
    r.Section(SEC_DETAIL).Height = I2T(0.26)
    On Error Resume Next
    r.Section(SEC_PAGE_FOOTER).Height = 0
    r.Section(SEC_PAGE_FOOTER).Visible = False
    On Error GoTo 0
    SaveReport r, "rptInvoiceLines"
End Sub

Private Sub BuildRptInvoice()
    Dim r As Report: Set r = NewReport("qryInvoicePrint", "Invoice")
    Const H As Integer = SEC_GRP_HEADER, D As Integer = SEC_DETAIL
    Dim c As Control, y As Single

    CreateGroupLevel r.Name, "OrderNumber", True, False
    r.Section(H).ForceNewPage = 1
    r.Section(H).KeepTogether = True

    Letterhead r, H, "INVOICE"
    InfoRow r, H, "Invoice #", "OrderNumber", 0.6
    InfoRow r, H, "Invoice Date", "=Format(Nz([ShipDate],[OrderDate]),""mmm d, yyyy"")", 0.8
    InfoRow r, H, "Terms", "PaymentTerms", 1
    InfoRow r, H, "Customer PO", "CustomerPO", 1.2
    HLine r, H, 0, 1.5, PAGE_W
    Caps r, H, "BILL TO", 0, 1.75, 3
    AddressBlock r, H, 1.95

    y = 3
    LineHeadings r, H, y
    ' A fixed box: it doesn't grow, so lines past INVOICE_LINES are cut off
    Set c = CreateReportControl(r.Name, acSubform, H, , , 0, I2T(y + 0.26), I2T(PAGE_W), I2T(INVOICE_LINES * 0.26 + 0.02))
    c.Name = "subLines"
    c.SourceObject = "Report.rptInvoiceLines"
    c.LinkMasterFields = "OrderID": c.LinkChildFields = "OrderID"
    c.CanGrow = False: c.CanShrink = False
    c.BorderStyle = 0
    y = y + 0.26 + INVOICE_LINES * 0.26 + 0.15

    HLine r, H, 4.5, y, PAGE_W - 4.5
    TotalRow r, H, "Subtotal", "=[Subtotal]", y + 0.1
    TotalRow r, H, "=""Tax ("" & Format([TaxRate],""0.00%"") & "")""", "=[TaxAmount]", y + 0.32
    TotalRow r, H, "Shipping", "=[ShippingCost]", y + 0.54
    TotalRow r, H, "Invoice Total", "=[OrderTotal]", y + 0.8, True
    TotalRow r, H, "Payments Received", "=[AmountPaid]", y + 1.1
    TotalRow r, H, "Amount Due", "=[BalanceDue]", y + 1.35, True
    Set c = Tb(r, H, "=""Shipped "" & Format([ShipDate],""mmm d, yyyy"") & "" via "" & [ShipVia] & IIf(IsNull([TrackingNumber]),"""","" - tracking "" & [TrackingNumber])", 0, y + 0.1, 4.3)
    c.FontSize = 9
    Set c = Tb(r, H, "=""Questions? Contact "" & [RepName] & "" or call "" & [SellerPhone] & "".""", 0, y + 0.35, 4.3)
    c.FontSize = 9

    ' Remittance stub
    y = y + 2
    Set c = Lb(r, H, "- - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -  please detach and return this portion with your payment  - - - - - - - - - - - - - - - - - - - - - - - - - - - - - - -", 0, y, PAGE_W)
    c.FontSize = 7: c.ForeColor = CLR_MUTED: c.TextAlign = 2
    Caps r, H, "REMIT TO", 0, y + 0.3, 3
    Tb(r, H, "SellerName", 0, y + 0.5, 3.5).FontBold = True
    Tb r, H, "SellerAddress", 0, y + 0.7, 3.5
    Tb r, H, "SellerCityStateZip", 0, y + 0.9, 3.5
    InfoRow r, H, "Invoice #", "OrderNumber", y + 0.3
    InfoRow r, H, "Customer", "CustomerName", y + 0.5
    InfoRow r, H, "Amount Due", "=Format([BalanceDue],""Currency"")", y + 0.7
    Lb r, H, "Amount Enclosed: ______________", 4.5, y + 1.05, 3
    r.Section(H).Height = I2T(y + 1.4)

    r.Section(D).Height = 0
    r.Section(D).Visible = False
    PageFooter r, "=""Invoice "" & [OrderNumber]"
    SaveReport r, "rptInvoice"
End Sub

'------------------------------------------------------------------------------
'  RMA SUMMARY
'------------------------------------------------------------------------------
Private Sub BuildRptRMASummary()
    Dim r As Report: Set r = NewReport("qryRMARouting", "RMA Summary")
    Const H As Integer = SEC_GRP_HEADER, F As Integer = SEC_GRP_FOOTER, D As Integer = SEC_DETAIL
    Dim c As Control, i As Integer

    CreateGroupLevel r.Name, "Route", True, True
    CreateGroupLevel r.Name, "OpenedDate", False, False
    r.Section(H).KeepTogether = True
    r.Section(H).RepeatSection = True

    If AddReportFooter(r) Then
        Set c = Tb(r, 1, "=DLookUp(""CompanyName"",""CompanyInfo"")", 0, 0, 4, 0.3)
        c.FontSize = 14: c.FontBold = True
        Set c = Lb(r, 1, "RMA SUMMARY", 3.5, 0, 4, 0.4)
        c.FontSize = 18: c.FontBold = True: c.ForeColor = CLR_ACCENT: c.TextAlign = 3
        Set c = Tb(r, 1, "=CommissionPeriod() & ""  -  "" & Count(*) & "" returns""", 3.5, 0.45, 4, 0.3)
        c.FontSize = 12: c.TextAlign = 3
        HLine r, 1, 0, 0.85, PAGE_W
        r.Section(1).Height = I2T(1)
        Set c = Tb(r, SEC_RPT_FOOTER, "=""Routes come from the saved query qryRMARouting. "" & Count(*) & "" returns in total.""", 0, 0.1, PAGE_W)
        c.FontSize = 9: c.ForeColor = CLR_MUTED
        r.Section(SEC_RPT_FOOTER).Height = I2T(0.4)
    End If

    Set c = Tb(r, H, "=[Route] & ""  ("" & Count(*) & "")""", 0, 0.08, 5, 0.28)
    c.FontSize = 12: c.FontBold = True: c.ForeColor = CLR_ACCENT
    Dim heads, xs, ws, srcs
    heads = Array("RMA #", "Opened", "Customer", "SKU", "Serial #", "Reason", "Days", "Status")
    srcs = Array("RMANumber", "OpenedDate", "CustomerName", "SKU", "SerialNumber", "Reason", "DaysInService", "RMAStatus")
    xs = Array(0, 1, 1.75, 3.6, 4.4, 5.35, 6.5, 6.95)
    ws = Array(1, 0.75, 1.85, 0.8, 0.95, 1.15, 0.45, 0.55)
    For i = 0 To UBound(heads)
        HeadCell r, H, CStr(heads(i)), CSng(xs(i)), 0.42, CSng(ws(i)), IIf(i = 6, 3, 1)
        Set c = Tb(r, D, CStr(srcs(i)), CSng(xs(i)), 0.03, CSng(ws(i)) - 0.05)
        c.FontSize = 8
        If i = 1 Then c.Format = "Short Date"
        If i = 6 Then c.TextAlign = 3
    Next
    r.Section(H).Height = I2T(0.7)
    r.Section(D).Height = I2T(0.22)
    r.Section(D).AlternateBackColor = CLR_BAND
    r.Section(F).Height = I2T(0.15)

    PageFooter r, "=""RMA summary - "" & CommissionPeriod() & ""   -   Page "" & [Page] & "" of "" & [Pages]"
    SaveReport r, "rptRMASummary"
End Sub

'==============================================================================
'  RUNTIME FUNCTIONS (called from buttons and report controls - keep Public)
'==============================================================================

' Sales Orders form: preview the invoice for this order
Public Function PrintInvoice(frm As Form)
    If frm.Dirty Then frm.Dirty = False
    If IsNull(frm!OrderID) Then
        MsgBox "Save the order first.", vbExclamation, "Cairn Systems"
        Exit Function
    End If
    DoCmd.OpenReport "rptInvoice", acViewPreview, WhereCondition:="OrderID=" & frm!OrderID
End Function

' Main menu: ask for the month, then preview the RMA summary
Public Function PrintRMASummary(frm As Form)
    Dim s As String, d1 As Date, crit As String
    s = InputBox("RMA summary for which month?" & vbCrLf & vbCrLf & _
                 "Enter any date in that month.", "RMA Summary", _
                 Format(DateSerial(Year(Date), Month(Date) - 1, 1), "Short Date"))
    If Len(s) = 0 Then Exit Function
    If Not IsDate(s) Then
        MsgBox "'" & s & "' is not a date.", vbExclamation, "Cairn Systems"
        Exit Function
    End If
    d1 = DateSerial(Year(CDate(s)), Month(CDate(s)), 1)
    crit = "OpenedDate >= " & SqlDate(d1) & " AND OpenedDate < " & SqlDate(DateAdd("m", 1, d1))
    If DCount("*", "qryRMARouting", crit) = 0 Then
        MsgBox "No returns were opened in " & Format(d1, "mmmm yyyy") & ".", vbInformation, "Cairn Systems"
        Exit Function
    End If
    mPeriod = Format(d1, "mmmm yyyy")
    DoCmd.OpenReport "rptRMASummary", acViewPreview, WhereCondition:=crit
End Function

' Quotes form: preview the current quote
Public Function PrintQuote(frm As Form)
    If frm.Dirty Then frm.Dirty = False
    If IsNull(frm!QuoteID) Then
        MsgBox "Save the quote first.", vbExclamation, "Cairn Systems"
        Exit Function
    End If
    DoCmd.OpenReport "rptQuote", acViewPreview, WhereCondition:="QuoteID=" & frm!QuoteID
End Function

' Sales Orders form: preview a receipt for the payment selected in the Payments list
Public Function PrintReceipt(frm As Form)
    Dim pay As Form: Set pay = frm!subPayments.Form
    If pay.Dirty Then pay.Dirty = False
    If pay.NewRecord Or IsNull(pay!PaymentID) Then
        MsgBox "Click on a payment in the Payments list first, then press Print Receipt.", vbExclamation, "Cairn Systems"
        Exit Function
    End If
    DoCmd.OpenReport "rptReceipt", acViewPreview, WhereCondition:="PaymentID=" & pay!PaymentID
End Function

' Main menu: ask for the month, then preview the commission statements
Public Function PrintCommission(frm As Form)
    Dim s As String, d1 As Date, crit As String
    s = InputBox("Commission statements for which month?" & vbCrLf & vbCrLf & _
                 "Enter any date in that month.", "Commission Statements", _
                 Format(DateSerial(Year(Date), Month(Date) - 1, 1), "Short Date"))
    If Len(s) = 0 Then Exit Function
    If Not IsDate(s) Then
        MsgBox "'" & s & "' is not a date.", vbExclamation, "Cairn Systems"
        Exit Function
    End If
    d1 = DateSerial(Year(CDate(s)), Month(CDate(s)), 1)
    crit = CommissionCriteria(d1)
    If DCount("*", "qryCommissionDetail", crit) = 0 Then
        MsgBox "No orders were booked in " & Format(d1, "mmmm yyyy") & ".", vbInformation, "Cairn Systems"
        Exit Function
    End If
    mPeriod = Format(d1, "mmmm yyyy")
    DoCmd.OpenReport "rptCommission", acViewPreview, WhereCondition:=crit
End Function

Public Function CommissionCriteria(ByVal monthStart As Date) As String
    CommissionCriteria = "OrderDate >= " & SqlDate(monthStart) & " AND OrderDate < " & SqlDate(DateAdd("m", 1, monthStart))
End Function

Public Function ClearCommissionPeriod()
    mPeriod = ""
End Function

Public Function CommissionPeriod() As String
    CommissionPeriod = IIf(Len(mPeriod), mPeriod, "All dates")
End Function

' Total paid on the order up to and including this payment
Public Function ReceiptPaidToDate(ByVal paymentID As Long) As Currency
    Dim oid As Variant, d As Variant
    oid = DLookup("OrderID", "Payments", "PaymentID=" & paymentID)
    d = DLookup("PaymentDate", "Payments", "PaymentID=" & paymentID)
    If IsNull(oid) Then Exit Function
    ReceiptPaidToDate = Nz(DSum("Amount", "Payments", "OrderID=" & oid & " AND (PaymentDate < " & SqlDate(d) & _
                        " OR (PaymentDate = " & SqlDate(d) & " AND PaymentID <= " & paymentID & "))"), 0)
End Function

Private Function SqlDate(ByVal d As Variant) As String
    SqlDate = Format(d, "\#yyyy\-mm\-dd hh\:nn\:ss\#")
End Function

'==============================================================================
'  BUILD HELPERS
'==============================================================================
Private Function I2T(ByVal inches As Single) As Long
    I2T = CLng(inches * 1440)
End Function

Private Function NewReport(ByVal recSrc As String, ByVal cap As String) As Report
    Dim r As Report
    Set r = CreateReport
    mTmp = r.Name
    r.RecordSource = recSrc
    r.OnClose = "=ClearCommissionPeriod()"
    r.Caption = cap
    r.Width = I2T(PAGE_W)
    On Error Resume Next            ' printer settings are optional
    r.Printer.TopMargin = 720: r.Printer.BottomMargin = 720
    r.Printer.LeftMargin = 720: r.Printer.RightMargin = 720
    r.Section(SEC_PAGE_HEADER).Height = 0
    r.Section(SEC_PAGE_HEADER).Visible = False
    Set NewReport = r
End Function

Private Sub SaveReport(r As Report, ByVal rptName As String)
    Dim tmp As String: tmp = r.Name
    Dim s As Integer
    On Error Resume Next            ' only the detail rows get alternate shading
    For s = 1 To 8
        If s <> SEC_DETAIL Then r.Section(s).AlternateBackColor = vbWhite: r.Section(s).BackColor = vbWhite
    Next
    On Error GoTo 0
    DoCmd.Close acReport, tmp, acSaveYes
    DoCmd.Rename rptName, acReport, tmp
    mTmp = ""
End Sub

Private Sub DiscardTemp()
    On Error Resume Next
    If Len(mTmp) Then
        DoCmd.Close acReport, mTmp, acSaveNo
        DoCmd.DeleteObject acReport, mTmp
    End If
    mTmp = ""
End Sub

Private Sub DropAllReports()
    Dim n, ao As AccessObject
    For Each n In Split(REPORT_LIST, ";")
        For Each ao In CurrentProject.AllReports
            If ao.Name = n Then
                DoCmd.Close acReport, n, acSaveNo
                DoCmd.DeleteObject acReport, n
                Exit For
            End If
        Next
    Next
End Sub

Private Function QueryExists(ByVal qName As String) As Boolean
    Dim q As DAO.QueryDef
    For Each q In CurrentDb.QueryDefs
        If q.Name = qName Then QueryExists = True: Exit Function
    Next
End Function

Private Function AddReportFooter(r As Report) As Boolean
    On Error Resume Next
    Dim h As Long: h = r.Section(SEC_RPT_FOOTER).Height
    If Err.Number <> 0 Then
        Err.Clear
        DoCmd.SelectObject acReport, r.Name
        DoCmd.RunCommand acCmdReportHdrFtr
        h = r.Section(SEC_RPT_FOOTER).Height
    End If
    AddReportFooter = (Err.Number = 0)
    If AddReportFooter Then r.Section(1).Height = 0
End Function

' Text box: field name or "=expression"
Private Function Tb(r As Report, ByVal sec As Integer, ByVal src As String, ByVal x As Single, ByVal y As Single, _
                    ByVal w As Single, Optional ByVal h As Single = 0.2) As Control
    Static n As Long
    Dim c As Control
    Set c = CreateReportControl(r.Name, acTextBox, sec, , IIf(Left(src, 1) = "=", "", src), I2T(x), I2T(y), I2T(w), I2T(h))
    If Left(src, 1) = "=" Then c.ControlSource = src
    n = n + 1
    c.Name = "t" & n & "_" & IIf(Left(src, 1) = "=", "calc", src)
    c.BorderStyle = 0
    c.BackStyle = 0
    c.FontSize = 10
    Set Tb = c
End Function

Private Function Lb(r As Report, ByVal sec As Integer, ByVal cap As String, ByVal x As Single, ByVal y As Single, _
                    ByVal w As Single, Optional ByVal h As Single = 0.2) As Control
    Static n As Long
    Dim c As Control
    Set c = CreateReportControl(r.Name, acLabel, sec, , , I2T(x), I2T(y), I2T(w), I2T(h))
    n = n + 1
    c.Name = "l" & n
    c.Caption = cap
    c.FontSize = 10
    Set Lb = c
End Function

' Small grey heading such as PREPARED FOR
Private Function Caps(r As Report, ByVal sec As Integer, ByVal cap As String, ByVal x As Single, ByVal y As Single, ByVal w As Single) As Control
    Dim c As Control: Set c = Lb(r, sec, cap, x, y, w)
    c.FontSize = 8: c.FontBold = True: c.ForeColor = CLR_MUTED
    Set Caps = c
End Function

Private Sub HLine(r As Report, ByVal sec As Integer, ByVal x As Single, ByVal y As Single, ByVal w As Single)
    Dim c As Control
    Set c = CreateReportControl(r.Name, acLine, sec, , , I2T(x), I2T(y), I2T(w), 0)
    c.BorderColor = CLR_ACCENT
End Sub

' Seller name/address on the left, big document title on the right
Private Sub Letterhead(r As Report, ByVal sec As Integer, ByVal docTitle As String)
    Dim c As Control
    Set c = Tb(r, sec, "SellerName", 0, 0, 4.5, 0.32)
    c.FontSize = 16: c.FontBold = True: c.ForeColor = CLR_ACCENT
    Tb r, sec, "SellerAddress", 0, 0.38, 4.5
    Tb r, sec, "SellerCityStateZip", 0, 0.58, 4.5
    Tb r, sec, "=""Phone "" & [SellerPhone]", 0, 0.78, 4.5
    Tb r, sec, "SellerWebsite", 0, 0.98, 4.5
    Set c = Lb(r, sec, docTitle, 4.5, 0, 3, 0.45)
    c.FontSize = 22: c.FontBold = True: c.ForeColor = CLR_ACCENT: c.TextAlign = 3
End Sub

' "Label: value" pair in the top-right block
Private Sub InfoRow(r As Report, ByVal sec As Integer, ByVal cap As String, ByVal src As String, ByVal y As Single)
    Dim c As Control
    Set c = Lb(r, sec, cap, 4.7, y, 1.3)
    c.ForeColor = CLR_MUTED: c.TextAlign = 3
    Set c = Tb(r, sec, src, 6.05, y, 1.45)
    c.TextAlign = 3: c.FontBold = True
    If src Like "*Date" Or src = "ValidUntil" Then c.Format = "mmm d, yyyy"
End Sub

Private Sub AddressBlock(r As Report, ByVal sec As Integer, ByVal y As Single)
    Tb(r, sec, "CustomerName", 0, y, 4).FontBold = True
    Tb r, sec, "BillAddress", 0, y + 0.2, 4
    Tb r, sec, "=[BillCity] & "", "" & [BillState] & "" "" & [BillZip]", 0, y + 0.4, 4
End Sub

Private Function HeadCell(r As Report, ByVal sec As Integer, ByVal cap As String, ByVal x As Single, ByVal y As Single, _
                          ByVal w As Single, ByVal align As Integer) As Control
    Dim c As Control: Set c = Lb(r, sec, cap, x, y, w, 0.24)
    c.BackStyle = 1: c.BackColor = CLR_ACCENT: c.ForeColor = vbWhite
    c.FontBold = True: c.FontSize = 9: c.TextAlign = align
    c.LeftMargin = 40: c.RightMargin = 60: c.TopMargin = 20
    Set HeadCell = c
End Function

' Column headings and detail row shared by quote and receipt
Private Sub LineHeadings(r As Report, ByVal sec As Integer, ByVal y As Single)
    HeadCell r, sec, "SKU", 0, y, 1.15, 1
    HeadCell r, sec, "Description", 1.15, y, 3.25, 1
    HeadCell r, sec, "Qty", 4.4, y, 0.6, 3
    HeadCell r, sec, "Unit Price", 5, y, 0.95, 3
    HeadCell r, sec, "Disc.", 5.95, y, 0.6, 3
    HeadCell r, sec, "Line Total", 6.55, y, 0.95, 3
End Sub

Private Sub LineDetail(r As Report, ByVal sec As Integer)
    Dim c As Control
    Tb r, sec, "SKU", 0.04, 0.03, 1.1
    Set c = Tb(r, sec, "ItemDescription", 1.19, 0.03, 3.15): c.CanGrow = True
    Tb(r, sec, "Quantity", 4.4, 0.03, 0.55).TextAlign = 3
    Set c = Tb(r, sec, "UnitPrice", 5, 0.03, 0.9): c.TextAlign = 3: c.Format = "Currency"
    Set c = Tb(r, sec, "=IIf(Nz([DiscountPct],0)=0,Null,[DiscountPct])", 5.95, 0.03, 0.55): c.TextAlign = 3: c.Format = "0%"
    Set c = Tb(r, sec, "LineTotal", 6.55, 0.03, 0.9): c.TextAlign = 3: c.Format = "Currency"
    r.Section(sec).CanGrow = True
    r.Section(sec).AlternateBackColor = CLR_BAND
End Sub

Private Function TotalRow(r As Report, ByVal sec As Integer, ByVal cap As String, ByVal src As String, ByVal y As Single, _
                          Optional ByVal strong As Boolean) As Control
    Dim c As Control
    If Left(cap, 1) = "=" Then Set c = Tb(r, sec, cap, 4.5, y, 1.8) Else Set c = Lb(r, sec, cap, 4.5, y, 1.8)
    c.TextAlign = 3
    If strong Then c.FontBold = True
    Set c = Tb(r, sec, src, 6.3, y, 1.2, IIf(strong, 0.24, 0.2))
    c.Format = "Currency": c.TextAlign = 3
    If strong Then c.FontBold = True: c.FontSize = 12
    Set TotalRow = c
End Function

Private Function SumCell(r As Report, ByVal sec As Integer, ByVal src As String, ByVal x As Single, ByVal y As Single, _
                         ByVal w As Single) As Control
    Dim c As Control: Set c = Tb(r, sec, src, x, y, w)
    c.Format = "Currency": c.TextAlign = 3: c.FontBold = True
    Set SumCell = c
End Function

Private Sub PageFooter(r As Report, ByVal expr As String)
    On Error Resume Next            ' skip if the report has no page footer
    Dim c As Control
    Set c = Tb(r, SEC_PAGE_FOOTER, expr, 0, 0.05, PAGE_W)
    c.FontSize = 8: c.ForeColor = CLR_MUTED: c.TextAlign = 2
    r.Section(SEC_PAGE_FOOTER).Height = I2T(0.3)
End Sub
