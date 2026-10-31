Attribute VB_Name = "CairnForms"
'==============================================================================
'  CAIRN SYSTEMS - FORMS BUILDER
'------------------------------------------------------------------------------
'  Run AFTER BuildCairnDatabase. Import this file the same way (Alt+F11 >
'  File > Import File), click inside BuildCairnForms and press F5.
'  Safe to run again: existing Cairn forms are deleted and re-created.
'
'  FORMS
'    frmMainMenu       Start-up dashboard: buttons, open phone messages,
'                      follow-ups due in the next 7 days
'    frmCustomers      Customer details + tabs for Contacts, Activities,
'                      Quotes and Orders; New Quote / New Order buttons
'    frmQuotes         Quote header, line items, live totals,
'                      "Create Order" copies the quote into a sales order,
'                      "Print Quote" previews rptQuote (CairnReports module)
'    frmOrders         Order header, line items, payments, balance due,
'                      "Print Receipt" for the selected payment
'    frmProducts, frmSalesReps, frmCompanyInfo
'    frmActivities     CRM notes / calls / meetings / follow-ups
'    frmPhoneMessages  "While you were out" message pad
'    frmLogin          Sign-in (start-up form); the main menu sends you here
'                      until you sign in
'    frmUsers          User accounts (admins only)
'    frmRMAs           Returns, with the route from qryRMARouting
'    sfrm*             Subforms used inside the forms above
'
'  frmLogin, frmUsers, frmRMAs and the "Send to Manufacturer" button need the
'  tables from AddOperationsTables (CairnBuild).
'
'  The forms have no code-behind; their buttons and events call the Public
'  functions at the bottom of this module, so keep this module in the database.
'==============================================================================
Option Compare Database
Option Explicit

Private mTmp As String        ' design-view name of the form being built

Private Const FORM_LIST As String = _
    "frmMainMenu;frmCustomers;frmQuotes;frmOrders;frmProducts;frmSalesReps;frmCompanyInfo;" & _
    "frmActivities;frmPhoneMessages;sfrmContacts;sfrmCustomerActivities;sfrmCustomerQuotes;" & _
    "sfrmCustomerOrders;sfrmQuoteLines;sfrmOrderLines;sfrmPayments;sfrmOpenMessages;sfrmFollowUps;" & _
    "frmLogin;frmUsers;frmRMAs"

' Shared row sources
Private Const RS_REPS As String = "SELECT RepID, FirstName & ' ' & LastName AS RepName FROM SalesReps ORDER BY FirstName, LastName"
Private Const RS_CUSTOMERS As String = "SELECT CustomerID, CompanyName FROM Customers ORDER BY CompanyName"
Private Const RS_PRODUCTS As String = "SELECT ProductID, SKU, ProductName FROM Products WHERE Active ORDER BY SKU"
Private Const VL_QUOTESTATUS As String = "Draft;Sent;Accepted;Declined;Expired"
Private Const VL_ORDERSTATUS As String = "Open;In Production;Shipped;Invoiced;Closed;Cancelled"
Private Const VL_CUSTSTATUS As String = "Prospect;Active;Inactive"
Private Const VL_TERMS As String = "Due on Receipt;Net 15;Net 30;Net 45;Net 60"
Private Const VL_ACTTYPE As String = "Note;Call;Email;Meeting;Task"
Private Const VL_PAYMETHOD As String = "Check;ACH;Wire;Credit Card;Cash"
Private Const VL_MSGSTATUS As String = "New;Returned;Closed"
Private Const VL_CATEGORY As String = "Sensor;Gateway;Module;Accessory;Service;Kit"
Private Const VL_RMAREASON As String = "Dead on Arrival;Failed in Warranty;Damaged in Shipping;Wrong Item Shipped;Intermittent Fault;Customer Damage"
Private Const VL_RMASTATUS As String = "Open;Received;In Repair;Replaced;Credited;Closed;Rejected"

' No message boxes; errors go to CairnForms_error.txt (for automated builds)
Public Sub BuildCairnFormsQuiet()
    BuildCairnForms True
End Sub

Public Sub BuildCairnForms(Optional ByVal quiet As Boolean = False)
    Dim stepName As String
    On Error GoTo Fail

    stepName = "removing old forms": DropAllForms

    ' Subforms first (main forms embed them)
    stepName = "sfrmContacts": BuildSfrmContacts
    stepName = "sfrmCustomerActivities": BuildSfrmCustomerActivities
    stepName = "sfrmCustomerQuotes": BuildSfrmCustomerQuotes
    stepName = "sfrmCustomerOrders": BuildSfrmCustomerOrders
    stepName = "sfrmQuoteLines": BuildSfrmLines "sfrmQuoteLines", "QuoteLines", False
    stepName = "sfrmOrderLines": BuildSfrmLines "sfrmOrderLines", "OrderLines", True
    stepName = "sfrmPayments": BuildSfrmPayments
    stepName = "sfrmOpenMessages": BuildSfrmOpenMessages
    stepName = "sfrmFollowUps": BuildSfrmFollowUps

    stepName = "frmCompanyInfo": BuildFrmCompanyInfo
    stepName = "frmSalesReps": BuildFrmSalesReps
    stepName = "frmProducts": BuildFrmProducts
    stepName = "frmActivities": BuildFrmActivities
    stepName = "frmPhoneMessages": BuildFrmPhoneMessages
    stepName = "frmCustomers": BuildFrmCustomers
    stepName = "frmQuotes": BuildFrmQuotes
    stepName = "frmOrders": BuildFrmOrders
    Dim ops As Boolean: ops = TableExists("Users")
    If ops Then
        stepName = "frmLogin": BuildFrmLogin
        stepName = "frmUsers": BuildFrmUsers
        stepName = "frmRMAs": BuildFrmRMAs
    End If
    stepName = "frmMainMenu": BuildFrmMainMenu ops

    stepName = "start-up form": SetDbProp "StartupForm", IIf(ops, "frmLogin", "frmMainMenu")
    SetDbProp "AppTitle", "Cairn Systems"

    Application.RefreshDatabaseWindow
    If Not quiet Then
        MsgBox "Cairn Systems forms built." & vbCrLf & vbCrLf & _
               "It opens automatically next time.", vbInformation, "Cairn Systems"
        DoCmd.OpenForm IIf(ops, "frmLogin", "frmMainMenu")
    End If
    Exit Sub
Fail:
    Dim msg As String: msg = "Error " & Err.Number & " while building " & stepName & ": " & Err.Description
    DiscardTemp
    If quiet Then
        Dim fh As Integer: fh = FreeFile
        Open CurrentProject.Path & "\CairnForms_error.txt" For Output As #fh
        Print #fh, msg
        Close #fh
    Else
        MsgBox msg, vbCritical, "Form step failed"
    End If
End Sub

'------------------------------------------------------------------------------
'  SUBFORMS
'------------------------------------------------------------------------------
Private Sub BuildSfrmContacts()
    Dim f As Form: Set f = NewForm("Contacts", "Contacts", 2)
    Dim y As Single
    DsTxt f, "FirstName", "First Name", y, 1.1
    DsTxt f, "LastName", "Last Name", y, 1.1
    DsTxt f, "Title", "Title", y, 1.6
    DsTxt f, "Phone", "Phone", y, 1.1
    DsTxt f, "Mobile", "Mobile", y, 1.1
    DsTxt f, "Email", "Email", y, 2
    DsChk f, "IsPrimary", "Primary", y
    DsTxt f, "Notes", "Notes", y, 2
    SaveForm f, "sfrmContacts"
End Sub

Private Sub BuildSfrmCustomerActivities()
    Dim f As Form: Set f = NewForm("SELECT * FROM Activities ORDER BY ActivityDate DESC", "Activities", 2)
    Dim y As Single
    DsTxt f, "ActivityDate", "Date", y, 1.5
    DsCbo f, "ActivityType", "Type", y, VL_ACTTYPE, "1", 0.8
    DsTxt f, "Subject", "Subject", y, 2
    DsTxt f, "Details", "Details", y, 3
    DsCbo f, "ContactID", "Contact", y, _
        "SELECT ContactID, FirstName & ' ' & LastName AS ContactName FROM Contacts " & _
        "WHERE CustomerID=[Forms]![frmCustomers]![CustomerID] ORDER BY IsPrimary, FirstName", "0;1.5", 1.3
    DsCbo f, "RepID", "Rep", y, RS_REPS, "0;1.5", 1.2
    DsTxt f, "FollowUpDate", "Follow Up", y, 1
    DsChk f, "Completed", "Done", y
    f.BeforeInsert = Fn("ActivityDefaults")
    SaveForm f, "sfrmCustomerActivities"
End Sub

