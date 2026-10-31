Attribute VB_Name = "CairnHistory"
'==============================================================================
'  CAIRN SYSTEMS - 10-YEAR HISTORY GENERATOR  (fictional demo data)
'------------------------------------------------------------------------------
'  Adds ten years of made-up customers, contacts, quotes, orders, payments,
'  CRM activities and phone messages to a COPY of the Cairn database:
'      2016 (year 1)  about $18M net sales
'      2025 (year 10) about $35M net sales  (steady ~7.7% a year growth)
'      2026           year-to-date at the same growth rate
'  Net sales = order lines after discounts, before tax and shipping,
'  excluding cancelled orders.
'
'  Run GenerateHistory ONCE, on a copy that already has the Cairn tables
'  (BuildCairnDatabase). It cannot be undone. A fixed random seed makes the
'  result the same every time it is run on the same starting database.
'==============================================================================
Option Compare Database
Option Explicit

Private Const FIRST_YEAR As Integer = 2016
Private Const LAST_FULL_YEAR As Integer = 2025
Private Const SALES_FIRST As Double = 18000000#
Private Const SALES_LAST As Double = 35000000#
Private Const SEED As Long = 20261010
Private Const OPS_SEED As Long = 20261014          ' operations history (AddOperationsHistory)
Private Const MFR_API_START As Date = #3/4/2019#   ' first order sent through the manufacturer API

Private Type TRep
    ID As Long
    Hire As Date
    Leave As Date          ' 0 = still employed
    Weight As Double       ' share of new accounts
End Type

Private Type TProd
    ID As Long
    SKU As String
    Price As Currency      ' current list price
    Intro As Integer       ' first year sold
    Disc As Integer        ' first year NOT sold (0 = still sold)
End Type

Private Type TCust
    ID As Long
    Name As String
    Industry As Integer
    Tier As Integer        ' 1 key account, 2 mid-size, 3 small
    Since As Date
    Churn As Date          ' 0 = still a customer
    RepID As Long
    TaxRate As Double
    TermsDays As Integer
    Contacts As String     ' "id,id,..." first one is primary
    Prospect As Boolean
    Orders As Long
    Installed As Long      ' devices sold to date (drives subscription renewals)
End Type

Private db As DAO.Database
Private reps() As TRep, nReps As Long
Private prods() As TProd, nProds As Long
Private custs() As TCust, nCusts As Long
Private skuIdx As Object, repIdx As Object, usedNames As Object
Private conName() As String, conPhone() As String
Private mToday As Date, samID As Long, samHire As Date
Private rsC As DAO.Recordset, rsCt As DAO.Recordset
Private rsQ As DAO.Recordset, rsQL As DAO.Recordset, rsO As DAO.Recordset, rsOL As DAO.Recordset
Private rsP As DAO.Recordset, rsA As DAO.Recordset, rsM As DAO.Recordset
Private mMain As String, mDevices As Long, mSeq As Long
Private mLog As String
Private mWhere As String       ' progress marker for error messages
Private mAsOf As Date          ' day after the last order (operations history)

' One year's orders, generated first and then written in date order
Private stDate() As Date, stCust() As Long, stLines() As String, stSub() As Currency
Private stRenew() As Boolean, stCancel() As Boolean, stMain() As String, nSt As Long

Public Sub GenerateHistoryQuiet()
    GenerateHistory True
End Sub

Public Sub GenerateHistory(Optional ByVal quiet As Boolean = False)
    Dim stepName As String, y As Integer, inTrans As Boolean
    On Error GoTo Fail
    Set db = CurrentDb
    If DCount("*", "Orders") > 50 Then Err.Raise vbObjectError + 1, , "This database already has more than 50 orders - history was already added."
    If Not quiet Then
        If MsgBox("Add 10 years of FICTIONAL history to this database?" & vbCrLf & vbCrLf & _
                  "Only do this on a copy. It cannot be undone and takes a few minutes.", _
                  vbYesNo + vbExclamation, "Cairn Systems") <> vbYes Then Exit Sub
    End If

    Dim junk As Single: junk = Rnd(-1): Randomize SEED
    mToday = Date
    mLog = ""
    DBEngine.SetOption dbMaxLocksPerFile, 2000000
    Set skuIdx = CreateObject("Scripting.Dictionary")
    Set repIdx = CreateObject("Scripting.Dictionary")
    Set usedNames = CreateObject("Scripting.Dictionary")
    ReDim conName(0 To 50000): ReDim conPhone(0 To 50000)
    ReDim stDate(0 To 4000): ReDim stCust(0 To 4000): ReDim stLines(0 To 4000): ReDim stSub(0 To 4000)
    ReDim stRenew(0 To 4000): ReDim stCancel(0 To 4000): ReDim stMain(0 To 4000)

    stepName = "products": SetupProducts
    stepName = "sales reps": SetupReps
    OpenRecordsets
    stepName = "customers"
    DBEngine.BeginTrans: inTrans = True
    SetupCustomers
    DBEngine.CommitTrans: inTrans = False

    For y = FIRST_YEAR To Year(mToday)
        stepName = "year " & y
        DBEngine.BeginTrans: inTrans = True
        ReassignReps DateSerial(y, 1, 1), 0.15
        AgeInstalledBase
        GenerateYear y
        DBEngine.CommitTrans: inTrans = False
    Next

    stepName = "phone messages"
    DBEngine.BeginTrans: inTrans = True
    GenerateMessages
    DBEngine.CommitTrans: inTrans = False
    CloseRecordsets

    stepName = "quote numbers": Renumber "Quotes", "QuoteID", "QuoteNumber", "QuoteDate", "Q"
    stepName = "order numbers": Renumber "Orders", "OrderID", "OrderNumber", "OrderDate", "SO"
    stepName = "customer status": FinishCustomers
    stepName = "summary": Summarize
    If HasTable("RMAs") Then stepName = "operations": AddOperationsData

    If quiet Then
        WriteLog "CairnHistory_log.txt", mLog
    Else
        MsgBox "History added." & vbCrLf & vbCrLf & mLog, vbInformation, "Cairn Systems"
    End If
    Exit Sub
Fail:
    Dim msg As String: msg = "Error " & Err.Number & " during " & stepName & " (" & mWhere & "): " & Err.Description
    On Error Resume Next
    If inTrans Then DBEngine.Rollback
    CloseRecordsets
    If quiet Then WriteLog "CairnHistory_log.txt", msg Else MsgBox msg, vbCritical, "History generator failed"
End Sub

'==============================================================================
'  OPERATIONS HISTORY: sign-ins, returns, manufacturer orders, and the dirt a
'  ten-year-old Access file collects. GenerateHistory runs this at the end when
'  the AddOperationsTables tables exist; on a database that already has its
'  sales history, run AddOperationsHistory once. It uses its own random seed,
'  so the sales history is not changed.
'==============================================================================
Public Sub AddOperationsHistoryQuiet()
    AddOperationsHistory True
End Sub

Public Sub AddOperationsHistory(Optional ByVal quiet As Boolean = False)
    On Error GoTo Fail
    Set db = CurrentDb
    If Not HasTable("RMAs") Then Err.Raise vbObjectError + 2, , "Run AddOperationsTables (CairnBuild module) first."
    If DCount("*", "Orders") < 1000 Then Err.Raise vbObjectError + 3, , "Run GenerateHistory first."
    If DCount("*", "ManufacturerOrders") > 0 Then Err.Raise vbObjectError + 4, , "Operations history was already added."
    If Not quiet Then
        If MsgBox("Add FICTIONAL users, returns, manufacturer orders and messy data to this database?" & vbCrLf & vbCrLf & _
                  "Only do this on a copy. It cannot be undone.", vbYesNo + vbExclamation, "Cairn Systems") <> vbYes Then Exit Sub
    End If
    mLog = ""
    AddOperationsData
    If quiet Then WriteLog "CairnHistory_log.txt", mLog Else MsgBox "Operations history added." & vbCrLf & vbCrLf & mLog, vbInformation, "Cairn Systems"
    Exit Sub
Fail:
    Dim msg As String: msg = "Error " & Err.Number & " during " & mWhere & ": " & Err.Description
    If quiet Then WriteLog "CairnHistory_log.txt", msg Else MsgBox msg, vbCritical, "Operations history failed"
End Sub

Private Sub AddOperationsData()
    Dim junk As Single: junk = Rnd(-1): Randomize OPS_SEED
    DBEngine.SetOption dbMaxLocksPerFile, 2000000
    mAsOf = DMax("OrderDate", "Orders") + 1
    mSeq = 0
    mWhere = "users": OpsUsers
    mWhere = "RMAs": OpsRMAs
    mWhere = "RMA numbers": Renumber "RMAs", "RMAID", "RMANumber", "OpenedDate", "RMA"
    mWhere = "manufacturer orders": OpsMfrOrders
    mWhere = "duplicate customers": OpsDuplicateCustomers 14
    mWhere = "retired statuses": OpsRetiredStatuses
    mWhere = "orphaned quote lines": OpsOrphanQuoteLines 38
    mWhere = "pricing codes": OpsPricingCodes
    mWhere = "summary": OpsSummary
End Sub

Private Sub OpsUsers()
    Dim rs As DAO.Recordset, u As DAO.Recordset, pw, lastSeen As Variant
    pw = Array("cairn2016", "Summer2019!", "Password1", "sales123", "Welcome1", "letmein", "Cairn2020", "qwerty12")
    ' The users AddOperationsTables created get dates that fit the history
    db.Execute "UPDATE Users AS u INNER JOIN SalesReps AS r ON u.RepID = r.RepID SET u.CreatedDate = r.HireDate", dbFailOnError
    db.Execute "UPDATE Users SET CreatedDate = #2016-01-04# WHERE UserName = 'admin'", dbFailOnError

    Set u = db.OpenRecordset("Users", dbOpenDynaset)
    Set rs = db.OpenRecordset("SELECT RepID, FirstName, LastName, HireDate, Active FROM SalesReps " & _
                              "WHERE RepID NOT IN (SELECT RepID FROM Users WHERE RepID Is Not Null) ORDER BY RepID", dbOpenSnapshot)
    Do Until rs.EOF
        u.AddNew
        u!UserName = LCase(Left(rs!FirstName, 1) & rs!LastName)
        If Rnd < 0.3 Then u("Password") = LCase(rs!FirstName) & Year(rs!HireDate) Else u("Password") = Pick(pw)
        u!RepID = rs!RepID
        u!IsAdmin = False
        u!CreatedDate = rs!HireDate
        u!Active = (rs!LastName <> "Fitch")          ' nobody turned off the other departed reps' sign-ins
        u.Update
        rs.MoveNext
    Loop
    rs.Close
    ' Shared accounts that were never meant to last
    u.AddNew: u!UserName = "sales": u("Password") = "sales": u!CreatedDate = #2/6/2017#: u.Update
    u.AddNew: u!UserName = "temp": u("Password") = "temp123": u!CreatedDate = #7/12/2021#: u!LastLogin = #8/20/2021 4:52:00 PM#: u.Update
    u.Close

    ' Last sign-in: recent for current reps, around their last order for those who left
    Set u = db.OpenRecordset("SELECT u.UserID, u.UserName, u.LastLogin, r.Active, u.RepID FROM Users AS u LEFT JOIN SalesReps AS r ON u.RepID = r.RepID " & _
                             "WHERE u.LastLogin Is Null ORDER BY u.UserID", dbOpenDynaset)
    Do Until u.EOF
        If IsNull(u!RepID) Or Nz(u!Active, False) Then
            lastSeen = BizDay(mAsOf - RB(1, 4))
        Else
            lastSeen = DMax("OrderDate", "Orders", "RepID=" & u!RepID) + RB(0, 9)
        End If
        u.Edit
        u!LastLogin = lastSeen + RB(7 * 60, 17 * 60) / 1440
        u.Update
        u.MoveNext
    Loop
    u.Close