Private Sub BuildSfrmCustomerQuotes()
    Dim f As Form: Set f = NewForm("SELECT * FROM qryQuoteTotals ORDER BY QuoteDate DESC", "Quotes", 2)
    Dim y As Single
    ReadOnly f
    DsTxt(f, "QuoteID", "QuoteID", y).ColumnHidden = True
    DsTxt f, "QuoteNumber", "Quote #", y, 1.3
    DsTxt f, "QuoteDate", "Date", y, 1
    DsTxt f, "QuoteStatus", "Status", y, 1
    DsTxt(f, "QuoteTotal", "Total", y, 1.2).Format = "Currency"
    DblClickOpens f, "frmQuotes", "QuoteID"
    SaveForm f, "sfrmCustomerQuotes"
End Sub

Private Sub BuildSfrmCustomerOrders()
    Dim f As Form: Set f = NewForm("SELECT * FROM qryOrderBalances ORDER BY OrderDate DESC", "Orders", 2)
    Dim y As Single
    ReadOnly f
    DsTxt(f, "OrderID", "OrderID", y).ColumnHidden = True
    DsTxt f, "OrderNumber", "Order #", y, 1.3
    DsTxt f, "OrderDate", "Date", y, 1
    DsTxt f, "OrderStatus", "Status", y, 1.2
    DsTxt(f, "OrderTotal", "Total", y, 1.1).Format = "Currency"
    DsTxt(f, "AmountPaid", "Paid", y, 1.1).Format = "Currency"
    DsTxt(f, "BalanceDue", "Balance", y, 1.1).Format = "Currency"
    DblClickOpens f, "frmOrders", "OrderID"
    SaveForm f, "sfrmCustomerOrders"
End Sub

Private Sub BuildSfrmLines(ByVal formName As String, ByVal tbl As String, ByVal withSerials As Boolean)
    Dim f As Form: Set f = NewForm(tbl, "Line Items", 2)
    Dim y As Single, c As Control
    Set c = DsCbo(f, "ProductID", "Product", y, RS_PRODUCTS, "0;1.1;3", 1.3)
    c.AfterUpdate = Fn("LineProductChanged")
    DsTxt f, "LineDescription", "Description (blank = product name)", y, 2.8
    DsTxt f, "Quantity", "Qty", y, 0.6
    DsTxt(f, "UnitPrice", "Unit Price", y, 1).Format = "Currency"
    DsTxt(f, "DiscountPct", "Discount", y, 0.8).Format = "Percent"
    Set c = DsTxt(f, "=Round(Nz([Quantity],0)*Nz([UnitPrice],0)*(1-Nz([DiscountPct],0)),2)", "Line Total", y, 1.1, "LineTotal")
    c.Format = "Currency": c.Locked = True: c.TabStop = False
    If withSerials Then DsTxt f, "SerialNumbers", "Serial Numbers", y, 2
    f.AfterUpdate = Fn("RecalcParent")
    f.AfterDelConfirm = Fn("RecalcParent")
    SaveForm f, formName
End Sub

Private Sub BuildSfrmPayments()
    Dim f As Form: Set f = NewForm("SELECT * FROM Payments ORDER BY PaymentDate", "Payments", 2)
    Dim y As Single
    DsTxt f, "PaymentDate", "Date", y, 1
    DsTxt(f, "Amount", "Amount", y, 1).Format = "Currency"
    DsCbo f, "PaymentMethod", "Method", y, VL_PAYMETHOD, "1.1", 1
    DsTxt f, "PaymentRef", "Reference", y, 1.3
    DsCbo f, "ReceivedByRepID", "Received By", y, RS_REPS, "0;1.5", 1.2
    DsTxt f, "Notes", "Notes", y, 1.5
    f.AfterUpdate = Fn("RecalcParent")
    f.AfterDelConfirm = Fn("RecalcParent")
    SaveForm f, "sfrmPayments"
End Sub

Private Sub BuildSfrmOpenMessages()
    Dim f As Form: Set f = NewForm("qryOpenPhoneMessages", "Open Phone Messages", 2)
    Dim y As Single
    ReadOnly f
    DsTxt(f, "MessageID", "MessageID", y).ColumnHidden = True
    DsChk f, "Urgent", "Urgent", y
    DsTxt(f, "ReceivedAt", "Received", y, 1.6).Format = "m/d/yy h:nn AM/PM"
    DsTxt f, "ForRep", "For", y, 1.1
    DsTxt f, "CallerName", "Caller", y, 1.2
    DsTxt f, "Company", "Company", y, 1.6
    DsTxt f, "CallerPhone", "Phone", y, 1
    DsTxt f, "Message", "Message", y, 3
    DsTxt f, "MessageStatus", "Status", y, 0.8
    DblClickOpens f, "frmPhoneMessages", "MessageID"
    SaveForm f, "sfrmOpenMessages"
End Sub

Private Sub BuildSfrmFollowUps()
    Dim f As Form: Set f = NewForm("qryFollowUpsDue", "Follow-ups Due", 2)
    Dim y As Single
    ReadOnly f
    DsTxt(f, "ActivityID", "ActivityID", y).ColumnHidden = True
    DsTxt f, "FollowUpDate", "Due", y, 0.9
    DsTxt f, "RepName", "Rep", y, 1.1
    DsTxt f, "CustomerName", "Customer", y, 1.6
    DsTxt f, "ContactName", "Contact", y, 1.2
    DsTxt f, "ContactPhone", "Phone", y, 1
    DsTxt f, "Subject", "Subject", y, 2
    DsTxt f, "Details", "Details", y, 3
    DblClickOpens f, "frmActivities", "ActivityID"
    SaveForm f, "sfrmFollowUps"
End Sub

'------------------------------------------------------------------------------
'  SIMPLE MAINTENANCE FORMS
'------------------------------------------------------------------------------
Private Sub BuildFrmCompanyInfo()
    Dim f As Form: Set f = NewForm("CompanyInfo", "Company Info")
    f.AllowAdditions = False: f.AllowDeletions = False: f.NavigationButtons = False
    Title f, "Company Info (letterhead for quotes and receipts)"
    Const a As Single = 0.2, B As Single = 3.8
    Txt f, "CompanyName", "Company Name", a, R(0)
    Txt f, "Address1", "Address 1", a, R(1)
    Txt f, "Address2", "Address 2", a, R(2)
    Txt f, "City", "City", a, R(3)
    Txt f, "State", "State", a, R(4)
    Txt f, "Zip", "Zip", a, R(5)
    Txt f, "Country", "Country", a, R(6)
    Txt f, "Phone", "Phone", B, R(0)
    Txt f, "Email", "Email", B, R(1)
    Txt f, "Website", "Website", B, R(2)
    Txt(f, "DefaultTaxRate", "Default Tax Rate", B, R(3), 1).Format = "Percent"
    Txt f, "QuoteTerms", "Quote Terms", B, R(4), 2.2, 0.85
    Txt f, "ReceiptFooter", "Receipt Footer", B, R(7), 2.2, 0.55
    Btn f, "cmdClose", "Close", B + 2.25, R(9), 1.1, Fn("CloseMe")
    SaveForm f, "frmCompanyInfo"
End Sub

Private Sub BuildFrmSalesReps()
    Dim f As Form: Set f = NewForm("SELECT * FROM SalesReps ORDER BY FirstName, LastName", "Sales Reps")
    Title f, "Sales Reps"
    FindBox f, "RepID", RS_REPS, "0;2"
    Const a As Single = 0.2, B As Single = 3.8
    Txt f, "FirstName", "First Name", a, R(0)
    Txt f, "LastName", "Last Name", a, R(1)
    Txt f, "Title", "Title", a, R(2)
    Txt f, "HireDate", "Hire Date", a, R(3), 1.2
    Chk f, "Active", "Active", a, R(4)
    Txt f, "Email", "Email", B, R(0)
    Txt f, "Phone", "Phone", B, R(1)
    Txt f, "Extension", "Extension", B, R(2), 0.8
    Txt(f, "CommissionRate", "Commission", B, R(3), 1).Format = "Percent"
    Btn f, "cmdClose", "Close", B + 2.25, R(5), 1.1, Fn("CloseMe")
    SaveForm f, "frmSalesReps"
End Sub

Private Sub BuildFrmProducts()
    Dim f As Form: Set f = NewForm("SELECT * FROM Products ORDER BY SKU", "Products")
    Title f, "Products"
    FindBox f, "ProductID", "SELECT ProductID, SKU, ProductName FROM Products ORDER BY SKU", "0;1.1;3"
    Const a As Single = 0.2, B As Single = 3.8
    Txt f, "SKU", "SKU", a, R(0), 1.4
    Txt f, "ProductName", "Product Name", a, R(1)
    Cbo f, "Category", "Category", a, R(2), VL_CATEGORY, "1.4", 1.4
    Txt(f, "UnitPrice", "Unit Price", a, R(3), 1.2).Format = "Currency"
    Txt(f, "UnitCost", "Unit Cost", a, R(4), 1.2).Format = "Currency"
    Txt f, "LeadTimeDays", "Lead Time (days)", a, R(5), 0.8
    Txt f, "FirmwareVersion", "Firmware", a, R(6), 1.2
    Chk f, "Active", "Active", a, R(7)
    Txt f, "Description", "Description", B, R(0), 2.2, 1.45
    Btn f, "cmdClose", "Close", B + 2.25, R(8), 1.1, Fn("CloseMe")
    SaveForm f, "frmProducts"
End Sub

Private Sub BuildFrmActivities()
    Dim f As Form: Set f = NewForm("SELECT * FROM Activities ORDER BY ActivityDate DESC", "Activities")
    Title f, "Activity (call / meeting / note)"
    Const a As Single = 0.2, B As Single = 3.8
    Dim c As Control
    Set c = Cbo(f, "CustomerID", "Customer", a, R(0), RS_CUSTOMERS, "0;2.5")
    c.AfterUpdate = Fn("ActivityCustomerChanged")
    Cbo f, "ContactID", "Contact", a, R(1), _
        "SELECT ContactID, FirstName & ' ' & LastName AS ContactName FROM Contacts " & _
        "WHERE CustomerID=[Forms]![frmActivities]![CustomerID] ORDER BY IsPrimary, FirstName", "0;2"
    Cbo f, "RepID", "Rep", a, R(2), RS_REPS, "0;2"
    Txt f, "ActivityDate", "Date / Time", a, R(3), 1.6
    Cbo f, "ActivityType", "Type", a, R(4), VL_ACTTYPE, "1.2", 1.2
    Txt f, "FollowUpDate", "Follow-up Date", a, R(5), 1.2
    Chk f, "Completed", "Completed", a, R(6)
    Txt f, "Subject", "Subject", B, R(0)
    Txt f, "Details", "Details", B, R(1), 2.2, 1.45
    Btn f, "cmdClose", "Close", B + 2.25, R(7), 1.1, Fn("CloseMe")
    f.OnCurrent = Fn("RequeryControls", "ContactID")
    SaveForm f, "frmActivities"
End Sub

Private Sub BuildFrmPhoneMessages()
    Dim f As Form: Set f = NewForm("SELECT * FROM PhoneMessages ORDER BY ReceivedAt DESC", "Phone Message")
    Title f, "While You Were Out"
    Const a As Single = 0.2, B As Single = 3.8
    Dim c As Control
    Cbo f, "ForRepID", "For", a, R(0), RS_REPS, "0;2"
    Txt f, "ReceivedAt", "Received", a, R(1), 1.6
    Txt f, "CallerName", "Caller Name", a, R(2)
    Set c = Cbo(f, "CustomerID", "Customer", a, R(3), RS_CUSTOMERS, "0;2.5")
    c.AfterUpdate = Fn("MessageCustomerChanged")
    Txt f, "CallerCompany", "Company", a, R(4)
    Txt f, "CallerPhone", "Phone", a, R(5), 1.4
    Cbo f, "TakenByRepID", "Taken By", a, R(6), RS_REPS, "0;2"
    Chk f, "Urgent", "Urgent", a, R(7)
    Chk f, "PleaseCallBack", "Please call back", a + 1.6, R(7)
    Chk f, "WillCallAgain", "Will call again", a, R(8)
    Txt f, "Message", "Message", B, R(0), 2.2, 1.15
    Cbo f, "MessageStatus", "Status", B, R(4), VL_MSGSTATUS, "1.2", 1.2
    Txt f, "ReturnedAt", "Returned At", B, R(5), 1.6
    Txt f, "ReturnNotes", "Return Notes", B, R(6), 2.2, 0.55
    Btn f, "cmdReturned", "Mark Returned", B + 1.15, R(8), 1.3, Fn("MessageReturned")
    Btn f, "cmdClose", "Close", B + 2.55, R(8), 0.9, Fn("CloseMe")
    SaveForm f, "frmPhoneMessages"
End Sub

'------------------------------------------------------------------------------
'  CUSTOMERS / QUOTES / ORDERS
'------------------------------------------------------------------------------
Private Sub BuildFrmCustomers()
    Dim f As Form: Set f = NewForm("SELECT * FROM Customers ORDER BY CompanyName", "Customers")
    Title f, "Customers"
    FindBox f, "CustomerID", RS_CUSTOMERS, "0;2.5", 6.4
    Const a As Single = 0.2, B As Single = 3.5, c As Single = 6.8
    Const lw As Single = 1.1, w As Single = 2
    Txt f, "CompanyName", "Company", a, R(0), w, , lw
    Txt f, "Industry", "Industry", a, R(1), w, , lw
    Cbo f, "CustomerStatus", "Status", a, R(2), VL_CUSTSTATUS, "1.2", 1.2, lw
    Cbo f, "AssignedRepID", "Sales Rep", a, R(3), RS_REPS, "0;2", w, lw
    Cbo f, "PaymentTerms", "Terms", a, R(4), VL_TERMS, "1.2", 1.2, lw, False
    Txt f, "CustomerSince", "Customer Since", a, R(5), 1.2, , lw
    Chk f, "TaxExempt", "Tax Exempt", a, R(6), lw
    Txt f, "Phone", "Phone", B, R(0), w, , lw
    Txt f, "Email", "Email", B, R(1), w, , lw
    Txt f, "Website", "Website", B, R(2), w, , lw
    Txt f, "BillAddress", "Bill Address", B, R(3), w, , lw
    Txt f, "BillCity", "Bill City", B, R(4), w, , lw
    Txt f, "BillState", "Bill State / Zip", B, R(5), 0.6, , lw
    Txt f, "BillZip", "", B + lw + 0.7, R(5), 1.3, , 0
    Txt f, "BillCountry", "Bill Country", B, R(6), w, , lw
    Txt f, "ShipAddress", "Ship Address", c, R(0), w, , lw
    Txt f, "ShipCity", "Ship City", c, R(1), w, , lw
    Txt f, "ShipState", "Ship State / Zip", c, R(2), 0.6, , lw
    Txt f, "ShipZip", "", c + lw + 0.7, R(2), 1.3, , 0
    Txt f, "ShipCountry", "Ship Country", c, R(3), w, , lw
    Txt f, "Notes", "Notes", c, R(4), w, 0.85, lw

    Btn f, "cmdNewQuote", "New Quote", a, R(7.3), 1.3, Fn("NewDocForCustomer", "frmQuotes")
    Btn f, "cmdNewOrder", "New Order", a + 1.4, R(7.3), 1.3, Fn("NewDocForCustomer", "frmOrders")
    Btn f, "cmdNewActivity", "Log Activity", a + 2.8, R(7.3), 1.3, Fn("NewDocForCustomer", "frmActivities")
    Btn f, "cmdClose", "Close", c + lw + w - 1.1, R(7.3), 1.1, Fn("CloseMe")

    Dim tb As Control, pg As Object, i As Integer
    Set tb = CreateControl(f.Name, acTabCtl, acDetail, , , I2T(a), I2T(R(8.6)), I2T(c + lw + w - a), I2T(2.9))
    tb.Name = "tabDetails"
    Do While tb.pages.Count < 4: tb.pages.Add: Loop
    Dim pages: pages = Array("Contacts", "Activities", "Quotes", "Orders")
    Dim subs: subs = Array("sfrmContacts", "sfrmCustomerActivities", "sfrmCustomerQuotes", "sfrmCustomerOrders")
    For i = 0 To 3
        Set pg = tb.pages(i)
        pg.Name = "pg" & pages(i): pg.Caption = pages(i)
        SubF f, subs(i), "CustomerID", 0, 0, 0, 0, pg
    Next
    SaveForm f, "frmCustomers"
End Sub