End Sub

' Returns against shipped hardware, plus some that point at orders that don't exist
Private Sub OpsRMAs()
    Dim rs As DAO.Recordset, r As DAO.Recordset, wty As Object, i As Integer, maxOrder As Long, cust As Variant, prod As Variant
    Set wty = CreateObject("Scripting.Dictionary")
    Set rs = db.OpenRecordset("SELECT DISTINCT ol.OrderID FROM OrderLines AS ol INNER JOIN Products AS p ON ol.ProductID = p.ProductID " & _
                              "WHERE p.SKU = 'CS-WTY-2'", dbOpenSnapshot)
    Do Until rs.EOF: wty(CStr(rs!OrderID)) = True: rs.MoveNext: Loop
    rs.Close

    Set r = db.OpenRecordset("RMAs", dbOpenDynaset, dbAppendOnly)
    Set rs = db.OpenRecordset("SELECT o.OrderID, o.CustomerID, o.RepID, o.ShipDate, ol.ProductID, p.SKU, ol.Quantity " & _
        "FROM (Orders AS o INNER JOIN OrderLines AS ol ON o.OrderID = ol.OrderID) INNER JOIN Products AS p ON ol.ProductID = p.ProductID " & _
        "WHERE o.ShipDate Is Not Null AND o.OrderStatus <> 'Cancelled' AND p.Category IN ('Sensor','Gateway','Module','Kit') " & _
        "ORDER BY o.OrderID, ol.OrderLineID", dbOpenSnapshot)
    Do Until rs.EOF
        If Rnd < 0.006 * Sqr(rs!Quantity) Then
            AddRMARow r, rs!OrderID, rs!CustomerID, rs!RepID, rs!ShipDate, rs!ProductID, rs!SKU, wty.Exists(CStr(rs!OrderID))
        End If
        rs.MoveNext
    Loop
    rs.Close

    ' Pilot units and walk-ins: an order number that was never in this file, or none at all
    maxOrder = DMax("OrderID", "Orders")
    For i = 1 To 35
        cust = DLookup("CustomerID", "Customers", "CustomerID >= " & RB(1, DMax("CustomerID", "Customers")) & " AND CustomerStatus <> 'Prospect'")
        prod = DLookup("ProductID", "Products", "Category = 'Sensor' AND ProductID >= " & RB(1, DMax("ProductID", "Products")))
        If Not IsNull(cust) And Not IsNull(prod) Then
            AddRMARow r, IIf(i <= 20, maxOrder + RB(1, 4000), Null), cust, Null, Null, prod, _
                      DLookup("SKU", "Products", "ProductID=" & prod), False
        End If
    Next
    r.Close
End Sub

Private Sub AddRMARow(r As DAO.Recordset, ByVal oid As Variant, ByVal cid As Long, ByVal rep As Variant, ByVal shipD As Variant, _
                      ByVal pid As Long, ByVal sku As String, ByVal extWty As Boolean)
    Dim d As Date, w As Integer, reason As String, status As String, age As Long
    w = WPick(Array(0.25, 0.5, 0.25))
    If IsNull(shipD) Then
        d = RandomBizDate(RB(2017, Year(mAsOf)), mAsOf - 1)
    Else
        Select Case w
        Case 0: d = BizDay(shipD + RB(1, 30))
        Case 1: d = BizDay(shipD + RB(31, 365))
        Case Else: d = BizDay(shipD + RB(366, 900))
        End Select
    End If
    If d >= mAsOf Then Exit Sub
    If w = 0 Then
        reason = Choose(WPick(Array(0.5, 0.3, 0.2)) + 1, "Dead on Arrival", "Damaged in Shipping", "Wrong Item Shipped")
    Else
        reason = Choose(WPick(Array(0.65, 0.22, 0.13)) + 1, "Failed in Warranty", "Intermittent Fault", "Customer Damage")
    End If
    age = mAsOf - d
    If age <= 14 Then
        status = Pick(Array("Open", "Open", "Received"))
    ElseIf age <= 30 Then
        status = Pick(Array("Received", "In Repair"))
    ElseIf reason = "Customer Damage" And Rnd < 0.6 Then
        status = "Rejected"
    Else
        status = Pick(Array("Replaced", "Replaced", "Credited", "Closed"))
    End If

    mSeq = mSeq + 1
    r.AddNew
    r!RMANumber = "TMPR" & mSeq
    r!CustomerID = cid
    If Not IsNull(oid) Then r!OrderID = oid
    r!ProductID = pid
    r!SerialNumber = Mid(sku, 4, 2) & RB(1, 9) & "-" & RandChars(6)
    r!OpenedDate = d
    If Not IsNull(rep) Then r!OpenedByRepID = rep
    r!Reason = reason
    r!ExtendedWarranty = extWty
    r!Description = RMAText(reason)
    r!RMAStatus = status
    Select Case status
    Case "Replaced": r!Disposition = "Replacement shipped"
    Case "Credited": r!Disposition = "Credit memo issued"
    Case "Closed": r!Disposition = "Repaired and returned"
    Case "Rejected": r!Disposition = "Physical damage - not covered"
    End Select
    If age > 30 Then
        r!ClosedDate = d + RB(4, 30)
        If status <> "Rejected" And Rnd < 0.35 Then r!ManufacturerRef = "MRMA-" & RB(20000, 89999)
    End If
    r.Update
End Sub

Private Function RMAText(ByVal reason As String) As String
    Select Case reason
    Case "Dead on Arrival": RMAText = Pick(Array("Never joined the network.", "No LED on power-up.", "Won't pair with the gateway.", "Unit dead out of the box."))
    Case "Damaged in Shipping": RMAText = Pick(Array("Housing cracked; box crushed.", "Antenna snapped in transit.", "Carton arrived wet."))
    Case "Wrong Item Shipped": RMAText = Pick(Array("Received indoor gateway, ordered outdoor.", "Wrong probe length.", "Shipped Gen 1 instead of current model."))
    Case "Failed in Warranty": RMAText = Pick(Array("Stopped reporting after a firmware update.", "Battery drains in under a month.", "Readings drift by several degrees.", "Radio drops off overnight."))
    Case "Intermittent Fault": RMAText = Pick(Array("Reports every few hours, then goes quiet.", "Random reboots in cold weather.", "Gaps in the data most mornings."))
    Case Else: RMAText = Pick(Array("Water inside the housing.", "Mount broken off; unit dropped.", "Opened by customer; seal broken."))
    End Select
End Function

' Every hardware order since the manufacturer API went live, as Access logged it.
' A few first attempts timed out after the manufacturer had already taken the
' order, so the retry ordered the hardware twice; Access only recorded one.
Private Sub OpsMfrOrders()
    Dim rs As DAO.Recordset, m As DAO.Recordset, users As Object, ref As Long, oid As Long
    Dim lines As String, hdr As String, sentAt As Date, x As Double, userID As Variant, rep As Variant, od As Date
    Set users = CreateObject("Scripting.Dictionary")
    Set rs = db.OpenRecordset("SELECT UserID, RepID FROM Users WHERE RepID Is Not Null", dbOpenSnapshot)
    Do Until rs.EOF: users(CStr(rs!RepID)) = rs!UserID.Value: rs.MoveNext: Loop
    rs.Close

    ref = 100000
    Set m = db.OpenRecordset("ManufacturerOrders", dbOpenDynaset, dbAppendOnly)
    Set rs = db.OpenRecordset("SELECT o.OrderID, o.OrderNumber, o.OrderDate, o.RepID, o.CustomerPO, c.CompanyName, c.ShipAddress, " & _
        "c.ShipCity, c.ShipState, c.ShipZip, p.SKU, ol.Quantity " & _
        "FROM ((Orders AS o INNER JOIN Customers AS c ON o.CustomerID = c.CustomerID) " & _
        "INNER JOIN OrderLines AS ol ON o.OrderID = ol.OrderID) INNER JOIN Products AS p ON ol.ProductID = p.ProductID " & _
        "WHERE o.OrderDate >= #" & Format(MFR_API_START, "yyyy-mm-dd") & "# AND o.OrderStatus <> 'Cancelled' " & _
        "AND p.Category IN ('Sensor','Gateway','Module','Kit','Accessory') " & _
        "ORDER BY o.OrderDate, o.OrderID, ol.OrderLineID", dbOpenSnapshot)
    Do Until rs.EOF
        oid = rs!OrderID: od = rs!OrderDate: rep = rs!RepID
        hdr = "{""dealer"":""CAIRN"",""po"":""" & rs!OrderNumber & """,""customerPo"":""" & Nz(rs!CustomerPO, "") & """," & _
              """shipTo"":{""name"":""" & rs!CompanyName & """,""address"":""" & Nz(rs!ShipAddress, "") & """,""city"":""" & _
              Nz(rs!ShipCity, "") & """,""state"":""" & Nz(rs!ShipState, "") & """,""zip"":""" & Nz(rs!ShipZip, "") & """},""lines"":["
        lines = ""
        Do While Not rs.EOF
            If rs!OrderID <> oid Then Exit Do
            lines = lines & IIf(Len(lines), ",", "") & "{""sku"":""" & rs!SKU & """,""qty"":" & rs!Quantity & "}"
            rs.MoveNext
        Loop
        sentAt = BizDay(od + RB(0, 2)) + RB(8 * 60, 17 * 60) / 1440
        If sentAt < mAsOf Then
            If users.Exists(CStr(rep)) Then userID = users(CStr(rep)) Else userID = Null
            x = Rnd
            If x < 0.022 Then
                ' timed out after the manufacturer took it (ref used up), retry ordered again
                ref = ref + 1
                MfrRow m, oid, 1, sentAt, userID, hdr & lines & "]}", Null, Null, "The operation timed out", Null
                ref = ref + 1
                MfrRow m, oid, 2, sentAt + 2 / 1440, userID, hdr & lines & "]}", 201, MfrReply(ref), Null, "MFR-" & ref
            ElseIf x < 0.027 Then
                ' connection dropped twice, then the user gave up; the manufacturer had both
                ref = ref + 2
                MfrRow m, oid, 1, sentAt, userID, hdr & lines & "]}", Null, Null, "The connection with the server was terminated abnormally", Null
                MfrRow m, oid, 2, sentAt + 1 / 1440, userID, hdr & lines & "]}", Null, Null, "The connection with the server was terminated abnormally", Null
            ElseIf x < 0.035 Then
                ' a server error that created nothing; the retry is fine
                MfrRow m, oid, 1, sentAt, userID, hdr & lines & "]}", 500, "{""error"":""internal error""}", Null, Null
                ref = ref + 1
                MfrRow m, oid, 2, sentAt + 3 / 1440, userID, hdr & lines & "]}", 201, MfrReply(ref), Null, "MFR-" & ref
            Else
                ref = ref + 1
                MfrRow m, oid, 1, sentAt, userID, hdr & lines & "]}", 201, MfrReply(ref), Null, "MFR-" & ref
            End If
        End If
    Loop
    rs.Close
    m.Close
End Sub

Private Function MfrReply(ByVal ref As Long) As String
    MfrReply = "{""mfrOrderId"":""MFR-" & ref & """,""status"":""accepted""}"
End Function

Private Sub MfrRow(m As DAO.Recordset, ByVal oid As Long, ByVal attempt As Integer, ByVal sentAt As Date, ByVal userID As Variant, _
                   ByVal body As String, ByVal status As Variant, ByVal resp As Variant, ByVal errText As Variant, ByVal ref As Variant)
    m.AddNew
    m!OrderID = oid
    m!Attempt = attempt
    m!SentAt = sentAt
    m!SentByUserID = userID
    m!RequestBody = body
    m!HttpStatus = status
    m!ResponseBody = resp
    m!ErrorText = errText
    m!ManufacturerRef = ref
    m.Update