Private Sub BuildFrmQuotes()
    Dim f As Form: Set f = NewForm("SELECT * FROM Quotes ORDER BY QuoteNumber DESC", "Quotes")
    Title f, "Quotes"
    FindBox f, "QuoteID", "SELECT QuoteID, QuoteNumber FROM Quotes ORDER BY QuoteNumber DESC", "0;1.5", 6.4
    Const a As Single = 0.2, B As Single = 3.5, c As Single = 6.8
    Const lw As Single = 1.1, w As Single = 2
    Dim cb As Control
    Txt f, "QuoteNumber", "Quote #", a, R(0), 1.4, , lw
    Txt f, "QuoteDate", "Quote Date", a, R(1), 1.2, , lw
    Txt(f, "ValidUntil", "Valid Until", a, R(2), 1.2, , lw).DefaultValue = "=Date()+30"
    Cbo f, "QuoteStatus", "Status", a, R(3), VL_QUOTESTATUS, "1.2", 1.2, lw
    Cbo f, "RepID", "Sales Rep", a, R(4), RS_REPS, "0;2", w, lw
    Set cb = Cbo(f, "CustomerID", "Customer", B, R(0), RS_CUSTOMERS, "0;2.5", w, lw)
    cb.AfterUpdate = Fn("DocCustomerChanged")
    Cbo f, "ContactID", "Contact", B, R(1), _
        "SELECT ContactID, FirstName & ' ' & LastName AS ContactName FROM Contacts " & _
        "WHERE CustomerID=[Forms]![frmQuotes]![CustomerID] ORDER BY IsPrimary, FirstName", "0;2", w, lw
    Txt(f, "TaxRate", "Tax Rate", B, R(2), 1, , lw).Format = "Percent"
    Txt(f, "ShippingCost", "Shipping", B, R(3), 1, , lw).Format = "Currency"
    Txt f, "Notes", "Notes", c, R(0), w, 0.55, lw
    Txt(f, "Terms", "Terms", c, R(2), w, 0.85, lw).ControlTipText = "Leave blank to use the standard terms from Company Info"

    SubF f, "sfrmQuoteLines", "QuoteID", a, R(5.4), c + lw + w - a, 2.5
    Const TY As Single = 5.4 + 2.5 / 0.3 + 0.4
    TotalBox f, "Subtotal", "Subtotal", "qryQuoteTotals", "QuoteID", c, R(TY)
    TotalBox f, "TaxAmount", "Tax", "qryQuoteTotals", "QuoteID", c, R(TY + 1)
    TotalBox f, "ShippingCost", "Shipping", "qryQuoteTotals", "QuoteID", c, R(TY + 2), "txtShipping"
    TotalBox(f, "QuoteTotal", "Quote Total", "qryQuoteTotals", "QuoteID", c, R(TY + 3)).FontBold = True

    Btn f, "cmdToOrder", "Create Order from Quote", a, R(TY), 2.2, Fn("QuoteToOrder")
    Btn f, "cmdPrint", "Print Quote", a + 2.3, R(TY), 1.4, Fn("PrintQuote")
    Btn f, "cmdClose", "Close", a, R(TY + 1.3), 1.1, Fn("CloseMe")

    f.OnLoad = Fn("DocLoad")
    f.OnCurrent = Fn("RequeryControls", "ContactID")
    f.BeforeInsert = Fn("DocBeforeInsert", "Q")
    SaveForm f, "frmQuotes"
End Sub

Private Sub BuildFrmOrders()
    Dim f As Form: Set f = NewForm("SELECT * FROM Orders ORDER BY OrderNumber DESC", "Orders")
    Title f, "Sales Orders"
    FindBox f, "OrderID", "SELECT OrderID, OrderNumber FROM Orders ORDER BY OrderNumber DESC", "0;1.5", 6.4
    Const a As Single = 0.2, B As Single = 3.5, c As Single = 6.8
    Const lw As Single = 1.1, w As Single = 2
    Dim cb As Control
    Txt f, "OrderNumber", "Order #", a, R(0), 1.4, , lw
    Txt f, "OrderDate", "Order Date", a, R(1), 1.2, , lw
    Txt f, "RequiredDate", "Required", a, R(2), 1.2, , lw
    Txt f, "ShipDate", "Ship Date", a, R(3), 1.2, , lw
    Cbo f, "OrderStatus", "Status", a, R(4), VL_ORDERSTATUS, "1.3", 1.4, lw
    Cbo f, "RepID", "Sales Rep", a, R(5), RS_REPS, "0;2", w, lw
    Set cb = Cbo(f, "CustomerID", "Customer", B, R(0), RS_CUSTOMERS, "0;2.5", w, lw)
    cb.AfterUpdate = Fn("DocCustomerChanged")
    Cbo f, "ContactID", "Contact", B, R(1), _
        "SELECT ContactID, FirstName & ' ' & LastName AS ContactName FROM Contacts " & _
        "WHERE CustomerID=[Forms]![frmOrders]![CustomerID] ORDER BY IsPrimary, FirstName", "0;2", w, lw
    Cbo f, "QuoteID", "From Quote", B, R(2), _
        "SELECT QuoteID, QuoteNumber FROM Quotes WHERE CustomerID=[Forms]![frmOrders]![CustomerID] ORDER BY QuoteNumber DESC", "0;1.4", 1.4, lw
    Txt f, "CustomerPO", "Customer PO", B, R(3), w, , lw
    Txt(f, "TaxRate", "Tax Rate", B, R(4), 1, , lw).Format = "Percent"
    Txt(f, "ShippingCost", "Shipping", B, R(5), 1, , lw).Format = "Currency"
    Txt f, "ShipVia", "Ship Via", c, R(0), w, , lw
    Txt f, "TrackingNumber", "Tracking #", c, R(1), w, , lw
    Txt f, "Notes", "Notes", c, R(2), w, 0.85, lw

    SubF f, "sfrmOrderLines", "OrderID", a, R(6.4), c + lw + w - a, 2.2
    Const PY As Single = 6.4 + 2.2 / 0.3 + 0.4
    Lbl f, "lblPayments", "Payments", a, R(PY), 2, True
    Btn f, "cmdReceipt", "Print Receipt", c - 0.3 - 1.5, R(PY) - 0.08, 1.5, Fn("PrintReceipt")
    SubF f, "sfrmPayments", "OrderID", a, R(PY + 0.9), c - a - 0.3, 1.6
    TotalBox f, "Subtotal", "Subtotal", "qryOrderBalances", "OrderID", c, R(PY)
    TotalBox f, "TaxAmount", "Tax", "qryOrderBalances", "OrderID", c, R(PY + 1)
    TotalBox f, "ShippingCost", "Shipping", "qryOrderBalances", "OrderID", c, R(PY + 2), "txtShipping"
    TotalBox(f, "OrderTotal", "Order Total", "qryOrderBalances", "OrderID", c, R(PY + 3)).FontBold = True
    TotalBox f, "AmountPaid", "Paid", "qryOrderBalances", "OrderID", c, R(PY + 4)
    TotalBox(f, "BalanceDue", "Balance Due", "qryOrderBalances", "OrderID", c, R(PY + 5)).FontBold = True
    Btn f, "cmdInvoice", "Print Invoice", c + lw + w - 2.55, R(PY + 6.4), 1.35, Fn("PrintInvoice")
    Btn f, "cmdClose", "Close", c + lw + w - 1.1, R(PY + 6.4), 1.1, Fn("CloseMe")
    If TableExists("ManufacturerOrders") Then
        Btn f, "cmdSendMfr", "Send to Manufacturer", c - 0.3 - 1.5 - 1.95, R(PY) - 0.08, 1.85, Fn("SendManufacturerOrder")
        Set cb = Txt(f, "=MfrRefs([OrderID])", "Mfr Order", c, R(5), w, , lw, "txtMfrRef")
        cb.Locked = True: cb.TabStop = False
    End If

    f.OnLoad = Fn("DocLoad")
    f.OnCurrent = Fn("RequeryControls", "ContactID;QuoteID")
    f.BeforeInsert = Fn("DocBeforeInsert", "SO")
    SaveForm f, "frmOrders"
End Sub

'------------------------------------------------------------------------------
'  SIGN-IN, USERS, RMAs
'------------------------------------------------------------------------------
Private Sub BuildFrmLogin()
    Dim f As Form: Set f = NewForm("", "Sign In")
    f.NavigationButtons = False
    Title f, "Cairn Systems - Sign In"
    Const a As Single = 0.2
    Txt f, "", "User name", a, R(1), 2, , 1.2, "txtUser"
    Txt(f, "", "Password", a, R(2), 2, , 1.2, "txtPassword").InputMask = "Password"
    Btn(f, "cmdSignIn", "Sign In", a + 1.2, R(3.4), 1.2, Fn("LoginCheck")).Default = True
    Btn f, "cmdQuit", "Exit", a + 2.5, R(3.4), 0.7, Fn("QuitApp")
    Lbl(f, "lblHelp", "Forgot your password? Ask Priya or call the front desk.", a, R(5), 4).ForeColor = RGB(128, 128, 128)
    SaveForm f, "frmLogin"
End Sub

Private Sub BuildFrmUsers()
    Dim f As Form: Set f = NewForm("SELECT * FROM Users ORDER BY UserName", "Users", 2)
    Dim y As Single
    DsTxt f, "UserName", "User Name", y, 1.2
    DsTxt f, "Password", "Password", y, 1.2
    DsCbo f, "RepID", "Sales Rep", y, RS_REPS, "0;1.5", 1.5
    DsChk f, "IsAdmin", "Admin", y
    DsChk f, "Active", "Active", y
    DsTxt f, "CreatedDate", "Created", y, 1
    DsTxt(f, "LastLogin", "Last Sign-in", y, 2.1).Format = "General Date"
    SaveForm f, "frmUsers"
End Sub

Private Sub BuildFrmRMAs()
    Dim f As Form: Set f = NewForm("SELECT * FROM RMAs ORDER BY RMANumber DESC", "RMAs")
    Title f, "Returns (RMA)"
    FindBox f, "RMAID", "SELECT RMAID, RMANumber FROM RMAs ORDER BY RMANumber DESC", "0;1.5", 6.4
    Const a As Single = 0.2, B As Single = 3.5, c As Single = 6.8
    Const lw As Single = 1.1, w As Single = 2
    Dim ctl As Control
    Txt f, "RMANumber", "RMA #", a, R(0), 1.4, , lw
    Txt f, "OpenedDate", "Opened", a, R(1), 1.2, , lw
    Cbo f, "RMAStatus", "Status", a, R(2), VL_RMASTATUS, "1.2", 1.2, lw
    Cbo f, "OpenedByRepID", "Opened By", a, R(3), RS_REPS, "0;2", w, lw
    Txt f, "ClosedDate", "Closed", a, R(4), 1.2, , lw
    Set ctl = Cbo(f, "CustomerID", "Customer", B, R(0), RS_CUSTOMERS, "0;2.5", w, lw)
    ctl.AfterUpdate = Fn("RequeryControls", "OrderID")
    Cbo f, "OrderID", "Order", B, R(1), _
        "SELECT OrderID, OrderNumber, OrderDate FROM Orders WHERE CustomerID=[Forms]![frmRMAs]![CustomerID] ORDER BY OrderDate DESC", "0;1.3;1", w, lw
    ' all products, not just current ones: returns come back on discontinued models
    Cbo f, "ProductID", "Product", B, R(2), "SELECT ProductID, SKU, ProductName FROM Products ORDER BY SKU", "0;1.1;3", w, lw
    Txt f, "SerialNumber", "Serial #", B, R(3), w, , lw
    Cbo f, "Reason", "Reason", B, R(4), VL_RMAREASON, "2", w, lw
    Chk f, "ExtendedWarranty", "Ext. Warranty", B, R(5), lw
    Txt f, "Disposition", "Disposition", c, R(0), w, , lw
    Txt f, "ManufacturerRef", "Mfr RMA #", c, R(1), w, , lw
    Txt f, "Description", "Description", c, R(2), w, 0.85, lw
    ' What OrderID actually holds; the combo above goes blank when that order isn't in the file
    Set ctl = Txt(f, "=IIf(IsNull([OrderID]),""(none)"",Nz(DLookUp(""OrderNumber"",""Orders"",""OrderID="" & [OrderID]),""order "" & [OrderID] & "" - not found""))", _
                  "Order on file", c, R(5), w, , lw, "txtOrderOnFile")
    ctl.Locked = True: ctl.TabStop = False
    Set ctl = Txt(f, "=DLookUp(""Route"",""qryRMARouting"",""RMAID="" & Nz([RMAID],0))", "Route", a, R(6.3), 2.1, , lw, "txtRoute")
    ctl.Locked = True: ctl.TabStop = False: ctl.FontBold = True: ctl.ForeColor = RGB(31, 78, 121)
    Set ctl = Txt(f, "=DLookUp(""DaysInService"",""qryRMARouting"",""RMAID="" & Nz([RMAID],0))", "Days in service", B, R(6.3), 0.8, , lw, "txtDays")
    ctl.Locked = True: ctl.TabStop = False
    Btn f, "cmdClose", "Close", c + lw + w - 1.1, R(6.2), 1.1, Fn("CloseMe")
    f.OnCurrent = Fn("RequeryControls", "OrderID")
    f.BeforeInsert = Fn("RMADefaults")
    SaveForm f, "frmRMAs"
End Sub

'------------------------------------------------------------------------------
'  MAIN MENU
'------------------------------------------------------------------------------
Private Sub BuildFrmMainMenu(Optional ByVal ops As Boolean)
    Dim f As Form: Set f = NewForm("", "Cairn Systems")
    f.NavigationButtons = False
    Title f, "Cairn Systems - Orders && CRM"
    Const a As Single = 0.2, B As Single = 2.4, SX As Single = 4.7, BW As Single = 2
    Dim i As Integer, items, y As Single

    Lbl f, "lblRecords", "Records", a, R(0), BW, True
    items = Array("Customers", "frmCustomers", "Quotes", "frmQuotes", "Sales Orders", "frmOrders", _
                  "Products", "frmProducts", "Sales Reps", "frmSalesReps", "Company Info", "frmCompanyInfo")
    If ops Then items = Split(Join(items, "|") & "|Returns (RMAs)|frmRMAs|Users|frmUsers", "|")
    y = 1
    For i = 0 To UBound(items) Step 2
        Btn f, "cmd" & items(i + 1), CStr(items(i)), a, R(y), BW, Fn(IIf(items(i + 1) = "frmUsers", "OpenDatasheet", "OpenFrm"), items(i + 1))
        y = y + 1.3
    Next
    Btn f, "cmdTakeMessage", "Take Phone Message", a, R(y + 0.4), BW, Fn("OpenNew", "frmPhoneMessages")
    Btn f, "cmdLogActivity", "Log Activity", a, R(y + 1.7), BW, Fn("OpenNew", "frmActivities")

    Lbl f, "lblMonthEnd", "Month-end (asks for month)", B, R(0), BW + 0.3, True
    items = Array("Orders", "qryMonthEnd_Orders", "Sales by Rep", "qryMonthEnd_SalesByRep", _
                  "Sales by Product", "qryMonthEnd_SalesByProduct", "Cash Received", "qryMonthEnd_CashReceived", _
                  "Quote Pipeline", "qryMonthEnd_QuotePipeline", "Open Receivables", "qryOpenReceivables")
    y = 1
    For i = 0 To UBound(items) Step 2
        Btn f, "cmd" & items(i + 1), CStr(items(i)), B, R(y), BW, Fn("OpenQry", items(i + 1))
        y = y + 1.3
    Next

    Btn f, "cmdCommission", "Commission Statements", B, R(y + 0.4), BW, Fn("PrintCommission")
    If ops Then
        Btn f, "cmdLeads", "Qualified Leads", B, R(y + 1.7), BW, Fn("OpenQry", "qryQualifiedLeads")
        Btn f, "cmdRMASummary", "RMA Summary", B, R(y + 3), BW, Fn("PrintRMASummary")
        Dim c As Control
        Set c = Txt(f, "=""Signed in as "" & [TempVars]![UserName]", "", SX + 2.6, 0.17, 2.4, , 0, "txtSignedIn")
        c.Locked = True: c.TabStop = False: c.BorderStyle = 0: c.BackStyle = 0: c.TextAlign = 3
        Btn f, "cmdSignOut", "Sign Out", SX + 5.1, 0.13, 1.1, Fn("SignOut")
        f.OnLoad = Fn("MenuLoad")
        f.OnTimer = Fn("MenuTimer")
    End If

    Lbl f, "lblMessages", "Open phone messages (double-click to open)", SX, R(0), 5, True
    SubF f, "sfrmOpenMessages", "", SX, R(0.9), 6.2, 2.1, , "subMessages"
    Lbl f, "lblFollowUps", "Follow-ups due in the next 7 days (double-click to open)", SX, R(8.5), 5, True
    SubF f, "sfrmFollowUps", "", SX, R(9.4), 6.2, 2.1, , "subFollowUps"
    f.OnActivate = Fn("RequeryControls", "subMessages;subFollowUps")
    SaveForm f, "frmMainMenu"
End Sub

'==============================================================================
'  RUNTIME FUNCTIONS (called from form and button events - keep Public)
'==============================================================================
Public Function OpenFrm(frm As Form, ByVal formName As String)
    DoCmd.OpenForm formName
End Function

Public Function OpenDatasheet(frm As Form, ByVal formName As String)
    DoCmd.OpenForm formName, acFormDS
End Function

Public Function OpenNew(frm As Form, ByVal formName As String)
    DoCmd.OpenForm formName, DataMode:=acFormAdd
End Function

Public Function OpenQry(frm As Form, ByVal queryName As String)
    On Error Resume Next          ' Cancel on the month prompt raises error 2001
    DoCmd.OpenQuery queryName
End Function

Public Function CloseMe(frm As Form)
    If frm.Dirty Then frm.Dirty = False
    DoCmd.Close acForm, frm.Name
End Function

' Double-click in a list: open the record in its own form
Public Function OpenRec(frm As Form, ByVal formName As String, ByVal keyField As String)
    If IsNull(frm(keyField)) Then Exit Function
    DoCmd.OpenForm formName, WhereCondition:=keyField & "=" & frm(keyField)
End Function

' "Go to" combo at the top of a form
Public Function FindRec(frm As Form, ByVal keyField As String)
    Dim v: v = frm!cboFind
    If IsNull(v) Then Exit Function
    If frm.Dirty Then frm.Dirty = False
    If frm.DataEntry Then frm.DataEntry = False
    If frm.FilterOn Then frm.FilterOn = False
    frm.Recordset.FindFirst keyField & "=" & v
    frm!cboFind = Null