End Sub

' The same customer entered twice under a slightly different name, with a
' recent order or two booked against the new record
Private Sub OpsDuplicateCustomers(ByVal n As Integer)
    Dim rs As DAO.Recordset, o As DAO.Recordset, ids() As Long, names() As String, cnt As Long
    Dim used As Object, i As Integer, k As Long, newID As Long, conID As Variant, first As Variant, tries As Integer
    Set used = CreateObject("Scripting.Dictionary")
    Set rs = db.OpenRecordset("SELECT c.CustomerID, c.CompanyName FROM Customers AS c WHERE c.CustomerStatus = 'Active' " & _
        "AND (SELECT Count(*) FROM Orders AS x WHERE x.CustomerID = c.CustomerID) BETWEEN 3 AND 15 ORDER BY c.CustomerID", dbOpenSnapshot)
    Do Until rs.EOF
        ReDim Preserve ids(0 To cnt): ReDim Preserve names(0 To cnt)
        ids(cnt) = rs!CustomerID: names(cnt) = rs!CompanyName: cnt = cnt + 1
        rs.MoveNext
    Loop
    rs.Close
    If cnt = 0 Then Exit Sub

    For i = 1 To n
        tries = 0
        Do
            k = RB(0, cnt - 1): tries = tries + 1
        Loop While used.Exists(k) And tries < 50
        used(k) = True
        db.Execute "INSERT INTO Customers (CompanyName, Industry, Phone, Email, Website, BillAddress, BillCity, BillState, BillZip, BillCountry, " & _
            "ShipAddress, ShipCity, ShipState, ShipZip, ShipCountry, AssignedRepID, PaymentTerms, TaxExempt, CustomerStatus) " & _
            "SELECT " & SqlText(NameVariant(names(k))) & ", Industry, Phone, Email, Website, BillAddress, BillCity, BillState, BillZip, BillCountry, " & _
            "ShipAddress, ShipCity, ShipState, ShipZip, ShipCountry, AssignedRepID, PaymentTerms, TaxExempt, 'Active' " & _
            "FROM Customers WHERE CustomerID = " & ids(k), dbFailOnError
        newID = db.OpenRecordset("SELECT @@IDENTITY")(0)
        db.Execute "INSERT INTO Contacts (CustomerID, FirstName, LastName, Title, Email, Phone, IsPrimary) " & _
            "SELECT TOP 1 " & newID & ", FirstName, LastName, Title, Email, Phone, -1 FROM Contacts " & _
            "WHERE CustomerID = " & ids(k) & " ORDER BY IsPrimary, ContactID", dbFailOnError
        conID = DMax("ContactID", "Contacts", "CustomerID = " & newID)
        first = Null
        Set o = db.OpenRecordset("SELECT TOP " & RB(1, 2) & " OrderID, QuoteID, OrderDate FROM Orders WHERE CustomerID = " & ids(k) & _
                                 " ORDER BY OrderDate DESC, OrderID DESC", dbOpenSnapshot)
        Do Until o.EOF
            db.Execute "UPDATE Orders SET CustomerID = " & newID & ", ContactID = " & Nz(conID, "Null") & " WHERE OrderID = " & o!OrderID, dbFailOnError
            If Not IsNull(o!QuoteID) Then
                db.Execute "UPDATE Quotes SET CustomerID = " & newID & ", ContactID = " & Nz(conID, "Null") & " WHERE QuoteID = " & o!QuoteID, dbFailOnError
            End If
            first = o!OrderDate
            o.MoveNext
        Loop
        o.Close
        If Not IsNull(first) Then db.Execute "UPDATE Customers SET CustomerSince = #" & Format(first, "yyyy-mm-dd") & "# WHERE CustomerID = " & newID, dbFailOnError
    Next
End Sub

Private Function NameVariant(ByVal s As String) As String
    Dim p As Long
    Select Case RB(0, 5)
    Case 0: NameVariant = s & " Inc."
    Case 1: NameVariant = UCase(s)
    Case 2: NameVariant = s & ", LLC"
    Case 3
        p = InStr(s, " ")
        If p > 5 Then NameVariant = Left(s, 2) & Mid(s, 4, 1) & Mid(s, 3, 1) & Mid(s, 5) Else NameVariant = s & " Inc"
    Case 4: NameVariant = Replace(s, " ", "  ", 1, 1)
    Case Else
        If InStr(s, " Co.") Then NameVariant = Replace(s, " Co.", " Company") Else NameVariant = "The " & s
    End Select
End Function

' Status values nobody uses any more, and rows saved by the old text-default bug
Private Sub OpsRetiredStatuses()
    db.Execute "UPDATE Orders SET OrderStatus = 'Back Order' WHERE OrderID IN (SELECT TOP 31 OrderID FROM Orders " & _
               "WHERE OrderStatus = 'Closed' AND OrderDate BETWEEN #2017-03-01# AND #2018-12-31# ORDER BY (OrderID * 7919) Mod 1009, OrderID)", dbFailOnError
    db.Execute "UPDATE Quotes SET QuoteStatus = 'On Hold' WHERE QuoteID IN (SELECT TOP 12 QuoteID FROM Quotes " & _
               "WHERE QuoteStatus = 'Expired' AND Year(QuoteDate) = 2018 ORDER BY (QuoteID * 7919) Mod 1009, QuoteID)", dbFailOnError
    db.Execute "UPDATE Quotes SET QuoteStatus = ""'Draft'"" WHERE QuoteID IN (SELECT TOP 9 QuoteID FROM Quotes " & _
               "WHERE QuoteStatus = 'Draft' ORDER BY (QuoteID * 7919) Mod 1009, QuoteID)", dbFailOnError
    db.Execute "UPDATE Customers SET CustomerStatus = ""'Prospect'"" WHERE CustomerID IN (SELECT TOP 4 CustomerID FROM Customers " & _
               "WHERE CustomerStatus = 'Prospect' ORDER BY (CustomerID * 7919) Mod 1009, CustomerID)", dbFailOnError
End Sub

' Someone removed the QuoteLines relationship (it kept refusing a delete) and
' put it back without enforcement; old declined quotes were later deleted and
' their lines stayed behind
Private Sub OpsOrphanQuoteLines(ByVal n As Integer)
    Dim rel As DAO.Relation
    On Error Resume Next
    db.Relations.Delete "FK_QuoteLines_Quote"
    On Error GoTo 0
    Set rel = db.CreateRelation("QuotesQuoteLines", "Quotes", "QuoteLines", dbRelationDontEnforce)
    rel.Fields.Append rel.CreateField("QuoteID")
    rel.Fields("QuoteID").ForeignName = "QuoteID"
    db.Relations.Append rel
    db.Execute "DELETE FROM Quotes WHERE QuoteID IN (SELECT TOP " & n & " QuoteID FROM Quotes WHERE QuoteStatus = 'Declined' " & _
               "AND Year(QuoteDate) = 2019 AND QuoteID NOT IN (SELECT QuoteID FROM Orders WHERE QuoteID Is Not Null) " & _
               "ORDER BY (QuoteID * 7919) Mod 1009, QuoteID)", dbFailOnError
End Sub

' Pricing arrangements typed into Notes as codes, each rep their own way
Private Sub OpsPricingCodes()
    Dim rs As DAO.Recordset, code As String
    Set rs = db.OpenRecordset("SELECT c.CustomerID, c.TaxExempt, c.Notes, (SELECT Count(*) FROM Orders AS o WHERE o.CustomerID = c.CustomerID) AS N " & _
                              "FROM Customers AS c WHERE c.CustomerStatus = 'Active' ORDER BY c.CustomerID", dbOpenSnapshot)
    Do Until rs.EOF
        If Rnd < 0.3 Then
            If rs!TaxExempt Then
                code = Pick(Array("[GOV]", "GOV", "gov pricing", "[GOV] PO required"))
            ElseIf rs!N >= 40 Then
                code = Pick(Array("[KEY]", "KEY ACCT", "key - 10% standing", "[KEY-10]"))
            Else
                code = Pick(Array("[DIST-15]", "DIST15", "dist 15%", "[NET60 OK]", "net 60 ok - per Priya", "PL-B"))
            End If
            db.Execute "UPDATE Customers SET Notes = " & SqlText(code & IIf(IsNull(rs!Notes), "", " " & rs!Notes)) & _
                       " WHERE CustomerID = " & rs!CustomerID, dbFailOnError
        End If
        rs.MoveNext
    Loop
    rs.Close
End Sub

Private Sub OpsSummary()
    mLog = mLog & vbCrLf & "Operations history:" & vbCrLf & _
        "  users " & Count1("SELECT Count(*) FROM Users") & " (" & _
        Count1("SELECT Count(*) FROM Users AS u INNER JOIN SalesReps AS r ON u.RepID = r.RepID WHERE u.Active AND NOT r.Active") & _
        " still active for reps who left)" & vbCrLf & _
        "  RMAs " & Count1("SELECT Count(*) FROM RMAs") & " (" & _
        Count1("SELECT Count(*) FROM RMAs AS r LEFT JOIN Orders AS o ON r.OrderID = o.OrderID WHERE o.OrderID Is Null") & " with no order on file)" & vbCrLf & _
        "  manufacturer calls " & Count1("SELECT Count(*) FROM ManufacturerOrders") & ", orders with a failed first attempt " & _
        Count1("SELECT Count(*) FROM ManufacturerOrders WHERE Attempt = 1 AND HttpStatus Is Null") & vbCrLf & _
        "  'Back Order' orders " & Count1("SELECT Count(*) FROM Orders WHERE OrderStatus = 'Back Order'") & _
        ", orphaned quote lines " & Count1("SELECT Count(*) FROM QuoteLines AS l LEFT JOIN Quotes AS q ON l.QuoteID = q.QuoteID WHERE q.QuoteID Is Null")
End Sub

Private Function Count1(ByVal sql As String) As Long
    Dim rs As DAO.Recordset
    Set rs = db.OpenRecordset(sql, dbOpenSnapshot)
    Count1 = Nz(rs(0), 0)
    rs.Close
End Function

Private Function SqlText(ByVal s As String) As String
    SqlText = "'" & Replace(s, "'", "''") & "'"
End Function

Private Function HasTable(ByVal tName As String) As Boolean
    Dim t As DAO.TableDef, d As DAO.Database
    Set d = CurrentDb
    For Each t In d.TableDefs
        If t.Name = tName Then HasTable = True: Exit Function
    Next
End Function

'------------------------------------------------------------------------------
'  REFERENCE DATA
'------------------------------------------------------------------------------
Private Function IndNames()
    IndNames = Array("Utilities", "Logistics", "Industrial", "Agriculture", "Building Management", _
                     "Food & Beverage", "Healthcare", "Transportation")
End Function

Private Function IndWeights()
    IndWeights = Array(0.12, 0.15, 0.18, 0.12, 0.18, 0.12, 0.06, 0.07)
End Function