End Function

' Requery one or more controls (names separated by ;)
Public Function RequeryControls(frm As Form, ByVal names As String)
    Dim n
    For Each n In Split(names, ";")
        frm.Controls(n).Requery
    Next
End Function

Public Function RecalcParent(frm As Form)
    On Error Resume Next          ' no parent when the subform is opened on its own
    frm.Parent.Recalc
End Function

' Next number like Q-2026-0105, SO-2026-0205 or RMA-2026-0012
Public Function NextNumber(ByVal prefix As String) As String
    Dim tbl As String, fld As String, stem As String, n As Variant
    Select Case prefix
    Case "Q": tbl = "Quotes": fld = "QuoteNumber"
    Case "RMA": tbl = "RMAs": fld = "RMANumber"
    Case Else: tbl = "Orders": fld = "OrderNumber"
    End Select
    stem = prefix & "-" & Year(Date) & "-"
    n = DMax("Val(Mid(" & fld & "," & Len(stem) + 1 & "))", tbl, fld & " Like '" & stem & "*'")
    NextNumber = stem & Format(Nz(n, 0) + 1, "0000")
End Function

' From frmCustomers: open a new quote / order / activity for this customer
Public Function NewDocForCustomer(frm As Form, ByVal formName As String)
    If frm.Dirty Then frm.Dirty = False
    If IsNull(frm!CustomerID) Then
        MsgBox "Save the customer first.", vbExclamation, "Cairn Systems"
        Exit Function
    End If
    DoCmd.OpenForm formName, DataMode:=acFormAdd, OpenArgs:=CStr(frm!CustomerID)
End Function

' Quote / order opened from a customer: preset the customer
Public Function DocLoad(frm As Form)
    If IsNull(frm.OpenArgs) Then Exit Function
    frm!CustomerID = CLng(frm.OpenArgs)
    DocCustomerChanged frm
End Function

Public Function DocBeforeInsert(frm As Form, ByVal prefix As String)
    Dim fld As String: fld = IIf(prefix = "Q", "QuoteNumber", "OrderNumber")
    If IsNull(frm(fld)) Then frm(fld) = NextNumber(prefix)
End Function

' Customer picked on a quote/order: tax rate, rep and primary contact
Public Function DocCustomerChanged(frm As Form)
    Dim cid: cid = frm!CustomerID
    If IsNull(cid) Then Exit Function
    If Nz(DLookup("TaxExempt", "Customers", "CustomerID=" & cid), False) Then
        frm!taxRate = 0
    Else
        frm!taxRate = Nz(DLookup("DefaultTaxRate", "CompanyInfo"), 0)
    End If
    If IsNull(frm!repID) Then frm!repID = DLookup("AssignedRepID", "Customers", "CustomerID=" & cid)
    frm.Controls("ContactID").Requery
    frm!contactID = DLookup("ContactID", "Contacts", "CustomerID=" & cid & " AND IsPrimary=True")
    On Error Resume Next          ' only frmOrders has a QuoteID combo
    frm.Controls("QuoteID").Requery
End Function

' frmActivities: customer picked (or preset from frmCustomers)
Public Function ActivityCustomerChanged(frm As Form)
    Dim cid: cid = frm!CustomerID
    frm.Controls("ContactID").Requery
    If IsNull(cid) Then Exit Function
    If IsNull(frm!repID) Then frm!repID = DLookup("AssignedRepID", "Customers", "CustomerID=" & cid)
    If IsNull(frm!contactID) Then frm!contactID = DLookup("ContactID", "Contacts", "CustomerID=" & cid & " AND IsPrimary=True")
End Function

' Activities tab on frmCustomers: default the rep to the customer's rep
Public Function ActivityDefaults(frm As Form)
    On Error Resume Next
    If IsNull(frm!repID) Then frm!repID = frm.Parent!AssignedRepID
End Function

Public Function LineProductChanged(frm As Form)
    If IsNull(frm!productID) Then Exit Function
    frm!UnitPrice = DLookup("UnitPrice", "Products", "ProductID=" & frm!productID)
End Function

Public Function MessageCustomerChanged(frm As Form)
    Dim cid: cid = frm!CustomerID
    If IsNull(cid) Then Exit Function
    frm!CallerCompany = DLookup("CompanyName", "Customers", "CustomerID=" & cid)
    If IsNull(frm!CallerPhone) Then frm!CallerPhone = DLookup("Phone", "Customers", "CustomerID=" & cid)
End Function

Public Function MessageReturned(frm As Form)
    frm!MessageStatus = "Returned"
    If IsNull(frm!ReturnedAt) Then frm!ReturnedAt = Now
    frm.Dirty = False
End Function

' Copy a quote and its lines into a new sales order
Public Function QuoteToOrder(frm As Form)
    If frm.Dirty Then frm.Dirty = False
    If IsNull(frm!QuoteID) Then Exit Function
    Dim qid As Long, oid As Variant, num As String, db As DAO.Database
    qid = frm!QuoteID
    oid = DLookup("OrderID", "Orders", "QuoteID=" & qid)
    If Not IsNull(oid) Then
        If MsgBox("Order " & DLookup("OrderNumber", "Orders", "OrderID=" & oid) & _
                  " was already created from this quote. Open it?", vbYesNo + vbQuestion, "Cairn Systems") = vbYes Then
            DoCmd.OpenForm "frmOrders", WhereCondition:="OrderID=" & oid
        End If
        Exit Function
    End If
    If DCount("*", "QuoteLines", "QuoteID=" & qid) = 0 Then
        MsgBox "This quote has no line items.", vbExclamation, "Cairn Systems"
        Exit Function
    End If
    If MsgBox("Create a sales order from quote " & frm!QuoteNumber & "?", vbYesNo + vbQuestion, "Cairn Systems") <> vbYes Then Exit Function

    Set db = CurrentDb
    num = NextNumber("SO")
    db.Execute "INSERT INTO Orders (OrderNumber, CustomerID, ContactID, RepID, QuoteID, OrderDate, OrderStatus, TaxRate, ShippingCost) " & _
               "SELECT '" & num & "', CustomerID, ContactID, RepID, QuoteID, Date(), 'Open', TaxRate, ShippingCost " & _
               "FROM Quotes WHERE QuoteID=" & qid, dbFailOnError
    oid = DLookup("OrderID", "Orders", "OrderNumber='" & num & "'")
    db.Execute "INSERT INTO OrderLines (OrderID, ProductID, LineDescription, Quantity, UnitPrice, DiscountPct) " & _
               "SELECT " & oid & ", ProductID, LineDescription, Quantity, UnitPrice, DiscountPct " & _
               "FROM QuoteLines WHERE QuoteID=" & qid & " ORDER BY QuoteLineID", dbFailOnError
    frm!QuoteStatus = "Accepted"
    frm.Dirty = False
    DoCmd.OpenForm "frmOrders", WhereCondition:="OrderID=" & oid
End Function

'------------------------------------------------------------------------------
'  Sign-in. Checks the name and password against the Users table and keeps
'  who signed in (and whether they're an admin) in TempVars for the forms.
'  It only lives on the S: drive, so nothing fancy.
'------------------------------------------------------------------------------
Public Function LoginCheck(frm As Form)
    Dim sql As String, rs As DAO.Recordset
    sql = "SELECT UserID, UserName, RepID, IsAdmin FROM Users WHERE UserName = '" & frm!txtUser & _
          "' AND [Password] = '" & frm!txtPassword & "' AND Active"
    Set rs = CurrentDb.OpenRecordset(sql, dbOpenSnapshot)
    If rs.EOF Then
        rs.Close
        MsgBox "Incorrect user name or password.", vbExclamation, "Cairn Systems"
        frm!txtPassword = Null
        Exit Function
    End If
    TempVars.Add "UserID", rs!userID.Value
    TempVars.Add "UserName", rs!UserName.Value
    TempVars.Add "RepID", rs!repID.Value
    TempVars.Add "IsAdmin", rs!IsAdmin.Value
    CurrentDb.Execute "UPDATE Users SET LastLogin = Now() WHERE UserID = " & rs!userID
    rs.Close
    DoCmd.Close acForm, frm.Name
    DoCmd.OpenForm "frmMainMenu"
End Function

Public Function SignOut(frm As Form)
    TempVars.RemoveAll
    DoCmd.Close acForm, frm.Name
    DoCmd.OpenForm "frmLogin"
End Function

Public Function QuitApp(frm As Form)
    DoCmd.Quit
End Function

' Main menu: nobody signed in yet? Send them to the sign-in form (on a timer,
' because a form can't close itself while it is still loading)
Public Function MenuLoad(frm As Form)
    If IsNull(TempVars("UserID")) Then
        frm.TimerInterval = 1
    Else
        frm!cmdfrmUsers.Visible = Nz(TempVars("IsAdmin"), False)
    End If
End Function