Private Function IndSuffixes(ByVal i As Integer)
    Select Case i
    Case 0: IndSuffixes = Array("Water District", "Irrigation District", "Municipal Utilities", "Water Authority", "Sanitation District")
    Case 1: IndSuffixes = Array("Cold Storage", "Logistics", "Distribution", "Warehousing", "Fulfillment")
    Case 2: IndSuffixes = Array("Manufacturing", "Fabrication", "Plastics", "Metalworks", "Machine Works", "Industries")
    Case 3: IndSuffixes = Array("Vineyards", "Farms", "Orchards", "Growers", "Ranch", "Dairy")
    Case 4: IndSuffixes = Array("Facilities", "Properties", "Property Management", "Realty Group", "Campus Services")
    Case 5: IndSuffixes = Array("Brewing Co.", "Foods", "Creamery", "Bottling", "Bakery")
    Case 6: IndSuffixes = Array("Medical Center", "Health", "Clinics", "Senior Living")
    Case Else: IndSuffixes = Array("Transit", "Fleet Services", "Trucking", "Freight Lines")
    End Select
End Function

' Sensors each industry buys ("TH" = whichever temperature sensor was current)
Private Function IndSensors(ByVal i As Integer)
    Select Case i
    Case 0: IndSensors = Array("CS-WL150", "CS-WL150", "CS-PR310")
    Case 1: IndSensors = Array("TH", "TH", "CS-DR110", "CS-LK120")
    Case 2: IndSensors = Array("CS-VB220", "CS-VB220", "CS-EM600", "CS-PR310")
    Case 3: IndSensors = Array("CS-SM210", "CS-SM210", "TH", "CS-WL150")
    Case 4: IndSensors = Array("CS-AQ500", "CS-LK120", "CS-EM600", "CS-DR110", "TH")
    Case 5: IndSensors = Array("TH", "TH", "CS-LK120", "CS-PR310")
    Case 6: IndSensors = Array("TH", "CS-AQ500", "CS-DR110")
    Case Else: IndSensors = Array("CS-GPS700", "CS-GPS700", "TH")
    End Select
End Function

Private Function MonthWeights()
    MonthWeights = Array(0.85, 0.9, 1.05, 1, 1, 0.95, 0.85, 0.9, 1.05, 1.1, 1.1, 1.25)
End Function

Private Function NamePrefixes()
    NamePrefixes = Array("Alder", "Aspen", "Bayview", "Birchwood", "Blue Ridge", "Bridgeport", "Canyon", "Cascade", "Cedar", _
        "Clearwater", "Coastal", "Copper", "Crescent", "Crystal Springs", "Desert Sky", "Eagle", "Eastside", "Evergreen", _
        "Fairview", "Falcon", "Foothill", "Frontier", "Golden State", "Granite", "Harbor", "Hawthorne", "Heritage", "Highland", _
        "Horizon", "Ironwood", "Juniper", "Keystone", "Lakeside", "Lakeview", "Liberty", "Lone Pine", "Madrone", "Maple", _
        "Meadowbrook", "Mesa Verde", "Mission", "Mountain View", "North Valley", "Northgate", "Oak Creek", "Oakridge", "Pacific", _
        "Palisade", "Pinecrest", "Pioneer", "Prairie", "Quail Run", "Red Rock", "Redwood", "Ridgeline", "Riverbend", "Rocky Point", _
        "Saddleback", "Sage", "Sequoia", "Shasta", "Sierra", "Silver Creek", "Skyline", "Stonebridge", "Summit", "Sunrise", _
        "Sunset", "Tahoe", "Timberline", "Trinity", "Tule", "Valley Oak", "Vista", "Westfield", "Whitewater", "Willow Creek", "Yosemite")
End Function

Private Function FirstNames()
    FirstNames = Array("James", "Mary", "Robert", "Patricia", "John", "Jennifer", "Michael", "Linda", "David", "Elizabeth", _
        "William", "Barbara", "Richard", "Susan", "Joseph", "Jessica", "Thomas", "Sarah", "Carlos", "Maria", "Daniel", "Karen", _
        "Matthew", "Lisa", "Anthony", "Nancy", "Mark", "Betty", "Steven", "Sandra", "Andrew", "Ashley", "Kevin", "Emily", "Brian", _
        "Michelle", "Jose", "Amanda", "Luis", "Melissa", "Ryan", "Stephanie", "Jason", "Rebecca", "Eric", "Laura", "Juan", "Rachel", _
        "Kyle", "Angela", "Wei", "Mei", "Hiroshi", "Yuki", "Arjun", "Priyanka", "Omar", "Fatima", "Diego", "Sofia")
End Function

Private Function LastNames()
    LastNames = Array("Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis", "Rodriguez", "Martinez", _
        "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson", "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", _
        "Thompson", "White", "Harris", "Sanchez", "Clark", "Ramirez", "Lewis", "Robinson", "Walker", "Young", "Allen", "King", _
        "Wright", "Scott", "Torres", "Nguyen", "Hill", "Flores", "Green", "Adams", "Nelson", "Baker", "Hall", "Rivera", "Campbell", _
        "Mitchell", "Carter", "Roberts", "Chen", "Kim", "Patel", "Singh", "Tanaka", "Cohen", "Murphy", "Sullivan", "Reyes", "Ortiz")
End Function

'------------------------------------------------------------------------------
'  PRODUCTS AND REPS
'------------------------------------------------------------------------------
Private Sub SetupProducts()
    Dim p, f, rs As DAO.Recordset
    For Each p In Array( _
        "CS-TH050|Temperature/Humidity Sensor, Gen 1 (LoRaWAN)|Sensor|79|32|1.8.4|21|0", _
        "CS-GW400|LoRaWAN Gateway, 4-Channel, Indoor|Gateway|449|205|4.1.7|14|0", _
        "CS-DR110|Door / Contact Sensor|Sensor|49|17|1.4.0|14|-1", _
        "CS-LK120|Water Leak Detection Sensor|Sensor|69|24|1.6.2|14|-1", _
        "CS-SM210|Soil Moisture & Temperature Probe|Sensor|159|61|2.2.0|21|-1", _
        "CS-AQ500|Indoor Air Quality Sensor (CO2 / VOC / PM2.5)|Sensor|299|118|1.3.1|28|-1", _
        "CS-EM600|Energy / Current Monitor, 3-Phase|Sensor|229|92|1.1.0|28|-1", _
        "CS-GPS700|GPS Asset Tracker (LTE-M)|Sensor|189|74|2.0.3|21|-1", _
        "CS-CLD-36|Cairn Cloud Subscription - 36 months per device|Service|96|15||0|-1", _
        "CS-SVC-TRN|Operator Training (per session)|Service|950|400||10|-1", _
        "CS-WTY-2|Extended Warranty, 2 years per device|Service|14|3||0|-1")
        f = Split(p, "|")
        If DCount("*", "Products", "SKU='" & f(0) & "'") = 0 Then
            db.Execute "INSERT INTO Products (SKU, ProductName, Category, UnitPrice, UnitCost, FirmwareVersion, LeadTimeDays, Active) VALUES ('" & _
                f(0) & "','" & f(1) & "','" & f(2) & "'," & f(3) & "," & f(4) & "," & IIf(f(5) = "", "Null", "'" & f(5) & "'") & "," & f(6) & "," & f(7) & ")", dbFailOnError
        End If
    Next

    Set rs = db.OpenRecordset("SELECT ProductID, SKU, UnitPrice FROM Products ORDER BY ProductID", dbOpenSnapshot)
    nProds = 0
    Do Until rs.EOF
        ReDim Preserve prods(0 To nProds)
        With prods(nProds)
            .ID = rs!ProductID: .SKU = rs!SKU: .Price = rs!UnitPrice
            .Intro = FIRST_YEAR: .Disc = 0
            Select Case .SKU
            Case "CS-TH050": .Disc = 2019
            Case "CS-GW400": .Disc = 2020
            Case "CS-VB220", "CS-DR110": .Intro = 2017
            Case "CS-LK120", "CS-SM210", "CS-DEVKIT", "CS-WTY-2": .Intro = 2018
            Case "CS-TH100", "CS-AQ500", "CS-CLD-36": .Intro = 2019
            Case "CS-GW800", "CS-EM600": .Intro = 2020
            Case "CS-NB420", "CS-GPS700": .Intro = 2021
            End Select
        End With
        skuIdx(CStr(rs!SKU)) = nProds
        nProds = nProds + 1
        rs.MoveNext
    Loop
    rs.Close
End Sub

Private Sub SetupReps()
    Dim rs As DAO.Recordset, p, f, t As String
    ' Past and present reps added for the history (the four existing reps are kept)
    For Each p In Array( _
        "Raj|Malhotra|Sales Director|2013-03-01|2020-08-31|0.04", _
        "Ben|Hartley|Account Executive|2014-02-03|2021-06-30|0.06", _
        "Carla|Mendes|Senior Account Executive|2015-05-11||0.065", _
        "Owen|Fitch|Account Executive|2016-01-04|2019-11-15|0.06", _
        "Nadia|Petrova|Senior Account Executive|2017-08-21||0.065", _
        "Tessa|Grant|Account Executive|2018-04-09|2023-03-31|0.06", _
        "Victor|Huang|Account Executive|2019-10-14||0.06", _
        "Lena|Brandt|Account Executive|2021-01-11||0.06", _
        "Marcus|Doyle|Account Executive|2024-07-15||0.055", _
        "Elise|Moreau|Account Executive|2025-03-03||0.055")
        f = Split(p, "|")
        If DCount("*", "SalesReps", "FirstName='" & f(0) & "' AND LastName='" & f(1) & "'") = 0 Then
            db.Execute "INSERT INTO SalesReps (FirstName, LastName, Title, Email, Phone, Extension, CommissionRate, HireDate, Active) VALUES ('" & _
                f(0) & "','" & f(1) & "','" & f(2) & "','" & LCase(f(0) & "." & f(1)) & "@cairnsystems.example','555-01" & RB(20, 99) & "','" & _
                RB(120, 199) & "'," & f(5) & ",#" & f(3) & "#," & IIf(f(4) = "", "True", "False") & ")", dbFailOnError
        End If
    Next

    Set rs = db.OpenRecordset("SELECT * FROM SalesReps ORDER BY RepID", dbOpenSnapshot)
    nReps = 0
    Do Until rs.EOF
        ReDim Preserve reps(0 To nReps)
        With reps(nReps)
            .ID = rs!RepID
            .Hire = Nz(rs!HireDate, DateSerial(2012, 1, 1))
            .Leave = 0
            t = Nz(rs!Title, "")
            .Weight = 1
            If t Like "*Manager*" Or t Like "*Director*" Then .Weight = 0.4
            If t Like "*Inside*" Then .Weight = 0.3: samID = .ID: samHire = .Hire
        End With
        Select Case rs!LastName
        Case "Malhotra": reps(nReps).Leave = #8/31/2020#
        Case "Hartley": reps(nReps).Leave = #6/30/2021#
        Case "Fitch": reps(nReps).Leave = #11/15/2019#
        Case "Grant": reps(nReps).Leave = #3/31/2023#
        End Select
        repIdx(CLng(rs!RepID)) = nReps
        nReps = nReps + 1
        rs.MoveNext
    Loop
    rs.Close
End Sub

Private Function RepActive(ByVal r As Long, ByVal d As Date) As Boolean
    RepActive = (reps(r).Hire <= d And (reps(r).Leave = 0 Or d < reps(r).Leave))
End Function

' A rep (ID) working on date d, weighted by their share of accounts
Private Function PickRepAt(ByVal d As Date) As Long
    Dim r As Long, tot As Double, x As Double
    For r = 0 To nReps - 1
        If RepActive(r, d) Then tot = tot + reps(r).Weight
    Next
    If tot = 0 Then PickRepAt = reps(0).ID: Exit Function
    x = Rnd * tot
    For r = 0 To nReps - 1
        If RepActive(r, d) Then
            x = x - reps(r).Weight
            If x < 0 Then PickRepAt = reps(r).ID: Exit Function
        End If
    Next
    For r = nReps - 1 To 0 Step -1
        If RepActive(r, d) Then PickRepAt = reps(r).ID: Exit Function
    Next
End Function

' The customer's rep on date d (a stand-in covers if that rep wasn't there yet / had left)
Private Function RepAt(ByVal ci As Long, ByVal d As Date) As Long
    If RepActive(repIdx(custs(ci).RepID), d) Then RepAt = custs(ci).RepID Else RepAt = PickRepAt(d)
End Function

' Accounts whose rep has left move to a current rep; a share of the rest are rebalanced each year
Private Sub ReassignReps(ByVal d As Date, Optional ByVal rotateShare As Double = 0)
    Dim ci As Long
    For ci = 0 To nCusts - 1
        If Not RepActive(repIdx(custs(ci).RepID), d) Or Rnd < rotateShare Then custs(ci).RepID = PickRepAt(d)
    Next
End Sub

'------------------------------------------------------------------------------
'  CUSTOMERS AND CONTACTS
'------------------------------------------------------------------------------
Private Sub SetupCustomers()
    Dim rs As DAO.Recordset, rc As DAO.Recordset, i As Integer, y As Integer, n As Long, k As Long, d As Date
    Dim ind As Variant, lastDay As Date

    ' Existing sample customers
    Set rs = db.OpenRecordset("SELECT * FROM Customers ORDER BY CustomerID", dbOpenSnapshot)
    Do Until rs.EOF
        ReDim Preserve custs(0 To nCusts)
        With custs(nCusts)
            .ID = rs!CustomerID: .Name = rs!CompanyName
            .Industry = 4
            For i = 0 To 7
                If IndNames()(i) = Nz(rs!Industry, "") Then .Industry = i
            Next
            If rs!Industry = "Building Management" Then .Industry = 4
            .Tier = 2
            .Since = Nz(rs!CustomerSince, Date)
            .RepID = Nz(rs!AssignedRepID, reps(0).ID)
            .TaxRate = IIf(rs!TaxExempt, 0, 0.0825)
            .TermsDays = TermsToDays(Nz(rs!PaymentTerms, "Net 30"))
            .Prospect = (rs!CustomerStatus = "Prospect")
            Set rc = db.OpenRecordset("SELECT ContactID, FirstName, LastName, Phone FROM Contacts WHERE CustomerID=" & rs!CustomerID & " ORDER BY IsPrimary, ContactID", dbOpenSnapshot)
            Do Until rc.EOF
                .Contacts = .Contacts & IIf(Len(.Contacts), ",", "") & rc!ContactID
                conName(rc!ContactID) = rc!FirstName & " " & Nz(rc!LastName, "")
                conPhone(rc!ContactID) = Nz(rc!Phone, "")
                rc.MoveNext
            Loop
            rc.Close
        End With
        usedNames(CStr(rs!CompanyName)) = True
        nCusts = nCusts + 1
        rs.MoveNext
    Loop
    rs.Close

    ' Customers already on the books when the history starts
    For k = 1 To 140
        NewCustomer DateSerial(RB(2011, 2015), RB(1, 12), RB(1, 28)), False
    Next
    ' New customers each year (more as the company grows)
    For y = FIRST_YEAR To Year(mToday)
        lastDay = IIf(y = Year(mToday), mToday - 1, DateSerial(y, 12, 31))
        n = 38 + 4 * (y - FIRST_YEAR) + RB(0, 12)
        If y = Year(mToday) Then n = n * (lastDay - DateSerial(y, 1, 1)) / 365
        For k = 1 To n
            NewCustomer RandomBizDate(y, lastDay), False
        Next
    Next
    ' Prospects that never bought
    For k = 1 To 160
        y = RB(2018, Year(mToday))
        lastDay = IIf(y = Year(mToday), mToday - 1, DateSerial(y, 12, 31))
        NewCustomer RandomBizDate(y, lastDay), True
    Next
End Sub

Private Sub NewCustomer(ByVal since As Date, ByVal prospect As Boolean)
    Dim nm As String, ind As Integer, dom As String, st As String, city As String, zip As String
    Dim nCon As Integer, i As Integer, fn As String, ln As String, ph As String, terms As String, exempt As Boolean
    Dim yrs As Double

    ind = WPick(IndWeights())
    Do
        nm = Pick(NamePrefixes()) & " " & Pick(IndSuffixes(ind))
    Loop While usedNames.Exists(nm)
    usedNames(nm) = True
    dom = LCase(Replace(Replace(Replace(Replace(nm, " ", ""), ".", ""), "&", ""), "'", "")) & ".example"
    PickPlace st, city, zip

    ReDim Preserve custs(0 To nCusts)
    With custs(nCusts)
        .Name = nm: .Industry = ind: .Since = since: .Prospect = prospect
        Select Case Rnd
        Case Is < 0.08: .Tier = 1
        Case Is < 0.38: .Tier = 2
        Case Else: .Tier = 3
        End Select
        If prospect Then .Tier = 2 + Int(Rnd * 2)
        If Not prospect Then
            yrs = -Log(1 - Rnd) * IIf(.Tier = 1, 28, 14)
            If since + yrs * 365 < mToday Then .Churn = since + Int(yrs * 365)
        End If
        .RepID = PickRepAt(IIf(since < DateSerial(FIRST_YEAR, 1, 1), DateSerial(FIRST_YEAR, 1, 1), since))
        exempt = (ind = 0 And Rnd < 0.9) Or (ind = 6 And Rnd < 0.3)
        .TaxRate = IIf(exempt Or st <> "CA", 0, Pick(Array(0.0725, 0.0775, 0.0825, 0.0825, 0.0875, 0.095)))
        If .Tier = 1 Then
            terms = Pick(Array("Net 30", "Net 45", "Net 45", "Net 60"))
        Else
            terms = Pick(Array("Net 30", "Net 30", "Net 30", "Net 30", "Net 30", "Net 30", "Net 15", "Due on Receipt", "Net 45"))
        End If
        .TermsDays = TermsToDays(terms)

        rsC.AddNew
        .ID = rsC!CustomerID
        rsC!CompanyName = nm: rsC!Industry = IndNames()(ind)
        rsC!Phone = Phone555(): rsC!Email = Pick(Array("info@", "ap@", "orders@", "office@")) & dom
        rsC!Website = "www." & dom
        rsC!BillAddress = RB(100, 9899) & " " & Pick(Array("Main", "Oak", "Industrial", "Commerce", "Harbor", "Airport", "Market", "Valley", _
            "River", "Mill", "Ranch", "Park", "Center", "Enterprise", "Technology", "Foothill", "Orchard", "Canal", "Depot", "Union")) & " " & _
            Pick(Array("St", "Ave", "Blvd", "Rd", "Way", "Dr", "Pkwy", "Ct"))
        rsC!BillCity = city: rsC!BillState = st: rsC!BillZip = zip: rsC!BillCountry = "USA"
        rsC!ShipAddress = IIf(Rnd < 0.15, "Dock " & RB(1, 6) & ", ", "") & rsC!BillAddress
        rsC!ShipCity = city: rsC!ShipState = st: rsC!ShipZip = zip: rsC!ShipCountry = "USA"
        rsC!AssignedRepID = .RepID
        rsC!PaymentTerms = terms
        rsC!TaxExempt = exempt
        rsC!CustomerStatus = IIf(prospect, "Prospect", "Active")
        rsC!CustomerSince = since
        rsC.Update

        nCon = Choose(.Tier, RB(3, 5), RB(2, 3), RB(1, 2))
        For i = 1 To nCon
            fn = Pick(FirstNames()): ln = Pick(LastNames()): ph = Phone555()
            rsCt.AddNew
            rsCt!CustomerID = .ID: rsCt!FirstName = fn: rsCt!LastName = ln
            rsCt!Title = Pick(Array("Operations Manager", "Facilities Manager", "Plant Manager", "Purchasing Manager", "Maintenance Supervisor", _
                "Engineering Manager", "IT Director", "General Manager", "Controller", "Owner", "Director of Operations", "Quality Manager"))
            rsCt!Email = LCase(Left(fn, 1) & ln) & "@" & dom
            rsCt!Phone = ph
            If Rnd < 0.5 Then rsCt!Mobile = Phone555()
            rsCt!IsPrimary = (i = 1)
            .Contacts = .Contacts & IIf(i > 1, ",", "") & rsCt!ContactID
            conName(rsCt!ContactID) = fn & " " & ln
            conPhone(rsCt!ContactID) = ph
            rsCt.Update
        Next
    End With
    nCusts = nCusts + 1
End Sub

Private Sub PickPlace(st As String, city As String, zip As String)
    Select Case WPick(Array(0.58, 0.07, 0.08, 0.06, 0.07, 0.06, 0.04, 0.02, 0.02))
    Case 0: st = "CA": city = Pick(Array("Sacramento", "Fresno", "Stockton", "Modesto", "Bakersfield", "San Jose", "Oakland", "Salinas", "Santa Rosa", _
                "Visalia", "Riverside", "Ontario", "Irvine", "San Diego", "Chico", "Redding", "Merced", "Paso Robles", "Napa", "Lodi")): zip = CStr(RB(90001, 96161))
    Case 1: st = "OR": city = Pick(Array("Portland", "Salem", "Eugene", "Medford", "Bend")): zip = CStr(RB(97001, 97920))
    Case 2: st = "WA": city = Pick(Array("Seattle", "Tacoma", "Spokane", "Yakima", "Vancouver")): zip = CStr(RB(98001, 99403))
    Case 3: st = "NV": city = Pick(Array("Reno", "Las Vegas", "Sparks", "Carson City")): zip = CStr(RB(89001, 89883))
    Case 4: st = "AZ": city = Pick(Array("Phoenix", "Tucson", "Mesa", "Yuma", "Flagstaff")): zip = CStr(RB(85001, 86556))
    Case 5: st = "TX": city = Pick(Array("Dallas", "Houston", "Austin", "San Antonio", "Lubbock")): zip = CStr(RB(75001, 79999))
    Case 6: st = "CO": city = Pick(Array("Denver", "Greeley", "Fort Collins", "Pueblo")): zip = CStr(RB(80001, 81658))
    Case 7: st = "UT": city = Pick(Array("Salt Lake City", "Ogden", "Provo")): zip = CStr(RB(84001, 84791))
    Case Else: st = "ID": city = Pick(Array("Boise", "Twin Falls", "Idaho Falls")): zip = CStr(RB(83201, 83877))
    End Select
End Sub

Private Function TermsToDays(ByVal terms As String) As Integer
    If terms Like "Net *" Then TermsToDays = Val(Mid(terms, 5)) Else TermsToDays = 0
End Function

Private Function ContactFor(ByVal ci As Long) As Long
    Dim c: c = Split(custs(ci).Contacts, ",")
    If UBound(c) < 0 Then Exit Function
    If Rnd < 0.6 Then ContactFor = CLng(c(0)) Else ContactFor = CLng(Pick(c))
End Function

' Random customer who was buying on date d (key accounts more often)
Private Function PickCustomer(ByVal d As Date) As Long
    Dim t As Long, i As Long
    For t = 1 To 2000
        i = Int(Rnd * nCusts)
        With custs(i)
            If Not .Prospect And .Since <= d And (.Churn = 0 Or d < .Churn) Then
                If Rnd < Choose(.Tier, 1, 0.35, 0.12) Then PickCustomer = i: Exit Function
            End If
        End With
    Next
    PickCustomer = -1
End Function

Private Function PickProspect(ByVal d As Date) As Long
    Dim t As Long, i As Long
    For t = 1 To 2000
        i = Int(Rnd * nCusts)
        If custs(i).Prospect And custs(i).Since <= d Then PickProspect = i: Exit Function
    Next
    PickProspect = -1
End Function

'------------------------------------------------------------------------------
'  ORDERS
'------------------------------------------------------------------------------
Private Function YearTarget(ByVal y As Integer) As Double
    Dim g As Double, t As Double
    g = (SALES_LAST / SALES_FIRST) ^ (1 / (LAST_FULL_YEAR - FIRST_YEAR))
    t = SALES_FIRST * g ^ (y - FIRST_YEAR)
    If y > FIRST_YEAR And y < LAST_FULL_YEAR Then t = t * (0.98 + Rnd * 0.04)
    If y > LAST_FULL_YEAR Then t = t * SeasonFraction(mToday - 1)
    YearTarget = t
End Function

' Share of a year's (seasonal) sales that falls on or before d
Private Function SeasonFraction(ByVal d As Date) As Double
    Dim w, m As Integer, tot As Double, part As Double
    w = MonthWeights()
    For m = 1 To 12
        tot = tot + w(m - 1)
        If m < Month(d) Then part = part + w(m - 1)
    Next
    part = part + w(Month(d) - 1) * Day(d) / Day(DateSerial(Year(d), Month(d) + 1, 0))
    SeasonFraction = part / tot
End Function

Private Function OrderSize(ByVal tier As Integer) As Double
    Dim s As Double
    Select Case tier
    Case 1: s = LogNorm(40000, 0.75)
    Case 2: s = LogNorm(11000, 0.7)
    Case Else: s = LogNorm(3200, 0.65)
    End Select
    If s < 400 Then s = 400
    If s > 600000 Then s = 600000
    OrderSize = s
End Function

Private Sub GenerateYear(ByVal y As Integer)
    Dim target As Double, total As Double, lastDay As Date, d As Date, ci As Long
    Dim S As Double, renew As Boolean, lines As String, subT As Currency, i As Long, idx() As Long, won As Long, midYear As Boolean

    lastDay = IIf(y = Year(mToday), mToday - 1, DateSerial(y, 12, 31))
    target = YearTarget(y)
    nSt = 0
    mWhere = "staging orders"
    Do While total < target - 2500
        mWhere = "staging: date": d = RandomBizDate(y, lastDay)
        mWhere = "staging: customer": ci = PickCustomer(d)
        If ci >= 0 Then
            mWhere = "staging: lines for customer " & ci & " industry " & custs(ci).Industry
            renew = False
            If custs(ci).Installed > 30 And Rnd < 0.16 Then
                renew = True
                S = custs(ci).Installed * (0.4 + Rnd * 0.6) * 36
                If S > 60000 Then S = 60000 * (0.7 + Rnd * 0.6)
            Else
                S = OrderSize(custs(ci).Tier)
            End If
            If target - total < 25000 And S > target - total Then S = target - total
            lines = BuildLines(ci, y, S, renew, subT)
            mWhere = "staging: store " & nSt: If nSt > UBound(stDate) Then GrowStage
            stDate(nSt) = d: stCust(nSt) = ci: stLines(nSt) = lines: stSub(nSt) = subT
            stRenew(nSt) = renew: stMain(nSt) = mMain
            stCancel(nSt) = (Rnd < 0.02)
            If Not stCancel(nSt) Then
                total = total + subT
                custs(ci).Orders = custs(ci).Orders + 1
                custs(ci).Installed = custs(ci).Installed + mDevices
                won = won + 1
            End If
            nSt = nSt + 1
        End If
    Loop

    mWhere = "sorting": SortStage idx
    For i = 0 To nSt - 1
        If Not midYear And stDate(idx(i)) >= DateSerial(y, 7, 1) Then ReassignReps DateSerial(y, 7, 1), 0.08: midYear = True
        mWhere = "writing order " & i: ProcessOrder idx(i)
    Next
    mWhere = "lost quotes": LostQuotes y, lastDay, Round(won * 0.7)
    mWhere = "check-ins": CheckIns y, lastDay
End Sub

' Older devices drop out of subscription renewals (replaced, retired or renewed on multi-year terms)
Private Sub AgeInstalledBase()
    Dim ci As Long
    For ci = 0 To nCusts - 1
        custs(ci).Installed = Int(custs(ci).Installed * 0.6)
    Next
End Sub

Private Sub GrowStage()
    Dim n As Long: n = UBound(stDate) * 2
    ReDim Preserve stDate(0 To n): ReDim Preserve stCust(0 To n): ReDim Preserve stLines(0 To n)
    ReDim Preserve stSub(0 To n): ReDim Preserve stRenew(0 To n): ReDim Preserve stCancel(0 To n): ReDim Preserve stMain(0 To n)
End Sub

' Line items for an order of roughly S dollars. Returns "productID|qty|price|disc;..."
Private Function BuildLines(ByVal ci As Long, ByVal y As Integer, ByVal S As Double, ByVal renew As Boolean, subT As Currency) As String
    Dim ls As String, p1 As Long, p2 As Long, gw As Long, q As Long, q1 As Long
    Dim unitCost As Double, fixedCost As Double, disc As Double
    Dim kit As Boolean, cloud As Boolean, inst As Boolean, trn As Boolean, wty As Boolean, nb As Boolean, gps As Boolean
    Dim days As Long, sessions As Long, cl As Long

    subT = 0: mDevices = 0
    If renew Then
        If y >= 2019 And Rnd < 0.35 Then cl = skuIdx("CS-CLD-36") Else cl = skuIdx("CS-CLD-12")
        q = Round(S / YearPrice(cl, y)): If q < 5 Then q = 5
        disc = IIf(q >= 500, 0.08, IIf(q >= 100, 0.04, 0))
        AddLine ls, subT, cl, q, y, disc
        mMain = q & " x " & prods(cl).SKU & " (renewal)"
        BuildLines = ls: Exit Function
    End If
    If custs(ci).Orders = 0 And S < 4000 And y >= 2018 And Rnd < 0.35 Then
        q = RB(1, 3)
        AddLine ls, subT, skuIdx("CS-DEVKIT"), q, y, 0
        mMain = q & " x CS-DEVKIT"
        BuildLines = ls: Exit Function
    End If

    p1 = SensorFor(ci, y): p2 = -1
    If Rnd < 0.3 Then p2 = SensorFor(ci, y)
    If p2 = p1 Then p2 = -1
    gps = (prods(p1).SKU = "CS-GPS700")
    gw = skuIdx(IIf(y < 2020, "CS-GW400", "CS-GW800"))
    kit = (Rnd < 0.7): cloud = (Rnd < 0.85)
    wty = (y >= 2018 And Rnd < 0.25)
    nb = (custs(ci).Industry = 7 And y >= 2021 And Not gps And Rnd < 0.5)
    inst = (S > 15000 And Rnd < 0.5)
    trn = (S > 40000 And Rnd < 0.3)

    unitCost = YearPrice(p1, y)
    If p2 >= 0 Then unitCost = 0.7 * unitCost + 0.3 * YearPrice(p2, y)
    If kit Then unitCost = unitCost + YearPrice(skuIdx("CS-MK01"), y)
    If cloud Then unitCost = unitCost + YearPrice(skuIdx("CS-CLD-12"), y)
    If wty Then unitCost = unitCost + YearPrice(skuIdx("CS-WTY-2"), y)
    If nb Then unitCost = unitCost + YearPrice(skuIdx("CS-NB420"), y)
    If Not gps Then unitCost = unitCost + YearPrice(gw, y) / 50
    If inst Then
        days = Round(S / 40000): If days < 1 Then days = 1
        fixedCost = days * YearPrice(skuIdx("CS-SVC-INST"), y)
    End If
    If trn Then
        sessions = RB(1, 3)
        fixedCost = fixedCost + sessions * YearPrice(skuIdx("CS-SVC-TRN"), y)
    End If
    q = Round((S - fixedCost) / unitCost): If q < 5 Then q = 5

    Select Case q
    Case Is >= 1000: disc = 0.12
    Case Is >= 500: disc = 0.1
    Case Is >= 200: disc = 0.07
    Case Is >= 100: disc = 0.05
    Case Else: disc = 0
    End Select
    If custs(ci).Tier = 1 Then disc = disc + 0.02

    q1 = q
    If p2 >= 0 Then q1 = Round(q * 0.7): If q1 < 1 Then q1 = 1
    AddLine ls, subT, p1, q1, y, disc
    If p2 >= 0 And q > q1 Then AddLine ls, subT, p2, q - q1, y, disc
    If Not gps Then AddLine ls, subT, gw, -Int(-q / 50), y, 0
    If nb Then AddLine ls, subT, skuIdx("CS-NB420"), q, y, disc
    If kit Then AddLine ls, subT, skuIdx("CS-MK01"), q, y, 0
    If cloud Then AddLine ls, subT, skuIdx("CS-CLD-12"), q, y, Round(disc / 2, 2)
    If wty Then AddLine ls, subT, skuIdx("CS-WTY-2"), q, y, 0
    If inst Then AddLine ls, subT, skuIdx("CS-SVC-INST"), days, y, 0
    If trn Then AddLine ls, subT, skuIdx("CS-SVC-TRN"), sessions, y, 0
    mDevices = q
    mMain = q & " x " & prods(p1).SKU
    If p2 >= 0 Then mMain = mMain & " + " & prods(p2).SKU
    BuildLines = ls
End Function

Private Sub AddLine(s As String, subT As Currency, ByVal pIdx As Long, ByVal qty As Long, ByVal y As Integer, ByVal disc As Double)
    Dim price As Currency: price = YearPrice(pIdx, y)
    s = s & prods(pIdx).ID & "|" & qty & "|" & Trim(Str(price)) & "|" & Trim(Str(disc)) & ";"
    subT = subT + Round(qty * price * (1 - disc), 2)
End Sub

' List price in a given year (about 3% a year lower going back)
Private Function YearPrice(ByVal pIdx As Long, ByVal y As Integer) As Currency
    If y >= Year(mToday) Then
        YearPrice = prods(pIdx).Price
    Else
        YearPrice = CCur(Round(prods(pIdx).Price / 1.03 ^ (Year(mToday) - y), 0))
        If YearPrice < 1 Then YearPrice = 1
    End If
End Function

Private Function Avail(ByVal pIdx As Long, ByVal y As Integer) As Boolean
    Avail = (prods(pIdx).Intro <= y And (prods(pIdx).Disc = 0 Or y < prods(pIdx).Disc))
End Function

Private Function SensorFor(ByVal ci As Long, ByVal y As Integer) As Long
    Dim fam, t As Integer, s As String, i As Long, th As String
    th = IIf(y < 2019, "CS-TH050", "CS-TH100")
    fam = IndSensors(custs(ci).Industry)
    For t = 1 To 8
        s = Pick(fam)
        If s = "TH" Then s = th
        i = skuIdx(s)
        If Avail(i, y) Then SensorFor = i: Exit Function
    Next
    SensorFor = skuIdx(th)
End Function

Private Function ShipCost(ByVal subT As Currency, ByVal serviceOnly As Boolean) As Currency
    If serviceOnly Or subT < 600 Then Exit Function
    ShipCost = Round(25 + subT * 0.004, 0)
    If ShipCost > 750 Then ShipCost = 750
End Function

Private Sub ProcessOrder(ByVal k As Long)
    Dim ci As Long, d As Date, rep As Long, con As Long, rate As Double
    Dim subT As Currency, tax As Currency, ship As Currency, total As Currency, paid As Currency
    Dim qid As Long, qd As Date, oid As Long, cancelled As Boolean, shipD As Date, shipped As Boolean
    Dim payD(1) As Date, payA(1) As Currency, nPay As Integer, i As Integer, status As String, via As String

    ci = stCust(k): d = stDate(k): subT = stSub(k): cancelled = stCancel(k)
    rep = RepAt(ci, d): con = ContactFor(ci): rate = custs(ci).TaxRate
    ship = ShipCost(subT, stRenew(k))
    tax = Round(subT * rate, 2)
    total = subT + tax + ship

    ' Most orders start as a quote
    If Not stRenew(k) And Rnd < 0.88 Then
        qd = PrevBizDay(d - RB(3, 45))
        If qd < custs(ci).Since Then qd = custs(ci).Since
        qid = AddQuote(ci, con, rep, qd, "Accepted", rate, ship, stLines(k))
        AddAct ci, con, rep, qd - RB(2, 20), Pick(Array("Call", "Meeting", "Meeting")), _
            Pick(Array("Discovery call", "Site walk", "Needs assessment", "Pilot review", "Dashboard demo")), _
            "Scoped " & stMain(k) & " for " & Pick(Array("an expansion", "a new site", "a pilot rollout", "replacing legacy data loggers", "compliance monitoring")) & "."
        AddAct ci, con, rep, qd, "Email", "Quote sent", "Sent quote for " & stMain(k) & "."
        AddAct ci, con, rep, d - RB(0, 3), "Call", "Verbal approval", "Customer approved the quote; PO to follow."
    End If

    If stRenew(k) Then shipD = BizDay(d + RB(0, 2)) Else shipD = BizDay(d + RB(7, 35))
    shipped = (Not cancelled And shipD < mToday)

    nPay = 0
    If Not cancelled Then
        If total > 2000 And Rnd < 0.35 Then
            payD(0) = BizDay(d + RB(0, 5)): payA(0) = Round(total / 2, 2)
            payD(1) = PayDue(ci, shipD): payA(1) = total - payA(0): nPay = 2
        Else
            payD(0) = PayDue(ci, shipD): payA(0) = total: nPay = 1
        End If
        For i = 0 To nPay - 1
            If payD(i) < mToday Then paid = paid + payA(i)
        Next
    End If

    If cancelled Then
        status = "Cancelled"
    ElseIf Not shipped Then
        status = IIf(d >= mToday - 7, "Open", "In Production")
    ElseIf paid >= total - 0.005 Then
        status = "Closed"
    ElseIf mToday - shipD <= 10 Then
        status = "Shipped"
    Else
        status = "Invoiced"
    End If

    rsO.AddNew
    oid = rsO!OrderID
    mSeq = mSeq + 1
    rsO!OrderNumber = "TMPO" & mSeq
    rsO!CustomerID = custs(ci).ID
    If con > 0 Then rsO!ContactID = con
    rsO!RepID = rep
    If qid > 0 Then rsO!QuoteID = qid
    If Rnd < 0.7 Then rsO!CustomerPO = Pick(Array("PO-", "PO ", "", "P")) & RB(10000, 99999)
    rsO!OrderDate = d
    rsO!RequiredDate = d + RB(14, 45)
    If shipped Then
        rsO!ShipDate = shipD
        If stRenew(k) Then
            via = "Electronic delivery"
        ElseIf Rnd < 0.03 Then
            via = "Customer Pickup"
        ElseIf subT > 30000 Then
            via = "FedEx Freight"
        Else
            via = Pick(Array("UPS Ground", "UPS Ground", "FedEx Ground"))
        End If
        rsO!ShipVia = via
        Select Case via
        Case "UPS Ground": rsO!TrackingNumber = "1Z" & RandChars(16)
        Case "FedEx Ground": rsO!TrackingNumber = RB(1000, 9999) & RB(1000, 9999) & RB(1000, 9999)
        Case "FedEx Freight": rsO!TrackingNumber = "PRO " & RB(100000000, 999999999)
        End Select
    End If
    rsO!OrderStatus = status
    rsO!TaxRate = rate
    rsO!ShippingCost = ship
    rsO.Update
    AddLines rsOL, "OrderID", oid, stLines(k)

    AddAct ci, con, rep, d, "Note", "Order booked", "PO received for " & stMain(k) & _
        IIf(cancelled, ". Later cancelled by the customer.", ". Target ship " & Format(shipD, "m/d/yyyy") & ".")
    For i = 0 To nPay - 1
        If payD(i) < mToday Then AddPayment oid, payD(i), payA(i), IIf(nPay = 2, IIf(i = 0, "50% deposit", "Final payment"), Null)
    Next
End Sub

Private Function PayDue(ByVal ci As Long, ByVal shipD As Date) As Date
    If custs(ci).TermsDays = 0 Then
        PayDue = BizDay(shipD + RB(0, 7))
    Else
        PayDue = BizDay(shipD + custs(ci).TermsDays + RB(-6, 18))
    End If
End Function

Private Function AddQuote(ByVal ci As Long, ByVal con As Long, ByVal rep As Long, ByVal qd As Date, ByVal status As String, _
                          ByVal rate As Double, ByVal ship As Currency, ByVal lines As String) As Long
    rsQ.AddNew
    AddQuote = rsQ!QuoteID
    mSeq = mSeq + 1
    rsQ!QuoteNumber = "TMPQ" & mSeq
    rsQ!CustomerID = custs(ci).ID
    If con > 0 Then rsQ!ContactID = con
    rsQ!RepID = rep
    rsQ!QuoteDate = qd
    rsQ!ValidUntil = qd + 30
    rsQ!QuoteStatus = status
    rsQ!TaxRate = rate
    rsQ!ShippingCost = ship
    rsQ.Update
    AddLines rsQL, "QuoteID", AddQuote, lines
End Function

Private Sub AddLines(rs As DAO.Recordset, ByVal parentField As String, ByVal parentID As Long, ByVal lines As String)
    Dim p, f
    For Each p In Split(lines, ";")
        If Len(p) Then
            f = Split(p, "|")
            rs.AddNew
            rs(parentField) = parentID
            rs!ProductID = CLng(f(0))
            rs!Quantity = CLng(f(1))
            rs!UnitPrice = CCur(Val(f(2)))
            rs!DiscountPct = Val(f(3))
            rs.Update
        End If
    Next
End Sub

Private Sub AddPayment(ByVal oid As Long, ByVal d As Date, ByVal amt As Currency, ByVal note As Variant)
    Dim m As String
    If amt >= 20000 Then
        m = Pick(Array("ACH", "ACH", "ACH", "ACH", "ACH", "Wire", "Wire", "Wire", "Check", "Check"))
    ElseIf amt < 10000 Then
        m = Pick(Array("Check", "Check", "Check", "Check", "Credit Card", "Credit Card", "Credit Card", "ACH", "ACH", "ACH"))
    Else
        m = Pick(Array("ACH", "Check"))
    End If
    rsP.AddNew
    rsP!OrderID = oid
    rsP!PaymentDate = d
    rsP!Amount = amt
    rsP!PaymentMethod = m
    Select Case m
    Case "Check": rsP!PaymentRef = "CHK " & RB(1000, 99999)
    Case "ACH": rsP!PaymentRef = "ACH-" & RB(100000, 999999)
    Case "Wire": rsP!PaymentRef = "WIRE-" & RB(10000, 99999)
    Case Else: rsP!PaymentRef = Pick(Array("VISA", "MC", "AMEX")) & " x" & RB(1000, 9999)
    End Select
    If samID > 0 And d >= samHire Then rsP!ReceivedByRepID = samID
    rsP!Notes = note
    rsP.Update
End Sub

'------------------------------------------------------------------------------
'  LOST QUOTES, CRM ACTIVITY, PHONE MESSAGES
'------------------------------------------------------------------------------
Private Sub LostQuotes(ByVal y As Integer, ByVal lastDay As Date, ByVal n As Long)
    Dim i As Long, d As Date, ci As Long, lines As String, subT As Currency, st As String
    Dim con As Long, rep As Long
    For i = 1 To n
        d = RandomBizDate(y, lastDay)
        ci = -1
        If Rnd < 0.4 Then ci = PickProspect(d)
        If ci < 0 Then ci = PickCustomer(d)
        If ci >= 0 Then
            lines = BuildLines(ci, y, OrderSize(custs(ci).Tier), False, subT)
            con = ContactFor(ci): rep = RepAt(ci, d)
            If d >= mToday - 5 Then
                st = "Draft"
            ElseIf d >= mToday - 30 Then
                st = "Sent"
            ElseIf Rnd < 0.55 Then
                st = "Declined"
            Else
                st = "Expired"
            End If
            AddQuote ci, con, rep, d, st, custs(ci).TaxRate, ShipCost(subT, False), lines
            If st <> "Draft" Then AddAct ci, con, rep, d, "Email", "Quote sent", "Sent quote for " & mMain & "."
            Select Case st
            Case "Declined"
                AddAct ci, con, rep, d + RB(5, 25), "Call", "Quote follow-up", Pick(Array( _
                    "Went with a competitor on price.", "Budget pushed to next fiscal year.", "Project on hold.", _
                    "Chose a wired system instead.", "Decided to keep the existing data loggers for now."))
            Case "Expired"
                AddAct ci, con, rep, d + RB(30, 35), "Note", "Quote expired", "No decision before the quote expired."
            Case "Sent"
                AddAct ci, con, rep, d + RB(3, 7), "Call", "Quote follow-up", "Left voicemail; will try again."
            End Select
        End If
    Next
End Sub

Private Sub CheckIns(ByVal y As Integer, ByVal lastDay As Date)
    Dim ci As Long, n As Integer, j As Integer, d As Date, k As Integer, subj, det
    subj = Array("Quarterly business review", "Account check-in", "Firmware update notice", "Renewal reminder", "Support follow-up")
    det = Array("Reviewed uptime and alert history. No issues.", "Customer is happy with the system; asked about new sensor types.", _
                "Told the customer about the firmware update and scheduled the rollout.", "Reminded the customer that cloud subscriptions renew soon.", _
                "Followed up on a gateway connectivity ticket; resolved.")
    For ci = 0 To nCusts - 1
        With custs(ci)
            If .Since <= lastDay And (.Churn = 0 Or .Churn > DateSerial(y, 1, 1)) Then
                If .Prospect Then n = IIf(Rnd < 0.5, 1, 0) Else n = Choose(.Tier, 4, 2, 1)
                For j = 1 To n
                    d = RandomBizDate(y, lastDay)
                    If d >= .Since And (.Churn = 0 Or d < .Churn) Then
                        If .Prospect Then
                            AddAct ci, ContactFor(ci), RepAt(ci, d), d, Pick(Array("Call", "Email")), "Prospect touch-base", _
                                Pick(Array("Sent case studies and brochure.", "Intro call; timing is next quarter.", "Invited to a webinar.", "No current project; check back later."))
                        Else
                            k = Int(Rnd * 5)
                            AddAct ci, ContactFor(ci), RepAt(ci, d), d, Pick(Array("Call", "Meeting", "Email")), subj(k), det(k)
                        End If
                    End If
                Next
            End If
        End With
    Next
End Sub

Private Sub AddAct(ByVal ci As Long, ByVal con As Long, ByVal rep As Long, ByVal d As Date, ByVal typ As String, _
                   ByVal subj As String, ByVal details As String)
    Dim fu As Date
    d = BizDay(Int(d)) + TimeSerial(RB(8, 16), RB(0, 11) * 5, 0)
    If d >= mToday Then Exit Sub
    rsA.AddNew
    rsA!CustomerID = custs(ci).ID
    If con > 0 Then rsA!ContactID = con
    rsA!RepID = rep
    rsA!ActivityDate = d
    rsA!ActivityType = typ
    rsA!Subject = Left(subj, 150)
    rsA!Details = details
    If d >= mToday - 30 And Rnd < 0.25 Then
        fu = BizDay(Int(d) + RB(3, 21))
        rsA!FollowUpDate = fu
        rsA!Completed = (fu < mToday - 1 And Rnd < 0.8)
    Else
        rsA!Completed = True
    End If
    rsA.Update
End Sub

Private Sub GenerateMessages()
    Dim d As Date, n As Integer, i As Integer, ci As Long, con As Long, t As Date, forRep As Long, taker As Long, tries As Integer
    Dim msgs, notes
    msgs = Array("Asked about pricing on additional sensors.", "Needs help resetting a gateway.", "Question about an invoice.", _
        "Wants to schedule a site visit.", "Calling about the delivery date.", "Interested in the air quality sensor.", _
        "Returning your call.", "Battery replacement question.", "Wants a quote for an expansion.", "Alerts not reaching their phones.", _
        "Asked for a W-9 and certificate of insurance.", "Wants to add users to the cloud dashboard.")
    notes = Array("Left voicemail.", "Spoke with caller; resolved.", "Scheduled a follow-up meeting.", "Sent the requested info by email.", "Forwarded to support.")
    d = DateSerial(FIRST_YEAR, 1, 4)
    Do While d < mToday
        If Weekday(d, vbMonday) <= 5 Then
            n = RB(0, 2 + (Year(d) - FIRST_YEAR) \ 3)
            For i = 1 To n
                ci = -1
                If Rnd < 0.9 Then
                    For tries = 1 To 30
                        ci = Int(Rnd * nCusts)
                        If custs(ci).Since <= d And (custs(ci).Churn = 0 Or d < custs(ci).Churn) Then Exit For
                        ci = -1
                    Next
                End If
                t = d + TimeSerial(RB(8, 16), RB(0, 59), 0)
                rsM.AddNew
                rsM!ReceivedAt = t
                If ci >= 0 Then
                    con = ContactFor(ci): forRep = RepAt(ci, d)
                    rsM!CustomerID = custs(ci).ID
                    rsM!CallerName = conName(con)
                    rsM!CallerCompany = custs(ci).Name
                    rsM!CallerPhone = conPhone(con)
                Else
                    forRep = PickRepAt(d)
                    rsM!CallerName = Pick(FirstNames()) & " " & Pick(LastNames())
                    rsM!CallerCompany = Pick(NamePrefixes()) & " " & Pick(Array("Consulting", "Electric", "Controls", "Systems Integrators", "Engineering"))
                    rsM!CallerPhone = Phone555()
                End If
                If samID > 0 And d >= samHire Then
                    taker = samID
                Else
                    taker = forRep
                    For tries = 1 To 5
                        taker = PickRepAt(d)
                        If taker <> forRep Then Exit For
                    Next
                End If
                rsM!ForRepID = forRep
                rsM!TakenByRepID = taker
                rsM!Message = Pick(msgs)
                rsM!Urgent = (Rnd < 0.08)
                rsM!PleaseCallBack = (Rnd < 0.8)
                rsM!WillCallAgain = (Rnd < 0.15)
                If t < mToday - 2 Then
                    If Rnd < 0.9 Then
                        rsM!MessageStatus = "Returned"
                        rsM!ReturnedAt = t + RB(1, 26) / 24
                        rsM!ReturnNotes = Pick(notes)
                    Else
                        rsM!MessageStatus = "Closed"
                    End If
                Else
                    rsM!MessageStatus = "New"
                End If
                rsM.Update
            Next
        End If
        d = d + 1
    Loop
End Sub

'------------------------------------------------------------------------------
'  FINISHING
'------------------------------------------------------------------------------
' Number quotes / orders in date order within each year: Q-2016-0001, SO-2016-0001 ...
Private Sub Renumber(ByVal tbl As String, ByVal idFld As String, ByVal numFld As String, ByVal dateFld As String, ByVal prefix As String)
    Dim rs As DAO.Recordset, y As Integer, n As Long
    db.Execute "UPDATE " & tbl & " SET " & numFld & " = 'TMP' & " & idFld, dbFailOnError
    Set rs = db.OpenRecordset("SELECT " & numFld & ", " & dateFld & " FROM " & tbl & " ORDER BY " & dateFld & ", " & idFld, dbOpenDynaset)
    Do Until rs.EOF
        If Year(rs(dateFld)) <> y Then y = Year(rs(dateFld)): n = 0
        n = n + 1
        rs.Edit
        rs(numFld) = prefix & "-" & y & "-" & Format(n, "0000")
        rs.Update
        rs.MoveNext
    Loop
    rs.Close
End Sub

Private Sub FinishCustomers()
    Dim ci As Long, last As Variant, st As String
    ReassignReps mToday
    For ci = 0 To nCusts - 1
        last = DMax("OrderDate", "Orders", "CustomerID=" & custs(ci).ID & " AND OrderStatus<>'Cancelled'")
        If IsNull(last) Then
            st = IIf(custs(ci).Churn > 0, "Inactive", "Prospect")
        ElseIf custs(ci).Churn > 0 Or last < mToday - 730 Then
            st = "Inactive"
        Else
            st = "Active"
        End If
        db.Execute "UPDATE Customers SET AssignedRepID=" & custs(ci).RepID & ", CustomerStatus='" & st & "' WHERE CustomerID=" & custs(ci).ID, dbFailOnError
    Next
End Sub

Private Sub Summarize()
    Dim rs As DAO.Recordset
    mLog = "Net sales by year (non-cancelled orders):" & vbCrLf
    Set rs = db.OpenRecordset("SELECT Year(OrderDate) AS Yr, Count(*) AS N, Sum(Subtotal) AS Sales FROM qryOrderTotals " & _
                              "WHERE OrderStatus<>'Cancelled' GROUP BY Year(OrderDate) ORDER BY Year(OrderDate)", dbOpenSnapshot)
    Do Until rs.EOF
        mLog = mLog & rs!Yr & "  " & Format(rs!N, "#,##0") & " orders  " & Format(rs!Sales, "$#,##0") & vbCrLf
        rs.MoveNext
    Loop
    rs.Close
    mLog = mLog & vbCrLf & "Rows: customers " & DCount("*", "Customers") & ", contacts " & DCount("*", "Contacts") & _
           ", quotes " & DCount("*", "Quotes") & ", quote lines " & DCount("*", "QuoteLines") & _
           ", orders " & DCount("*", "Orders") & ", order lines " & DCount("*", "OrderLines") & _
           ", payments " & DCount("*", "Payments") & ", activities " & DCount("*", "Activities") & _
           ", phone messages " & DCount("*", "PhoneMessages") & ", sales reps " & DCount("*", "SalesReps") & _
           ", products " & DCount("*", "Products")
End Sub

Private Sub OpenRecordsets()
    Set rsC = db.OpenRecordset("Customers", dbOpenDynaset, dbAppendOnly)
    Set rsCt = db.OpenRecordset("Contacts", dbOpenDynaset, dbAppendOnly)
    Set rsQ = db.OpenRecordset("Quotes", dbOpenDynaset, dbAppendOnly)
    Set rsQL = db.OpenRecordset("QuoteLines", dbOpenDynaset, dbAppendOnly)
    Set rsO = db.OpenRecordset("Orders", dbOpenDynaset, dbAppendOnly)
    Set rsOL = db.OpenRecordset("OrderLines", dbOpenDynaset, dbAppendOnly)
    Set rsP = db.OpenRecordset("Payments", dbOpenDynaset, dbAppendOnly)
    Set rsA = db.OpenRecordset("Activities", dbOpenDynaset, dbAppendOnly)
    Set rsM = db.OpenRecordset("PhoneMessages", dbOpenDynaset, dbAppendOnly)
End Sub

Private Sub CloseRecordsets()
    On Error Resume Next
    rsC.Close: rsCt.Close: rsQ.Close: rsQL.Close: rsO.Close: rsOL.Close: rsP.Close: rsA.Close: rsM.Close
End Sub

Private Sub WriteLog(ByVal fileName As String, ByVal text As String)
    Dim fh As Integer: fh = FreeFile
    Open CurrentProject.Path & "\" & fileName For Output As #fh
    Print #fh, text
    Close #fh
End Sub

'------------------------------------------------------------------------------
'  RANDOM HELPERS
'------------------------------------------------------------------------------
Private Function RB(ByVal lo As Long, ByVal hi As Long) As Long
    RB = lo + Int(Rnd * (hi - lo + 1))
End Function

Private Function Pick(arr As Variant) As Variant
    Pick = arr(LBound(arr) + Int(Rnd * (UBound(arr) - LBound(arr) + 1)))
End Function

Private Function WPick(w As Variant) As Integer
    Dim i As Integer, tot As Double, x As Double
    For i = LBound(w) To UBound(w): tot = tot + w(i): Next
    x = Rnd * tot
    For i = LBound(w) To UBound(w)
        x = x - w(i)
        If x < 0 Then WPick = i: Exit Function
    Next
    WPick = UBound(w)
End Function

Private Function LogNorm(ByVal median As Double, ByVal sigma As Double) As Double
    Dim u As Double
    Do: u = Rnd: Loop While u = 0
    LogNorm = median * Exp(sigma * Sqr(-2 * Log(u)) * Cos(6.28318530718 * Rnd))
End Function

Private Function Phone555() As String
    Phone555 = "555-" & Format(RB(1000, 9999), "0000")
End Function

Private Function RandChars(ByVal n As Integer) As String
    Const CH As String = "0123456789ABCDEFGHJKLMNPRSTUVWXYZ"
    Dim i As Integer
    For i = 1 To n
        RandChars = RandChars & Mid(CH, RB(1, Len(CH)), 1)
    Next
End Function

Private Function BizDay(ByVal d As Date) As Date
    Select Case Weekday(d)
    Case vbSaturday: BizDay = d + 2
    Case vbSunday: BizDay = d + 1
    Case Else: BizDay = d
    End Select
End Function

Private Function PrevBizDay(ByVal d As Date) As Date
    Select Case Weekday(d)
    Case vbSaturday: PrevBizDay = d - 1
    Case vbSunday: PrevBizDay = d - 2
    Case Else: PrevBizDay = d
    End Select
End Function

' Weekday in year y (no later than lastDay), weighted by season
Private Function RandomBizDate(ByVal y As Integer, ByVal lastDay As Date) As Date
    Dim w, m As Integer, maxM As Integer, dd As Integer, d As Date
    w = MonthWeights()
    maxM = IIf(Year(lastDay) = y, Month(lastDay), 12)
    Do
        Do
            m = WPick(w) + 1
        Loop While m > maxM
        If m = maxM And Year(lastDay) = y Then dd = RB(1, Day(lastDay)) Else dd = RB(1, Day(DateSerial(y, m + 1, 0)))
        d = DateSerial(y, m, dd)
    Loop While d > lastDay Or Weekday(d) = vbSaturday Or Weekday(d) = vbSunday
    RandomBizDate = d
End Function

Private Sub SortStage(idx() As Long)
    Dim i As Long
    ReDim idx(0 To IIf(nSt > 0, nSt - 1, 0))
    For i = 0 To nSt - 1: idx(i) = i: Next
    If nSt > 1 Then QSort idx, 0, nSt - 1
End Sub

Private Sub QSort(idx() As Long, ByVal lo As Long, ByVal hi As Long)
    Dim i As Long, j As Long, p As Date, t As Long
    i = lo: j = hi: p = stDate(idx((lo + hi) \ 2))
    Do While i <= j
        Do While stDate(idx(i)) < p: i = i + 1: Loop
        Do While stDate(idx(j)) > p: j = j - 1: Loop
        If i <= j Then t = idx(i): idx(i) = idx(j): idx(j) = t: i = i + 1: j = j - 1
    Loop
    If lo < j Then QSort idx, lo, j
    If i < hi Then QSort idx, i, hi
End Sub