Public Function MenuTimer(frm As Form)
    frm.TimerInterval = 0
    DoCmd.Close acForm, frm.Name
    DoCmd.OpenForm "frmLogin"
End Function

'------------------------------------------------------------------------------
'  RMAs
'------------------------------------------------------------------------------
Public Function RMADefaults(frm As Form)
    If IsNull(frm!RMANumber) Then frm!RMANumber = NextNumber("RMA")
    If IsNull(frm!OpenedByRepID) Then frm!OpenedByRepID = TempVars("RepID")
End Function

'------------------------------------------------------------------------------
'  Manufacturer custom orders. Sends the order's hardware lines to the
'  manufacturer's order API: JSON pasted together as a string, a synchronous
'  MSXML2.XMLHTTP call that freezes Access until it returns, and a Retry that
'  sends the same order again with nothing to tell the manufacturer it is a repeat.
'------------------------------------------------------------------------------
Public Function SendManufacturerOrder(frm As Form)
    Dim url As String, body As String, lines As String, rs As DAO.Recordset, rsLog As DAO.Recordset
    Dim http As Object, status As Long, resp As String, errText As String, ref As String, cid As Long
    If frm.Dirty Then frm.Dirty = False
    If IsNull(frm!orderID) Then Exit Function
    url = Nz(DLookup("ManufacturerApiUrl", "CompanyInfo"), "")
    If Len(url) = 0 Then
        MsgBox "No manufacturer address is set up in Company Info.", vbExclamation, "Cairn Systems"
        Exit Function
    End If

    Set rs = CurrentDb.OpenRecordset("SELECT SKU, Quantity FROM qryOrderLines WHERE OrderID=" & frm!orderID & _
        " AND Category IN ('Sensor','Gateway','Module','Kit','Accessory') ORDER BY OrderLineID", dbOpenSnapshot)
    Do Until rs.EOF
        lines = lines & IIf(Len(lines), ",", "") & "{""sku"":""" & rs!sku & """,""qty"":" & rs!Quantity & "}"
        rs.MoveNext
    Loop
    rs.Close
    If Len(lines) = 0 Then
        MsgBox "This order has no hardware to order from the manufacturer.", vbInformation, "Cairn Systems"
        Exit Function
    End If
    cid = frm!CustomerID
    body = "{""dealer"":""CAIRN"",""po"":""" & frm!OrderNumber & """,""customerPo"":""" & Nz(frm!CustomerPO, "") & """," & _
           """shipTo"":{""name"":""" & DLookup("CompanyName", "Customers", "CustomerID=" & cid) & """," & _
           """address"":""" & Nz(DLookup("ShipAddress", "Customers", "CustomerID=" & cid), "") & """," & _
           """city"":""" & Nz(DLookup("ShipCity", "Customers", "CustomerID=" & cid), "") & """," & _
           """state"":""" & Nz(DLookup("ShipState", "Customers", "CustomerID=" & cid), "") & """," & _
           """zip"":""" & Nz(DLookup("ShipZip", "Customers", "CustomerID=" & cid), "") & """}," & _
           """lines"":[" & lines & "]}"

    Do
        status = 0: resp = "": errText = "": ref = ""
        Set http = CreateObject("MSXML2.XMLHTTP.6.0")
        On Error Resume Next
        http.Open "POST", url, False
        http.setRequestHeader "Content-Type", "application/json"
        http.send body
        If Err.Number <> 0 Then
            errText = Trim(Replace(Err.Description, vbCrLf, " "))
            If Len(errText) = 0 Then errText = "Error " & Err.Number
        Else
            status = http.status: resp = http.responseText
        End If
        On Error GoTo 0
        If status = 201 Then ref = JsonText(resp, "mfrOrderId")

        Set rsLog = CurrentDb.OpenRecordset("ManufacturerOrders", dbOpenDynaset, dbAppendOnly)
        rsLog.AddNew
        rsLog!orderID = frm!orderID
        rsLog!attempt = Nz(DMax("Attempt", "ManufacturerOrders", "OrderID=" & frm!orderID), 0) + 1
        rsLog!sentAt = Now
        rsLog!SentByUserID = TempVars("UserID")
        rsLog!RequestBody = body
        If status <> 0 Then rsLog!HttpStatus = status
        If Len(resp) Then rsLog!ResponseBody = resp
        If Len(errText) Then rsLog!ErrorText = Left(errText, 255)
        If Len(ref) Then rsLog!ManufacturerRef = ref
        rsLog.Update
        rsLog.Close

        If Len(ref) Then
            frm.Recalc
            MsgBox "The manufacturer accepted the order." & vbCrLf & vbCrLf & "Manufacturer order: " & ref, vbInformation, "Cairn Systems"
            Exit Function
        End If
        If Len(errText) = 0 Then errText = "The manufacturer answered " & status & ": " & Left(resp, 200)
    Loop While MsgBox("The manufacturer did not confirm the order." & vbCrLf & vbCrLf & errText & vbCrLf & vbCrLf & _
                      "Try again?", vbRetryCancel + vbExclamation, "Cairn Systems") = vbRetry
    frm.Recalc
End Function

' Manufacturer order numbers recorded for an order, e.g. "MFR-100231, MFR-100232"
Public Function MfrRefs(ByVal orderID As Variant) As Variant
    Dim rs As DAO.Recordset, s As String
    MfrRefs = Null
    If IsNull(orderID) Then Exit Function
    On Error GoTo Done                ' no ManufacturerOrders table in older databases
    Set rs = CurrentDb.OpenRecordset("SELECT ManufacturerRef FROM ManufacturerOrders WHERE OrderID=" & orderID & _
                                     " AND ManufacturerRef Is Not Null ORDER BY SentAt", dbOpenSnapshot)
    Do Until rs.EOF
        s = s & IIf(Len(s), ", ", "") & rs!ManufacturerRef
        rs.MoveNext
    Loop
    rs.Close
    If Len(s) Then MfrRefs = s
Done:
End Function

' Value of "key":"value" in a JSON reply (enough for the manufacturer's answer)
Private Function JsonText(ByVal json As String, ByVal key As String) As String
    Dim p As Long, q As Long
    p = InStr(json, """" & key & """")
    If p = 0 Then Exit Function
    p = InStr(p + Len(key) + 2, json, """")
    If p = 0 Then Exit Function
    q = InStr(p + 1, json, """")
    If q > p Then JsonText = Mid(json, p + 1, q - p - 1)
End Function

'==============================================================================
'  BUILD HELPERS
'==============================================================================
Private Function I2T(ByVal inches As Single) As Long
    I2T = CLng(inches * 1440)
End Function

' Top of layout row n (inches)
Private Function R(ByVal n As Single) As Single
    R = 0.65 + n * 0.3
End Function

' Event property that calls one of the runtime functions: =Fn([Form],"a","b")
Private Function Fn(ByVal funcName As String, ParamArray args()) As String
    Dim s As String, a
    s = "=" & funcName & "([Form]"
    For Each a In args
        s = s & ",""" & a & """"
    Next
    Fn = s & ")"
End Function

Private Function NewForm(ByVal recSrc As String, ByVal cap As String, Optional ByVal view As Integer = 0) As Form
    Dim f As Form
    Set f = CreateForm
    mTmp = f.Name
    f.RecordSource = recSrc
    f.Caption = cap
    f.DefaultView = view
    f.AutoCenter = True
    f.DividingLines = False
    If view = 0 Then
        f.RecordSelectors = False
        f.AllowDatasheetView = False
        f.ScrollBars = 0
    End If
    Set NewForm = f
End Function

Private Sub SaveForm(f As Form, ByVal formName As String)
    Dim tmp As String: tmp = f.Name
    DoCmd.Close acForm, tmp, acSaveYes
    DoCmd.Rename formName, acForm, tmp
    mTmp = ""
End Sub

Private Sub DiscardTemp()
    On Error Resume Next
    If Len(mTmp) Then
        DoCmd.Close acForm, mTmp, acSaveNo
        DoCmd.DeleteObject acForm, mTmp
    End If
    mTmp = ""
End Sub

Private Sub DropAllForms()
    Dim n
    For Each n In Split(FORM_LIST, ";")
        If FormExists(CStr(n)) Then
            DoCmd.Close acForm, n, acSaveNo
            DoCmd.DeleteObject acForm, n
        End If
    Next
End Sub

Private Function TableExists(ByVal tName As String) As Boolean
    Dim t As DAO.TableDef
    For Each t In CurrentDb.TableDefs
        If t.Name = tName Then TableExists = True: Exit Function
    Next
End Function

Private Function FormExists(ByVal formName As String) As Boolean
    Dim ao As AccessObject
    For Each ao In CurrentProject.AllForms
        If ao.Name = formName Then FormExists = True: Exit Function
    Next
End Function

Private Sub SetDbProp(ByVal propName As String, ByVal v As String)
    Dim db As DAO.Database: Set db = CurrentDb
    On Error Resume Next
    db.Properties(propName) = v
    If Err.Number <> 0 Then
        Err.Clear
        db.Properties.Append db.CreateProperty(propName, dbText, v)
    End If
End Sub

Private Sub Title(f As Form, ByVal cap As String)
    Dim c As Control
    Set c = CreateControl(f.Name, acLabel, acDetail, , , I2T(0.2), I2T(0.1), I2T(6), I2T(0.4))
    c.Name = "lblFormTitle": c.Caption = cap
    c.FontSize = 16: c.FontBold = True: c.ForeColor = RGB(31, 78, 121)
End Sub

Private Function Lbl(f As Form, ByVal nm As String, ByVal cap As String, ByVal x As Single, ByVal y As Single, _
                     ByVal w As Single, Optional ByVal heading As Boolean) As Control
    Dim c As Control
    Set c = CreateControl(f.Name, acLabel, acDetail, , , I2T(x), I2T(y), I2T(w), I2T(0.25))
    c.Name = nm: c.Caption = cap
    If heading Then c.FontBold = True: c.ForeColor = RGB(31, 78, 121)
    Set Lbl = c
End Function

' Label to the left of a control (attached, so it moves/hides with it)
Private Sub AttachLabel(f As Form, c As Control, ByVal cap As String, ByVal x As Single, ByVal y As Single, ByVal lw As Single)
    If lw <= 0 Then Exit Sub
    Dim l As Control
    Set l = CreateControl(f.Name, acLabel, acDetail, c.Name, , I2T(x), I2T(y), I2T(lw - 0.05), I2T(0.25))
    l.Name = "lbl" & c.Name
    l.Caption = cap
End Sub

' Bound text box (src = field) or calculated one (src starts with "=")
Private Function Txt(f As Form, ByVal src As String, ByVal cap As String, ByVal x As Single, ByVal y As Single, _
                     Optional ByVal w As Single = 2.2, Optional ByVal h As Single = 0.25, Optional ByVal lw As Single = 1.35, _
                     Optional ByVal nm As String) As Control
    Dim c As Control
    Set c = CreateControl(f.Name, acTextBox, acDetail, , IIf(Left(src, 1) = "=", "", src), I2T(x + lw), I2T(y), I2T(w), I2T(h))
    If Left(src, 1) = "=" Then c.ControlSource = src
    c.Name = IIf(Len(nm), nm, src)
    If h > 0.3 Then c.ScrollBars = 2: c.EnterKeyBehavior = True
    AttachLabel f, c, cap, x, y, lw
    Set Txt = c
End Function

Private Function Cbo(f As Form, ByVal src As String, ByVal cap As String, ByVal x As Single, ByVal y As Single, _
                     ByVal rowSrc As String, ByVal colWidths As String, Optional ByVal w As Single = 2.2, _
                     Optional ByVal lw As Single = 1.35, Optional ByVal limit As Boolean = True) As Control
    Dim c As Control
    Set c = CreateControl(f.Name, acComboBox, acDetail, , src, I2T(x + lw), I2T(y), I2T(w), I2T(0.25))
    If Len(src) Then c.Name = src
    SetupList c, rowSrc, colWidths, limit
    AttachLabel f, c, cap, x, y, lw
    Set Cbo = c
End Function

Private Sub SetupList(c As Control, ByVal rowSrc As String, ByVal colWidths As String, ByVal limit As Boolean)
    Dim parts, i As Integer, tw As String, total As Single
    If UCase(Left(rowSrc, 6)) = "SELECT" Then
        c.RowSourceType = "Table/Query"
    Else
        c.RowSourceType = "Value List"
    End If
    c.RowSource = rowSrc
    parts = Split(colWidths, ";")
    For i = 0 To UBound(parts)
        tw = tw & IIf(i > 0, ";", "") & I2T(CSng(parts(i)))
        total = total + CSng(parts(i))
    Next
    c.ColumnCount = UBound(parts) + 1
    c.ColumnWidths = tw
    c.BoundColumn = 1
    c.ListWidth = I2T(total + 0.25)
    c.LimitToList = limit
End Sub

Private Function Chk(f As Form, ByVal src As String, ByVal cap As String, ByVal x As Single, ByVal y As Single, _
                     Optional ByVal lw As Single = 1.35) As Control
    Dim c As Control
    Set c = CreateControl(f.Name, acCheckBox, acDetail, , src, I2T(x + lw), I2T(y + 0.04))
    c.Name = src
    AttachLabel f, c, cap, x, y, lw
    Set Chk = c
End Function

Private Function Btn(f As Form, ByVal nm As String, ByVal cap As String, ByVal x As Single, ByVal y As Single, _
                     ByVal w As Single, ByVal clickExpr As String) As Control
    Dim c As Control
    Set c = CreateControl(f.Name, acCommandButton, acDetail, , , I2T(x), I2T(y), I2T(w), I2T(0.33))
    c.Name = nm: c.Caption = cap
    c.OnClick = clickExpr
    Set Btn = c
End Function

' Read-only total pulled from a totals query for the current record
Private Function TotalBox(f As Form, ByVal fld As String, ByVal cap As String, ByVal qry As String, ByVal keyFld As String, _
                          ByVal x As Single, ByVal y As Single, Optional ByVal nm As String) As Control
    Dim c As Control
    Set c = Txt(f, "=CCur(Nz(DLookUp(""" & fld & """,""" & qry & """,""" & keyFld & "="" & Nz([" & keyFld & "],0)),0))", _
                cap, x, y, 1.3, , 1.1, IIf(Len(nm), nm, "txt" & fld))
    c.Format = "Currency": c.Locked = True: c.TabStop = False
    c.BackStyle = 0: c.TextAlign = 3
    Set TotalBox = c
End Function

Private Sub SubF(f As Form, ByVal srcForm As String, ByVal linkFld As String, ByVal x As Single, ByVal y As Single, _
                 ByVal w As Single, ByVal h As Single, Optional pg As Object, Optional ByVal nm As String)
    Dim c As Control
    If pg Is Nothing Then
        Set c = CreateControl(f.Name, acSubform, acDetail, , , I2T(x), I2T(y), I2T(w), I2T(h))
    Else
        Set c = CreateControl(f.Name, acSubform, acDetail, pg.Name, , pg.Left + 60, pg.Top + 60, pg.Width - 120, pg.Height - 120)
    End If
    c.Name = IIf(Len(nm), nm, "sub" & Mid(srcForm, 5))
    c.SourceObject = "Form." & srcForm
    If Len(linkFld) Then c.LinkMasterFields = linkFld: c.LinkChildFields = linkFld
End Sub

Private Sub FindBox(f As Form, ByVal keyFld As String, ByVal rowSrc As String, ByVal colWidths As String, _
                    Optional ByVal x As Single = 3.8)
    Dim c As Control
    Set c = CreateControl(f.Name, acComboBox, acDetail, , , I2T(x + 0.75), I2T(0.17), I2T(2.2), I2T(0.25))
    c.Name = "cboFind"
    SetupList c, rowSrc, colWidths, True
    c.AfterUpdate = Fn("FindRec", keyFld)
    AttachLabel f, c, "Go to:", x, 0.17, 0.75
End Sub

' ---- datasheet helpers (position only sets column order) ----
Private Function DsTxt(f As Form, ByVal src As String, ByVal cap As String, y As Single, _
                       Optional ByVal w As Single = 1, Optional ByVal nm As String) As Control
    Dim c As Control
    Set c = Txt(f, src, cap, 0.1, y, w, , 1.5, nm)
    y = y + 0.3
    On Error Resume Next
    c.ColumnWidth = I2T(w)
    Set DsTxt = c
End Function

Private Function DsCbo(f As Form, ByVal src As String, ByVal cap As String, y As Single, ByVal rowSrc As String, _
                       ByVal colWidths As String, Optional ByVal w As Single = 1.2) As Control
    Dim c As Control
    Set c = Cbo(f, src, cap, 0.1, y, rowSrc, colWidths, w, 1.5)
    y = y + 0.3
    On Error Resume Next
    c.ColumnWidth = I2T(w)
    Set DsCbo = c
End Function

Private Function DsChk(f As Form, ByVal src As String, ByVal cap As String, y As Single) As Control
    Set DsChk = Chk(f, src, cap, 0.1, y, 1.5)
    y = y + 0.3
End Function

Private Sub ReadOnly(f As Form)
    f.AllowEdits = False: f.AllowAdditions = False: f.AllowDeletions = False
End Sub

Private Sub DblClickOpens(f As Form, ByVal formName As String, ByVal keyFld As String)
    Dim c As Control
    For Each c In f.Controls
        If c.ControlType = acTextBox Or c.ControlType = acCheckBox Then
            c.OnDblClick = Fn("OpenRec", formName, keyFld)
        End If
    Next
End Sub

