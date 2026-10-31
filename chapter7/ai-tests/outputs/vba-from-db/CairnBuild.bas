Attribute VB_Name = "CairnBuild"
'==============================================================================
'  CAIRN SYSTEMS - ORDER TRACKING & CRM DATABASE BUILDER
'------------------------------------------------------------------------------
'  HOW TO USE (Microsoft Access on Windows, 2010 or later / Microsoft 365):
'    1. Open Access > Blank database > name it CairnSystems.accdb > Create.
'    2. Close the empty "Table1" that Access opens (don't save it).
'    3. Press Alt+F11 to open the VBA editor.
'    4. File > Import File... > choose this CairnBuild.bas file.
'    5. Click inside "Sub BuildCairnDatabase", press F5.
'    6. Back in Access, press F5 (or close/reopen) to see the new tables & queries.
'       Database Tools > Relationships > "All Relationships" shows the diagram.
'
'  WHAT IT CREATES
'    Tables   : CompanyInfo, SalesReps, Customers, Contacts, Products,
'               Quotes, QuoteLines, Orders, OrderLines, Payments,
'               Activities (CRM notes/calls/meetings), PhoneMessages,
'               Users, RMAs, ManufacturerOrders (AddOperationsTables)
'    Relations: enforced referential integrity; line items cascade-delete
'               with their quote/order. (RMAs has none - see AddOperationsTables.)
'    Queries  : ready-made sources for the Quote, Receipt and Month-End reports,
'               plus CRM views (open phone messages, follow-ups due).
'    Sample data so every query returns rows right away. Run
'               ClearSampleData to wipe it before going live.
'==============================================================================
Option Compare Database
Option Explicit

Private cn As Object   ' ADO connection (supports DEFAULT / ON DELETE CASCADE)

Public Sub BuildCairnDatabase()
    If TableExists("Customers") Then
        MsgBox "This database already has a Customers table. " & _
               "Run this in a new blank database.", vbExclamation, "Cairn Systems"
        Exit Sub
    End If

    Set cn = CurrentProject.Connection
    CreateTables
    CreateIndexes
    DBEngine.Idle dbRefreshCache      ' let DAO see the tables ADO just created
    CurrentDb.TableDefs.Refresh
    SetFieldFormats
    RepairTextDefaults
    LoadSampleData
    CreateQueries
    AddOperationsTables True
    Set cn = Nothing

    Application.RefreshDatabaseWindow
    MsgBox "Cairn Systems database built." & vbCrLf & vbCrLf & _
           "15 tables, relationships, sample data and report queries created.", _
           vbInformation, "Cairn Systems"
End Sub

'------------------------------------------------------------------------------
'  OPERATIONS: sign-in, RMAs, manufacturer orders
'  Added to the file years after the sales tables, and built the way they were
'  built then: plaintext passwords, an RMA table with no relationships, and a
'  manufacturer log. Safe to run on an existing database; existing tables are
'  left alone.
'------------------------------------------------------------------------------
Public Sub AddOperationsTables(Optional ByVal quiet As Boolean = False)
    Dim s As String, db As DAO.Database
    Set cn = CurrentProject.Connection

    If Not TableExists("Users") Then
        s = "CREATE TABLE Users ("
        s = s & "UserID AUTOINCREMENT CONSTRAINT PK_Users PRIMARY KEY, "
        s = s & "UserName TEXT(30) NOT NULL CONSTRAINT UQ_Users_Name UNIQUE, [Password] TEXT(50), "
        s = s & "RepID LONG, IsAdmin YESNO DEFAULT 0, Active YESNO DEFAULT -1, "
        s = s & "CreatedDate DATETIME DEFAULT Date(), LastLogin DATETIME, "
        s = s & "CONSTRAINT FK_Users_Rep FOREIGN KEY (RepID) REFERENCES SalesReps (RepID))"
        x s
        AddUser "mtorres", "maya2022", "Maya", "Torres", False
        AddUser "dliu", "Password1", "Derek", "Liu", False
        AddUser "pshah", "Cairn!2020", "Priya", "Shah", True
        AddUser "reception", "Welcome1", "Sam", "Okafor", False
        x "INSERT INTO Users (UserName, [Password], IsAdmin) VALUES ('admin', 'admin', -1)"
    End If

    ' No relationships to Orders, Products or Customers: nobody set them up
    If Not TableExists("RMAs") Then
        s = "CREATE TABLE RMAs ("
        s = s & "RMAID AUTOINCREMENT CONSTRAINT PK_RMAs PRIMARY KEY, "
        s = s & "RMANumber TEXT(20) NOT NULL CONSTRAINT UQ_RMAs_Number UNIQUE, "
        s = s & "CustomerID LONG, OrderID LONG, ProductID LONG, SerialNumber TEXT(40), "
        s = s & "OpenedDate DATETIME DEFAULT Date(), OpenedByRepID LONG, Reason TEXT(40), ExtendedWarranty YESNO DEFAULT 0, "
        s = s & "Description MEMO, RMAStatus TEXT(20), Disposition TEXT(60), ManufacturerRef TEXT(30), ClosedDate DATETIME)"
        x s
        x "CREATE INDEX IX_RMAs_Opened ON RMAs (OpenedDate)"
    End If

    ' Every call to the manufacturer's order API, as Access saw it
    If Not TableExists("ManufacturerOrders") Then
        s = "CREATE TABLE ManufacturerOrders ("
        s = s & "MfrRequestID AUTOINCREMENT CONSTRAINT PK_ManufacturerOrders PRIMARY KEY, "
        s = s & "OrderID LONG NOT NULL, Attempt INTEGER DEFAULT 1, SentAt DATETIME DEFAULT Now(), SentByUserID LONG, "
        s = s & "RequestBody MEMO, HttpStatus INTEGER, ResponseBody MEMO, ErrorText TEXT(255), ManufacturerRef TEXT(30), "
        s = s & "CONSTRAINT FK_Mfr_Order FOREIGN KEY (OrderID) REFERENCES Orders (OrderID))"
        x s
    End If

    If Not FieldExists("CompanyInfo", "ManufacturerApiUrl") Then
        x "ALTER TABLE CompanyInfo ADD COLUMN ManufacturerApiUrl TEXT(150)"
        x "UPDATE CompanyInfo SET ManufacturerApiUrl = 'http://localhost:8765/orders'"
    End If

    DBEngine.Idle dbRefreshCache
    Set db = CurrentDb
    db.TableDefs.Refresh
    db.TableDefs("RMAs").Fields("RMAStatus").DefaultValue = """Open"""
    SetFmt "ManufacturerOrders", "SentAt", "General Date"
    SetColWidth "ManufacturerOrders", "SentAt", 1.9
    SetColWidth "Users", "LastLogin", 1.9

    ' A few RMAs for the small sample database (the History generator adds its own)
    If DCount("*", "RMAs") = 0 And DCount("*", "Orders") <= 50 Then
        AddRMA "RMA-2026-0001", 2, 2, "CS-TH100", "TH1-7Q4K2M", #9/24/2026#, "Dead on Arrival", "Three sensors in the Dock 4 shipment never joined the network.", "Open"
        AddRMA "RMA-2026-0002", 1, 1, "CS-WL150", "WL1-3HX9PA", #9/30/2026#, "Damaged in Shipping", "Housing cracked; box crushed on arrival.", "Received"
        AddRMA "RMA-2026-0003", 3, Null, "CS-VB220", "VB2-88RT0D", #10/2/2026#, "Failed in Warranty", "Pilot unit from before the order system; no order on file.", "Open"
    End If

    CreateOperationsQueries
    Set cn = Nothing
    Application.RefreshDatabaseWindow
    If Not quiet Then MsgBox "Users, RMAs and manufacturer order tables are ready.", vbInformation, "Cairn Systems"
End Sub

Private Sub CreateOperationsQueries()
    Dim s As String

    ' RMA routing. Grew one IIf at a time; read the branches in order.
    s = "SELECT r.RMAID, r.RMANumber, r.OpenedDate, r.RMAStatus, r.Reason, r.SerialNumber, r.ExtendedWarranty, "
    s = s & "c.CompanyName AS CustomerName, c.TaxExempt, p.SKU, p.Category, o.OrderNumber, "
    s = s & "Nz(o.ShipDate, o.OrderDate) AS InServiceFrom, r.OpenedDate - Nz(o.ShipDate, o.OrderDate) AS DaysInService, "
    s = s & "IIf(r.Reason='Damaged in Shipping','Carrier Claim',"
    s = s & "IIf(r.Reason='Dead on Arrival' And [DaysInService]<=30,'Advance Replacement',"
    s = s & "IIf(p.Category='Service','Account Manager',"
    s = s & "IIf([DaysInService]<=365 Or r.ExtendedWarranty,'Manufacturer Repair',"
    s = s & "IIf(r.Reason='Dead on Arrival','Advance Replacement',"
    s = s & "IIf([DaysInService]<=90,'Bench Test',"
    s = s & "IIf(c.TaxExempt,'Account Manager',"
    s = s & "IIf([DaysInService]>730,'Scrap - Out of Warranty','Customer Pays Repair')))))))) AS Route "
    s = s & "FROM ((RMAs AS r LEFT JOIN Customers AS c ON r.CustomerID = c.CustomerID) "
    s = s & "LEFT JOIN Products AS p ON r.ProductID = p.ProductID) "
    s = s & "LEFT JOIN Orders AS o ON r.OrderID = o.OrderID;"
    q "qryRMARouting", s

    ' Qualified leads: what sales management decided counts, written down once
    s = "SELECT CustomerID, Max(QuoteTotal) AS OpenQuote FROM qryQuoteTotals WHERE QuoteStatus = 'Sent' GROUP BY CustomerID;"
    q "qryOpenQuoteValue", s
    s = "SELECT CustomerID, Count(*) AS Touches FROM Activities WHERE ActivityDate >= Date() - 90 GROUP BY CustomerID;"
    q "qryRecentTouches", s
    s = "SELECT c.CustomerID, c.CompanyName, c.Industry, r.FirstName & ' ' & r.LastName AS RepName, "
    s = s & "Nz(t.Touches, 0) AS RecentTouches, q.OpenQuote "
    s = s & "FROM ((Customers AS c LEFT JOIN SalesReps AS r ON c.AssignedRepID = r.RepID) "
    s = s & "LEFT JOIN qryRecentTouches AS t ON c.CustomerID = t.CustomerID) "
    s = s & "LEFT JOIN qryOpenQuoteValue AS q ON c.CustomerID = q.CustomerID "
    s = s & "WHERE c.CustomerStatus = 'Prospect' "
    s = s & "AND (q.OpenQuote >= 5000 OR (Nz(t.Touches, 0) >= 2 AND c.Industry IN ('Utilities', 'Food & Beverage', 'Healthcare'))) "
    s = s & "AND EXISTS (SELECT 1 FROM Contacts AS k WHERE k.CustomerID = c.CustomerID "
    s = s & "AND (k.Title Like '*Director*' OR k.Title Like '*Manager*')) "
    s = s & "ORDER BY q.OpenQuote DESC, Nz(t.Touches, 0) DESC;"
    q "qryQualifiedLeads", s

    s = "SELECT m.MfrRequestID, o.OrderNumber, m.Attempt, m.SentAt, u.UserName AS SentBy, m.HttpStatus, "
    s = s & "m.ManufacturerRef, m.ErrorText, m.RequestBody, m.ResponseBody, m.OrderID "
    s = s & "FROM (ManufacturerOrders AS m INNER JOIN Orders AS o ON m.OrderID = o.OrderID) "
    s = s & "LEFT JOIN Users AS u ON m.SentByUserID = u.UserID "
    s = s & "ORDER BY m.SentAt DESC;"
    q "qryManufacturerLog", s
    SetColWidth "qryManufacturerLog", "SentAt", 1.9, True
    SetColWidth "qryManufacturerLog", "ErrorText", 2.8, True
    SetColWidth "qryManufacturerLog", "RequestBody", 2.2, True
    SetColWidth "qryManufacturerLog", "ResponseBody", 2.2, True
End Sub

' Datasheet column width saved with a table or query field
Private Sub SetColWidth(ByVal objName As String, ByVal fName As String, ByVal inches As Single, Optional ByVal isQuery As Boolean)
    Dim db As DAO.Database, f As DAO.Field
    Set db = CurrentDb
    If isQuery Then Set f = db.QueryDefs(objName).Fields(fName) Else Set f = db.TableDefs(objName).Fields(fName)
    On Error Resume Next
    f.Properties("ColumnWidth") = CInt(inches * 1440)
    If Err.Number <> 0 Then
        Err.Clear
        f.Properties.Append f.CreateProperty("ColumnWidth", dbInteger, CInt(inches * 1440))
    End If
End Sub

Private Sub AddUser(ByVal uName As String, ByVal pwd As String, ByVal first As String, ByVal last As String, ByVal admin As Boolean)
    x "INSERT INTO Users (UserName, [Password], RepID, IsAdmin) SELECT " & Sq(uName) & ", " & Sq(pwd) & ", RepID, " & _
      IIf(admin, "-1", "0") & " FROM SalesReps WHERE FirstName=" & Sq(first) & " AND LastName=" & Sq(last)
End Sub

Private Sub AddRMA(ByVal num As String, ByVal custID As Long, ByVal orderID As Variant, ByVal sku As String, ByVal serial As String, _
                   ByVal opened As Date, ByVal reason As String, ByVal descr As String, ByVal status As String)
    x "INSERT INTO RMAs (RMANumber, CustomerID, OrderID, ProductID, SerialNumber, OpenedDate, Reason, Description, RMAStatus) " & _
      "SELECT " & Sq(num) & ", " & custID & ", " & nm(orderID) & ", ProductID, " & Sq(serial) & ", " & Dt(opened) & ", " & _
      Sq(reason) & ", " & Sq(descr) & ", " & Sq(status) & " FROM Products WHERE SKU=" & Sq(sku)
End Sub

Private Function FieldExists(ByVal tName As String, ByVal fName As String) As Boolean
    Dim db As DAO.Database, f As DAO.Field
    Set db = CurrentDb                ' keep a reference, or the loop's object goes away (error 3420)
    For Each f In db.TableDefs(tName).Fields
        If f.Name = fName Then FieldExists = True: Exit Function
    Next
End Function

'------------------------------------------------------------------------------
'  TABLES
'------------------------------------------------------------------------------
Private Sub CreateTables()
    Dim s As String

    ' Company letterhead info for quotes/receipts (single row)
    s = "CREATE TABLE CompanyInfo ("
    s = s & "CompanyInfoID AUTOINCREMENT CONSTRAINT PK_CompanyInfo PRIMARY KEY, "
    s = s & "CompanyName TEXT(100) NOT NULL, Address1 TEXT(100), Address2 TEXT(100), "
    s = s & "City TEXT(50), State TEXT(20), Zip TEXT(15), Country TEXT(50), "
    s = s & "Phone TEXT(30), Email TEXT(100), Website TEXT(100), "
    s = s & "DefaultTaxRate DOUBLE DEFAULT 0, QuoteTerms MEMO, ReceiptFooter MEMO)"
    x s

    s = "CREATE TABLE SalesReps ("
    s = s & "RepID AUTOINCREMENT CONSTRAINT PK_SalesReps PRIMARY KEY, "
    s = s & "FirstName TEXT(50) NOT NULL, LastName TEXT(50) NOT NULL, Title TEXT(50), "
    s = s & "Email TEXT(100), Phone TEXT(30), Extension TEXT(10), "
    s = s & "CommissionRate DOUBLE DEFAULT 0.05, HireDate DATETIME, "
    s = s & "Active YESNO DEFAULT -1)"
    x s

    s = "CREATE TABLE Customers ("
    s = s & "CustomerID AUTOINCREMENT CONSTRAINT PK_Customers PRIMARY KEY, "
    s = s & "CompanyName TEXT(100) NOT NULL, Industry TEXT(50), "
    s = s & "Phone TEXT(30), Email TEXT(100), Website TEXT(100), "
    s = s & "BillAddress TEXT(150), BillCity TEXT(50), BillState TEXT(20), BillZip TEXT(15), BillCountry TEXT(50) DEFAULT 'USA', "
    s = s & "ShipAddress TEXT(150), ShipCity TEXT(50), ShipState TEXT(20), ShipZip TEXT(15), ShipCountry TEXT(50) DEFAULT 'USA', "
    s = s & "AssignedRepID LONG, PaymentTerms TEXT(30) DEFAULT 'Net 30', TaxExempt YESNO DEFAULT 0, "
    s = s & "CustomerStatus TEXT(20) DEFAULT 'Prospect', CustomerSince DATETIME DEFAULT Date(), Notes MEMO, "
    s = s & "CONSTRAINT FK_Customers_Rep FOREIGN KEY (AssignedRepID) REFERENCES SalesReps (RepID))"
    x s

    s = "CREATE TABLE Contacts ("
    s = s & "ContactID AUTOINCREMENT CONSTRAINT PK_Contacts PRIMARY KEY, "
    s = s & "CustomerID LONG NOT NULL, FirstName TEXT(50) NOT NULL, LastName TEXT(50), Title TEXT(80), "
    s = s & "Email TEXT(100), Phone TEXT(30), Mobile TEXT(30), IsPrimary YESNO DEFAULT 0, Notes MEMO, "
    s = s & "CONSTRAINT FK_Contacts_Customer FOREIGN KEY (CustomerID) REFERENCES Customers (CustomerID) ON DELETE CASCADE)"
    x s

    s = "CREATE TABLE Products ("
    s = s & "ProductID AUTOINCREMENT CONSTRAINT PK_Products PRIMARY KEY, "
    s = s & "SKU TEXT(30) NOT NULL CONSTRAINT UQ_Products_SKU UNIQUE, ProductName TEXT(100) NOT NULL, "
    s = s & "Category TEXT(30), Description MEMO, UnitPrice CURRENCY DEFAULT 0, UnitCost CURRENCY DEFAULT 0, "
    s = s & "FirmwareVersion TEXT(20), LeadTimeDays INTEGER, Active YESNO DEFAULT -1)"
    x s

    s = "CREATE TABLE Quotes ("
    s = s & "QuoteID AUTOINCREMENT CONSTRAINT PK_Quotes PRIMARY KEY, "
    s = s & "QuoteNumber TEXT(20) NOT NULL CONSTRAINT UQ_Quotes_Number UNIQUE, "
    s = s & "CustomerID LONG NOT NULL, ContactID LONG, RepID LONG NOT NULL, "
    s = s & "QuoteDate DATETIME DEFAULT Date(), ValidUntil DATETIME, QuoteStatus TEXT(20) DEFAULT 'Draft', "
    s = s & "TaxRate DOUBLE DEFAULT 0, ShippingCost CURRENCY DEFAULT 0, Terms MEMO, Notes MEMO, "
    s = s & "CONSTRAINT FK_Quotes_Customer FOREIGN KEY (CustomerID) REFERENCES Customers (CustomerID), "
    s = s & "CONSTRAINT FK_Quotes_Contact FOREIGN KEY (ContactID) REFERENCES Contacts (ContactID), "
    s = s & "CONSTRAINT FK_Quotes_Rep FOREIGN KEY (RepID) REFERENCES SalesReps (RepID))"
    x s

    s = "CREATE TABLE QuoteLines ("
    s = s & "QuoteLineID AUTOINCREMENT CONSTRAINT PK_QuoteLines PRIMARY KEY, "
    s = s & "QuoteID LONG NOT NULL, ProductID LONG NOT NULL, LineDescription TEXT(255), "
    s = s & "Quantity LONG DEFAULT 1, UnitPrice CURRENCY DEFAULT 0, DiscountPct DOUBLE DEFAULT 0, "
    s = s & "CONSTRAINT FK_QuoteLines_Quote FOREIGN KEY (QuoteID) REFERENCES Quotes (QuoteID) ON DELETE CASCADE, "
    s = s & "CONSTRAINT FK_QuoteLines_Product FOREIGN KEY (ProductID) REFERENCES Products (ProductID))"
    x s

    s = "CREATE TABLE Orders ("
    s = s & "OrderID AUTOINCREMENT CONSTRAINT PK_Orders PRIMARY KEY, "
    s = s & "OrderNumber TEXT(20) NOT NULL CONSTRAINT UQ_Orders_Number UNIQUE, "
    s = s & "CustomerID LONG NOT NULL, ContactID LONG, RepID LONG NOT NULL, QuoteID LONG, "
    s = s & "CustomerPO TEXT(50), OrderDate DATETIME DEFAULT Date(), RequiredDate DATETIME, ShipDate DATETIME, "
    s = s & "OrderStatus TEXT(20) DEFAULT 'Open', TaxRate DOUBLE DEFAULT 0, ShippingCost CURRENCY DEFAULT 0, "
    s = s & "ShipVia TEXT(50), TrackingNumber TEXT(60), Notes MEMO, "
    s = s & "CONSTRAINT FK_Orders_Customer FOREIGN KEY (CustomerID) REFERENCES Customers (CustomerID), "
    s = s & "CONSTRAINT FK_Orders_Contact FOREIGN KEY (ContactID) REFERENCES Contacts (ContactID), "
    s = s & "CONSTRAINT FK_Orders_Rep FOREIGN KEY (RepID) REFERENCES SalesReps (RepID), "
    s = s & "CONSTRAINT FK_Orders_Quote FOREIGN KEY (QuoteID) REFERENCES Quotes (QuoteID))"
    x s

    s = "CREATE TABLE OrderLines ("
    s = s & "OrderLineID AUTOINCREMENT CONSTRAINT PK_OrderLines PRIMARY KEY, "
    s = s & "OrderID LONG NOT NULL, ProductID LONG NOT NULL, LineDescription TEXT(255), "
    s = s & "Quantity LONG DEFAULT 1, UnitPrice CURRENCY DEFAULT 0, DiscountPct DOUBLE DEFAULT 0, "
    s = s & "SerialNumbers MEMO, "
    s = s & "CONSTRAINT FK_OrderLines_Order FOREIGN KEY (OrderID) REFERENCES Orders (OrderID) ON DELETE CASCADE, "
    s = s & "CONSTRAINT FK_OrderLines_Product FOREIGN KEY (ProductID) REFERENCES Products (ProductID))"
    x s

    s = "CREATE TABLE Payments ("
    s = s & "PaymentID AUTOINCREMENT CONSTRAINT PK_Payments PRIMARY KEY, "
    s = s & "OrderID LONG NOT NULL, PaymentDate DATETIME DEFAULT Date(), Amount CURRENCY NOT NULL, "
    s = s & "PaymentMethod TEXT(20), PaymentRef TEXT(50), ReceivedByRepID LONG, Notes MEMO, "
    s = s & "CONSTRAINT FK_Payments_Order FOREIGN KEY (OrderID) REFERENCES Orders (OrderID), "
    s = s & "CONSTRAINT FK_Payments_Rep FOREIGN KEY (ReceivedByRepID) REFERENCES SalesReps (RepID))"
    x s

    ' CRM: notes, calls, emails, meetings logged against a customer
    s = "CREATE TABLE Activities ("
    s = s & "ActivityID AUTOINCREMENT CONSTRAINT PK_Activities PRIMARY KEY, "
    s = s & "CustomerID LONG NOT NULL, ContactID LONG, RepID LONG NOT NULL, "
    s = s & "ActivityDate DATETIME DEFAULT Now(), ActivityType TEXT(20) DEFAULT 'Note', "
    s = s & "Subject TEXT(150), Details MEMO, FollowUpDate DATETIME, Completed YESNO DEFAULT 0, "
    s = s & "CONSTRAINT FK_Activities_Customer FOREIGN KEY (CustomerID) REFERENCES Customers (CustomerID) ON DELETE CASCADE, "
    s = s & "CONSTRAINT FK_Activities_Contact FOREIGN KEY (ContactID) REFERENCES Contacts (ContactID), "
    s = s & "CONSTRAINT FK_Activities_Rep FOREIGN KEY (RepID) REFERENCES SalesReps (RepID))"
    x s

    ' CRM: "While you were out" messages taken on behalf of a rep
    s = "CREATE TABLE PhoneMessages ("
    s = s & "MessageID AUTOINCREMENT CONSTRAINT PK_PhoneMessages PRIMARY KEY, "
    s = s & "ReceivedAt DATETIME DEFAULT Now(), ForRepID LONG NOT NULL, TakenByRepID LONG, "
    s = s & "CustomerID LONG, CallerName TEXT(100) NOT NULL, CallerCompany TEXT(100), CallerPhone TEXT(30), "
    s = s & "Message MEMO, Urgent YESNO DEFAULT 0, PleaseCallBack YESNO DEFAULT -1, WillCallAgain YESNO DEFAULT 0, "
    s = s & "MessageStatus TEXT(20) DEFAULT 'New', ReturnedAt DATETIME, ReturnNotes MEMO, "
    s = s & "CONSTRAINT FK_Msg_ForRep FOREIGN KEY (ForRepID) REFERENCES SalesReps (RepID), "
    s = s & "CONSTRAINT FK_Msg_TakenBy FOREIGN KEY (TakenByRepID) REFERENCES SalesReps (RepID), "
    s = s & "CONSTRAINT FK_Msg_Customer FOREIGN KEY (CustomerID) REFERENCES Customers (CustomerID))"
    x s
End Sub

Private Sub CreateIndexes()
    x "CREATE INDEX IX_Customers_Name ON Customers (CompanyName)"
    x "CREATE INDEX IX_Quotes_Date ON Quotes (QuoteDate)"
    x "CREATE INDEX IX_Orders_Date ON Orders (OrderDate)"
    x "CREATE INDEX IX_Orders_Status ON Orders (OrderStatus)"
    x "CREATE INDEX IX_Payments_Date ON Payments (PaymentDate)"
    x "CREATE INDEX IX_Activities_Date ON Activities (ActivityDate)"
    x "CREATE INDEX IX_Activities_FollowUp ON Activities (FollowUpDate)"
    x "CREATE INDEX IX_Msg_Status ON PhoneMessages (MessageStatus)"
End Sub

' ADO stores DEFAULT 'x' with the single quotes, and Access forms then save the quotes as part of the value.
' Re-set text defaults in Access's own "x" form and clean any rows already saved with quotes. Safe to run again.
Public Sub RepairTextDefaults()
    Dim db As DAO.Database, p, f
    Do While Forms.Count > 0               ' open forms lock their tables
        DoCmd.Close acForm, Forms(0).Name
    Loop
    Set db = CurrentDb
    For Each p In Array("Customers|BillCountry|USA", "Customers|ShipCountry|USA", "Customers|PaymentTerms|Net 30", _
                        "Customers|CustomerStatus|Prospect", "Quotes|QuoteStatus|Draft", "Orders|OrderStatus|Open", _
                        "Activities|ActivityType|Note", "PhoneMessages|MessageStatus|New")
        f = Split(p, "|")
        db.TableDefs(f(0)).Fields(f(1)).DefaultValue = """" & f(2) & """"
        db.Execute "UPDATE " & f(0) & " SET " & f(1) & " = Mid(" & f(1) & ", 2, Len(" & f(1) & ") - 2) WHERE " & f(1) & " Like ""'*'""", dbFailOnError
    Next
End Sub

Private Sub SetFieldFormats()
    SetFmt "CompanyInfo", "DefaultTaxRate", "Percent"
    SetFmt "SalesReps", "CommissionRate", "Percent"
    SetFmt "Quotes", "TaxRate", "Percent"
    SetFmt "Orders", "TaxRate", "Percent"
    SetFmt "QuoteLines", "DiscountPct", "Percent"
    SetFmt "OrderLines", "DiscountPct", "Percent"
    SetFmt "Activities", "ActivityDate", "General Date"
    SetFmt "PhoneMessages", "ReceivedAt", "General Date"
    SetFmt "PhoneMessages", "ReturnedAt", "General Date"
End Sub

'------------------------------------------------------------------------------
'  SAMPLE DATA  (fictional - replace with your own)
'------------------------------------------------------------------------------
Private Sub LoadSampleData()
    x "INSERT INTO CompanyInfo (CompanyName, Address1, City, State, Zip, Country, Phone, Email, Website, DefaultTaxRate, QuoteTerms, ReceiptFooter) " & _
      "VALUES ('Cairn Systems', '[Street Address]', '[City]', '[ST]', '[Zip]', 'USA', '[Phone]', 'sales@cairnsystems.example', 'www.cairnsystems.example', 0.0825, " & _
      "'Prices valid 30 days from quote date. Net 30 on approved credit. Standard lead time 3-4 weeks ARO. Firmware updates included for 12 months.', " & _
      "'Thank you for your business! Questions about this receipt? Contact your account representative.')"

    ' Sales reps (RepID 1-4)
    x "INSERT INTO SalesReps (FirstName, LastName, Title, Email, Phone, Extension, CommissionRate, HireDate) VALUES ('Maya','Torres','Account Executive','maya.torres@cairnsystems.example','555-0101','101',0.06,#2022-03-14#)"
    x "INSERT INTO SalesReps (FirstName, LastName, Title, Email, Phone, Extension, CommissionRate, HireDate) VALUES ('Derek','Liu','Account Executive','derek.liu@cairnsystems.example','555-0102','102',0.06,#2023-06-05#)"
    x "INSERT INTO SalesReps (FirstName, LastName, Title, Email, Phone, Extension, CommissionRate, HireDate) VALUES ('Priya','Shah','Sales Manager','priya.shah@cairnsystems.example','555-0100','100',0.04,#2020-09-01#)"
    x "INSERT INTO SalesReps (FirstName, LastName, Title, Email, Phone, Extension, CommissionRate, HireDate) VALUES ('Sam','Okafor','Inside Sales / Reception','sam.okafor@cairnsystems.example','555-0110','110',0.03,#2024-01-08#)"

    ' Customers (CustomerID 1-5)
    x "INSERT INTO Customers (CompanyName, Industry, Phone, Email, BillAddress, BillCity, BillState, BillZip, ShipAddress, ShipCity, ShipState, ShipZip, AssignedRepID, PaymentTerms, TaxExempt, CustomerStatus, CustomerSince, Notes) " & _
      "VALUES ('Northfield Water District','Utilities','555-2100','ap@northfieldwater.example','400 Reservoir Rd','Northfield','CA','95001','400 Reservoir Rd','Northfield','CA','95001',1,'Net 45',-1,'Active',#2024-05-10#,'Tax-exempt certificate on file. Monitoring 3 reservoirs + lift stations.')"
    x "INSERT INTO Customers (CompanyName, Industry, Phone, Email, BillAddress, BillCity, BillState, BillZip, ShipAddress, ShipCity, ShipState, ShipZip, AssignedRepID, PaymentTerms, TaxExempt, CustomerStatus, CustomerSince, Notes) " & _
      "VALUES ('Bluepine Cold Storage','Logistics','555-3300','purchasing@bluepine.example','88 Freezer Ln','Stockton','CA','95206','Dock 4, 88 Freezer Ln','Stockton','CA','95206',2,'Net 30',0,'Active',#2025-02-18#,'Cold-chain compliance; needs temp logs exported weekly.')"
    x "INSERT INTO Customers (CompanyName, Industry, Phone, Email, BillAddress, BillCity, BillState, BillZip, ShipAddress, ShipCity, ShipState, ShipZip, AssignedRepID, PaymentTerms, TaxExempt, CustomerStatus, CustomerSince, Notes) " & _
      "VALUES ('Harlow Manufacturing','Industrial','555-4410','ops@harlowmfg.example','2250 Foundry Ave','Fresno','CA','93721','2250 Foundry Ave','Fresno','CA','93721',1,'Net 30',0,'Active',#2025-11-03#,'Predictive maintenance pilot on CNC line.')"
    x "INSERT INTO Customers (CompanyName, Industry, Phone, Email, BillAddress, BillCity, BillState, BillZip, ShipAddress, ShipCity, ShipState, ShipZip, AssignedRepID, PaymentTerms, TaxExempt, CustomerStatus, CustomerSince, Notes) " & _
      "VALUES ('Mesa Ridge Vineyards','Agriculture','555-5120','office@mesaridge.example','17 Vista Hill Rd','Paso Robles','CA','93446','17 Vista Hill Rd','Paso Robles','CA','93446',2,'Net 30',0,'Prospect',#2026-09-01#,'Soil moisture + frost alerts. Met at AgTech expo.')"
    x "INSERT INTO Customers (CompanyName, Industry, Phone, Email, BillAddress, BillCity, BillState, BillZip, ShipAddress, ShipCity, ShipState, ShipZip, AssignedRepID, PaymentTerms, TaxExempt, CustomerStatus, CustomerSince, Notes) " & _
      "VALUES ('Granite Peak Facilities','Building Management','555-6230','fm@granitepeak.example','900 Summit Blvd','Sacramento','CA','95814','900 Summit Blvd','Sacramento','CA','95814',3,'Net 30',0,'Active',#2026-07-22#,'HVAC/air quality across 4 office buildings.')"

    ' Contacts (ContactID 1-6)
    x "INSERT INTO Contacts (CustomerID, FirstName, LastName, Title, Email, Phone, IsPrimary) VALUES (1,'Elena','Brooks','Operations Superintendent','ebrooks@northfieldwater.example','555-2101',-1)"
    x "INSERT INTO Contacts (CustomerID, FirstName, LastName, Title, Email, Phone, IsPrimary) VALUES (2,'Marcus','Webb','Facilities Director','mwebb@bluepine.example','555-3301',-1)"
    x "INSERT INTO Contacts (CustomerID, FirstName, LastName, Title, Email, Phone, IsPrimary) VALUES (3,'Hannah','Cole','Maintenance Manager','hcole@harlowmfg.example','555-4411',-1)"
    x "INSERT INTO Contacts (CustomerID, FirstName, LastName, Title, Email, Phone, IsPrimary) VALUES (4,'Luis','Ortega','Vineyard Manager','luis@mesaridge.example','555-5121',-1)"
    x "INSERT INTO Contacts (CustomerID, FirstName, LastName, Title, Email, Phone, IsPrimary) VALUES (5,'Grace','Kim','Facilities Manager','gkim@granitepeak.example','555-6231',-1)"
    x "INSERT INTO Contacts (CustomerID, FirstName, LastName, Title, Email, Phone, IsPrimary) VALUES (3,'Tom','Reyes','Purchasing Agent','treyes@harlowmfg.example','555-4415',0)"

    ' Products (ProductID 1-10)
    AddProduct "CS-TH100", "Temperature/Humidity Sensor (LoRaWAN)", "Sensor", 89, 34.5, "2.4.1", 21
    AddProduct "CS-VB220", "Vibration & Condition Monitoring Sensor", "Sensor", 249, 98, "1.9.0", 28
    AddProduct "CS-PR310", "Pressure Transducer Node", "Sensor", 179, 71, "3.1.2", 21
    AddProduct "CS-WL150", "Ultrasonic Water Level Sensor", "Sensor", 199, 77, "2.0.5", 21
    AddProduct "CS-GW800", "LoRaWAN Gateway, 8-Channel, Outdoor", "Gateway", 649, 290, "5.2.0", 14
    AddProduct "CS-NB420", "NB-IoT / LTE-M Cellular Module", "Module", 59, 22, "1.3.3", 30
    AddProduct "CS-MK01", "Universal Mounting Kit", "Accessory", 19, 5, "", 7
    AddProduct "CS-CLD-12", "Cairn Cloud Subscription - 12 months per device", "Service", 36, 6, "", 0
    AddProduct "CS-SVC-INST", "On-site Installation & Commissioning (per day)", "Service", 1200, 600, "", 10
    AddProduct "CS-DEVKIT", "Developer Kit (sensor, gateway, SDK)", "Kit", 399, 160, "5.2.0", 7

    ' Quotes (QuoteID 1-4)
    AddQuote "Q-2026-0101", 1, 1, 1, #8/18/2026#, #9/17/2026#, "Accepted", 0, 150
    AddQuote "Q-2026-0102", 2, 2, 2, #9/2/2026#, #10/2/2026#, "Accepted", 0.0825, 75
    AddQuote "Q-2026-0103", 4, 4, 2, #9/22/2026#, #10/22/2026#, "Sent", 0.0775, 40
    AddQuote "Q-2026-0104", 3, 3, 1, #9/25/2026#, #10/25/2026#, "Draft", 0.0825, 0

    AddLine "QuoteLines", "QuoteID", 1, 4, 40, 199, 0.1
    AddLine "QuoteLines", "QuoteID", 1, 5, 2, 649, 0
    AddLine "QuoteLines", "QuoteID", 1, 8, 42, 36, 0
    AddLine "QuoteLines", "QuoteID", 1, 9, 2, 1200, 0
    AddLine "QuoteLines", "QuoteID", 2, 1, 120, 89, 0.05
    AddLine "QuoteLines", "QuoteID", 2, 5, 3, 649, 0
    AddLine "QuoteLines", "QuoteID", 2, 7, 120, 19, 0
    AddLine "QuoteLines", "QuoteID", 3, 1, 30, 89, 0
    AddLine "QuoteLines", "QuoteID", 3, 4, 6, 199, 0
    AddLine "QuoteLines", "QuoteID", 3, 5, 1, 649, 0
    AddLine "QuoteLines", "QuoteID", 3, 10, 1, 399, 0
    AddLine "QuoteLines", "QuoteID", 4, 2, 60, 249, 0.08
    AddLine "QuoteLines", "QuoteID", 4, 5, 2, 649, 0
    AddLine "QuoteLines", "QuoteID", 4, 8, 62, 36, 0

    ' Orders (OrderID 1-4)
    x "INSERT INTO Orders (OrderNumber, CustomerID, ContactID, RepID, QuoteID, CustomerPO, OrderDate, RequiredDate, ShipDate, OrderStatus, TaxRate, ShippingCost, ShipVia, TrackingNumber) " & _
      "VALUES ('SO-2026-0201',1,1,1,1,'NWD-44817',#2026-08-25#,#2026-09-15#,#2026-09-08#,'Invoiced',0,150,'UPS Ground','1Z999AA10000000001')"
    x "INSERT INTO Orders (OrderNumber, CustomerID, ContactID, RepID, QuoteID, CustomerPO, OrderDate, RequiredDate, ShipDate, OrderStatus, TaxRate, ShippingCost, ShipVia, TrackingNumber) " & _
      "VALUES ('SO-2026-0202',2,2,2,2,'BP-7781',#2026-09-09#,#2026-09-30#,#2026-09-21#,'Shipped',0.0825,75,'FedEx Freight','FX0000000002')"
    x "INSERT INTO Orders (OrderNumber, CustomerID, ContactID, RepID, CustomerPO, OrderDate, RequiredDate, OrderStatus, TaxRate, ShippingCost) " & _
      "VALUES ('SO-2026-0203',5,5,3,'GPF-1029',#2026-09-15#,#2026-10-10#,'In Production',0.0825,60)"
    x "INSERT INTO Orders (OrderNumber, CustomerID, ContactID, RepID, OrderDate, RequiredDate, OrderStatus, TaxRate, ShippingCost) " & _
      "VALUES ('SO-2026-0204',3,6,1,#2026-09-28#,#2026-10-12#,'Open',0.0825,25)"

    AddLine "OrderLines", "OrderID", 1, 4, 40, 199, 0.1
    AddLine "OrderLines", "OrderID", 1, 5, 2, 649, 0
    AddLine "OrderLines", "OrderID", 1, 8, 42, 36, 0
    AddLine "OrderLines", "OrderID", 1, 9, 2, 1200, 0
    AddLine "OrderLines", "OrderID", 2, 1, 120, 89, 0.05
    AddLine "OrderLines", "OrderID", 2, 5, 3, 649, 0
    AddLine "OrderLines", "OrderID", 2, 7, 120, 19, 0
    AddLine "OrderLines", "OrderID", 3, 1, 50, 89, 0
    AddLine "OrderLines", "OrderID", 3, 5, 1, 649, 0
    AddLine "OrderLines", "OrderID", 3, 6, 10, 59, 0
    AddLine "OrderLines", "OrderID", 4, 10, 2, 399, 0
    AddLine "OrderLines", "OrderID", 4, 2, 4, 249, 0

    ' Payments
    x "INSERT INTO Payments (OrderID, PaymentDate, Amount, PaymentMethod, PaymentRef, ReceivedByRepID, Notes) VALUES (1,#2026-08-26#,6262,'Check','CHK 20931',4,'50% deposit')"
    x "INSERT INTO Payments (OrderID, PaymentDate, Amount, PaymentMethod, PaymentRef, ReceivedByRepID, Notes) VALUES (1,#2026-09-24#,6262,'ACH','ACH-883120',4,'Final payment')"
    x "INSERT INTO Payments (OrderID, PaymentDate, Amount, PaymentMethod, PaymentRef, ReceivedByRepID, Notes) VALUES (2,#2026-09-12#,7000,'Wire','WIRE-55102',2,'Deposit')"
    x "INSERT INTO Payments (OrderID, PaymentDate, Amount, PaymentMethod, PaymentRef, ReceivedByRepID, Notes) VALUES (3,#2026-09-15#,3109.17,'Credit Card','VISA x4421',3,'50% deposit')"

    ' CRM activities
    AddActivity 1, 1, 1, #8/12/2026 10:30:00 AM#, "Meeting", "Site walk at Reservoir 2", "Walked lift stations with Elena. Needs 40 level sensors + 2 gateways. Wants install crew.", Null, True
    AddActivity 2, 2, 2, #9/1/2026 2:15:00 PM#, "Call", "Expansion to Dock 4", "Marcus wants 120 more temp sensors for new freezer wing. Quote by Tuesday.", Null, True
    AddActivity 4, 4, 2, #9/20/2026 9:00:00 AM#, "Meeting", "AgTech expo follow-up", "Interested in frost alerts. Sending starter quote + dev kit.", #10/6/2026#, False
    AddActivity 3, 3, 1, #9/24/2026 11:00:00 AM#, "Call", "Vibration pilot results", "Pilot caught bearing fault early. Hannah pushing for full CNC line (60 units).", #10/2/2026#, False
    AddActivity 5, 5, 3, #9/15/2026 4:00:00 PM#, "Email", "Order confirmation", "Confirmed SO-2026-0203, target ship 10/8.", Null, True
    AddActivity 2, 2, 2, #9/29/2026 1:00:00 PM#, "Note", "Balance due", "Remaining balance on SO-0202 due Net 30 from ship date (10/21).", #10/21/2026#, False

    ' Phone messages taken on behalf of reps
    AddMsg #10/1/2026 9:12:00 AM#, 1, 4, 3, "Hannah Cole", "Harlow Manufacturing", "555-4411", "Wants to discuss volume pricing on Q-2026-0104 before Friday.", True, True, "New"
    AddMsg #10/1/2026 3:40:00 PM#, 2, 4, 4, "Luis Ortega", "Mesa Ridge Vineyards", "555-5121", "Got the quote. Question about battery life in cold weather.", False, True, "New"
    AddMsg #9/30/2026 11:05:00 AM#, 3, 4, 5, "Grace Kim", "Granite Peak Facilities", "555-6231", "Checking ship date on SO-2026-0203.", False, True, "Returned"
    AddMsg #10/2/2026 10:20:00 AM#, 1, 2, Null, "Jordan Pike", "Lakeside Brewing Co.", "555-7788", "Referral from Northfield. Interested in tank temperature monitoring.", False, True, "New"
    x "UPDATE PhoneMessages SET ReturnedAt=#2026-09-30 13:30#, ReturnNotes='Confirmed 10/8 ship date.' WHERE MessageStatus='Returned'"
End Sub

'------------------------------------------------------------------------------
'  SAVED QUERIES  (use these as Record Sources for your reports)
'------------------------------------------------------------------------------
' Re-create just the saved queries (safe to run again; existing ones are replaced)
Public Sub RebuildQueries(Optional ByVal quiet As Boolean = False)
    CreateQueries
    If TableExists("RMAs") Then CreateOperationsQueries
    Application.RefreshDatabaseWindow
    If Not quiet Then MsgBox "Queries rebuilt.", vbInformation, "Cairn Systems"
End Sub

Private Sub CreateQueries()
    Dim s As String

    ' ---- Quote calculations ----
    s = "SELECT ql.QuoteLineID, ql.QuoteID, ql.ProductID, p.SKU, Nz(ql.LineDescription, p.ProductName) AS ItemDescription, "
    s = s & "ql.Quantity, ql.UnitPrice, ql.DiscountPct, Round(ql.Quantity*ql.UnitPrice*(1-ql.DiscountPct),2) AS LineTotal "
    s = s & "FROM QuoteLines AS ql INNER JOIN Products AS p ON ql.ProductID = p.ProductID;"
    q "qryQuoteLines", s

    s = "SELECT q.QuoteID, q.QuoteNumber, q.CustomerID, q.RepID, q.QuoteDate, q.QuoteStatus, "
    s = s & "CCur(Nz(Sum(l.LineTotal),0)) AS Subtotal, q.TaxRate, CCur(Round(Nz(Sum(l.LineTotal),0)*q.TaxRate,2)) AS TaxAmount, q.ShippingCost, "
    s = s & "CCur(Nz(Sum(l.LineTotal),0) + Round(Nz(Sum(l.LineTotal),0)*q.TaxRate,2) + q.ShippingCost) AS QuoteTotal "
    s = s & "FROM Quotes AS q LEFT JOIN qryQuoteLines AS l ON q.QuoteID = l.QuoteID "
    s = s & "GROUP BY q.QuoteID, q.QuoteNumber, q.CustomerID, q.RepID, q.QuoteDate, q.QuoteStatus, q.TaxRate, q.ShippingCost;"
    q "qryQuoteTotals", s

    ' Quote header + lines (no CompanyInfo here: Access rejects a comma join
    ' mixed with outer joins, so the seller details are added in a second step)
    s = "SELECT q.QuoteID, q.QuoteNumber, q.QuoteDate, q.ValidUntil, q.QuoteStatus, q.Terms, q.Notes, "
    s = s & "c.CompanyName AS CustomerName, c.BillAddress, c.BillCity, c.BillState, c.BillZip, c.BillCountry, "
    s = s & "ct.FirstName & ' ' & ct.LastName AS ContactName, ct.Email AS ContactEmail, ct.Phone AS ContactPhone, "
    s = s & "r.FirstName & ' ' & r.LastName AS RepName, r.Email AS RepEmail, r.Phone AS RepPhone, "
    s = s & "l.QuoteLineID, l.SKU, l.ItemDescription, l.Quantity, l.UnitPrice, l.DiscountPct, l.LineTotal, "
    s = s & "t.Subtotal, t.TaxRate, t.TaxAmount, t.ShippingCost, t.QuoteTotal "
    s = s & "FROM ((((Quotes AS q INNER JOIN Customers AS c ON q.CustomerID = c.CustomerID) "
    s = s & "INNER JOIN SalesReps AS r ON q.RepID = r.RepID) "
    s = s & "INNER JOIN qryQuoteTotals AS t ON q.QuoteID = t.QuoteID) "
    s = s & "LEFT JOIN Contacts AS ct ON q.ContactID = ct.ContactID) "
    s = s & "LEFT JOIN qryQuoteLines AS l ON q.QuoteID = l.QuoteID;"
    q "qryQuotePrintBase", s

    ' Source for the printable QUOTE report (group on QuoteNumber)
    s = "SELECT b.*, Nz(b.Terms, ci.QuoteTerms) AS QuoteTerms, "
    s = s & "ci.CompanyName AS SellerName, ci.Address1 AS SellerAddress, ci.City & ', ' & ci.State & ' ' & ci.Zip AS SellerCityStateZip, "
    s = s & "ci.Phone AS SellerPhone, ci.Email AS SellerEmail, ci.Website AS SellerWebsite "
    s = s & "FROM qryQuotePrintBase AS b, CompanyInfo AS ci;"
    q "qryQuotePrint", s

    ' ---- Order calculations ----
    s = "SELECT ol.OrderLineID, ol.OrderID, ol.ProductID, p.SKU, p.Category, Nz(ol.LineDescription, p.ProductName) AS ItemDescription, "
    s = s & "ol.Quantity, ol.UnitPrice, ol.DiscountPct, Round(ol.Quantity*ol.UnitPrice*(1-ol.DiscountPct),2) AS LineTotal, "
    s = s & "ol.Quantity*p.UnitCost AS LineCost, ol.SerialNumbers "
    s = s & "FROM OrderLines AS ol INNER JOIN Products AS p ON ol.ProductID = p.ProductID;"
    q "qryOrderLines", s

    s = "SELECT o.OrderID, o.OrderNumber, o.CustomerID, o.RepID, o.OrderDate, o.OrderStatus, "
    s = s & "CCur(Nz(Sum(l.LineTotal),0)) AS Subtotal, o.TaxRate, CCur(Round(Nz(Sum(l.LineTotal),0)*o.TaxRate,2)) AS TaxAmount, o.ShippingCost, "
    s = s & "CCur(Nz(Sum(l.LineTotal),0) + Round(Nz(Sum(l.LineTotal),0)*o.TaxRate,2) + o.ShippingCost) AS OrderTotal, "
    s = s & "CCur(Nz(Sum(l.LineCost),0)) AS ProductCost "
    s = s & "FROM Orders AS o LEFT JOIN qryOrderLines AS l ON o.OrderID = l.OrderID "
    s = s & "GROUP BY o.OrderID, o.OrderNumber, o.CustomerID, o.RepID, o.OrderDate, o.OrderStatus, o.TaxRate, o.ShippingCost;"
    q "qryOrderTotals", s

    s = "SELECT OrderID, Sum(Amount) AS AmountPaid, Max(PaymentDate) AS LastPaymentDate FROM Payments GROUP BY OrderID;"
    q "qryOrderPayments", s

    s = "SELECT t.*, CCur(Nz(p.AmountPaid,0)) AS AmountPaid, CCur(t.OrderTotal - Nz(p.AmountPaid,0)) AS BalanceDue, p.LastPaymentDate "
    s = s & "FROM qryOrderTotals AS t LEFT JOIN qryOrderPayments AS p ON t.OrderID = p.OrderID;"
    q "qryOrderBalances", s

    ' Payment header + lines (seller details added in a second step, as above)
    s = "SELECT pay.PaymentID, pay.OrderID, pay.PaymentDate, pay.Amount AS PaymentAmount, pay.PaymentMethod, pay.PaymentRef, "
    s = s & "o.OrderNumber, o.OrderDate, o.CustomerPO, "
    s = s & "c.CompanyName AS CustomerName, c.BillAddress, c.BillCity, c.BillState, c.BillZip, c.BillCountry, "
    s = s & "r.FirstName & ' ' & r.LastName AS RepName, "
    s = s & "l.OrderLineID, l.SKU, l.ItemDescription, l.Quantity, l.UnitPrice, l.DiscountPct, l.LineTotal, "
    s = s & "b.Subtotal, b.TaxAmount, b.ShippingCost, b.OrderTotal, b.AmountPaid, b.BalanceDue "
    s = s & "FROM ((((Payments AS pay INNER JOIN Orders AS o ON pay.OrderID = o.OrderID) "
    s = s & "INNER JOIN Customers AS c ON o.CustomerID = c.CustomerID) "
    s = s & "INNER JOIN SalesReps AS r ON o.RepID = r.RepID) "
    s = s & "INNER JOIN qryOrderBalances AS b ON o.OrderID = b.OrderID) "
    s = s & "LEFT JOIN qryOrderLines AS l ON o.OrderID = l.OrderID;"
    q "qryReceiptPrintBase", s

    ' Source for the printable RECEIPT report (one receipt per PaymentID)
    s = "SELECT rb.*, "
    s = s & "ci.CompanyName AS SellerName, ci.Address1 AS SellerAddress, ci.City & ', ' & ci.State & ' ' & ci.Zip AS SellerCityStateZip, "
    s = s & "ci.Phone AS SellerPhone, ci.Website AS SellerWebsite, ci.ReceiptFooter "
    s = s & "FROM qryReceiptPrintBase AS rb, CompanyInfo AS ci;"
    q "qryReceiptPrint", s

    ' Invoice header, one row per order (lines come from qryOrderLines in a subreport)
    s = "SELECT b.OrderID, b.OrderNumber, b.OrderDate, b.OrderStatus, b.Subtotal, b.TaxRate, b.TaxAmount, b.ShippingCost, "
    s = s & "b.OrderTotal, b.AmountPaid, b.BalanceDue, o.CustomerPO, o.ShipDate, o.ShipVia, o.TrackingNumber, "
    s = s & "c.CompanyName AS CustomerName, c.BillAddress, c.BillCity, c.BillState, c.BillZip, c.PaymentTerms, "
    s = s & "r.FirstName & ' ' & r.LastName AS RepName "
    s = s & "FROM ((qryOrderBalances AS b INNER JOIN Orders AS o ON b.OrderID = o.OrderID) "
    s = s & "INNER JOIN Customers AS c ON b.CustomerID = c.CustomerID) "
    s = s & "INNER JOIN SalesReps AS r ON b.RepID = r.RepID;"
    q "qryInvoiceBase", s

    s = "SELECT ib.*, ci.CompanyName AS SellerName, ci.Address1 AS SellerAddress, "
    s = s & "ci.City & ', ' & ci.State & ' ' & ci.Zip AS SellerCityStateZip, ci.Phone AS SellerPhone, ci.Website AS SellerWebsite "
    s = s & "FROM qryInvoiceBase AS ib, CompanyInfo AS ci;"
    q "qryInvoicePrint", s

    ' ---- Month-end reporting (prompts for any date in the month, e.g. 9/1/2026) ----
    s = "PARAMETERS [Month (any date in month)] DateTime; "
    s = s & "SELECT b.OrderNumber, b.OrderDate, b.OrderStatus, c.CompanyName AS CustomerName, "
    s = s & "r.FirstName & ' ' & r.LastName AS RepName, b.Subtotal, b.TaxAmount, b.ShippingCost, b.OrderTotal, "
    s = s & "b.ProductCost, b.Subtotal - b.ProductCost AS GrossMargin, b.AmountPaid, b.BalanceDue "
    s = s & "FROM (qryOrderBalances AS b INNER JOIN Customers AS c ON b.CustomerID = c.CustomerID) "
    s = s & "INNER JOIN SalesReps AS r ON b.RepID = r.RepID "
    s = s & "WHERE b.OrderStatus <> 'Cancelled' "
    s = s & "AND b.OrderDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) "
    s = s & "AND b.OrderDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) "
    s = s & "ORDER BY b.OrderDate;"
    q "qryMonthEnd_Orders", s

    s = "PARAMETERS [Month (any date in month)] DateTime; "
    s = s & "SELECT r.RepID, r.FirstName & ' ' & r.LastName AS RepName, Count(b.OrderID) AS OrderCount, "
    s = s & "Sum(b.Subtotal) AS NetSales, Sum(b.OrderTotal) AS GrossBilled, Sum(b.Subtotal - b.ProductCost) AS GrossMargin, "
    s = s & "r.CommissionRate, Round(Sum(b.Subtotal)*r.CommissionRate,2) AS CommissionEarned "
    s = s & "FROM qryOrderTotals AS b INNER JOIN SalesReps AS r ON b.RepID = r.RepID "
    s = s & "WHERE b.OrderStatus <> 'Cancelled' "
    s = s & "AND b.OrderDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) "
    s = s & "AND b.OrderDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) "
    s = s & "GROUP BY r.RepID, r.FirstName, r.LastName, r.CommissionRate "
    s = s & "ORDER BY Sum(b.Subtotal) DESC;"
    q "qryMonthEnd_SalesByRep", s

    s = "PARAMETERS [Month (any date in month)] DateTime; "
    s = s & "SELECT l.Category, l.SKU, l.ItemDescription, Sum(l.Quantity) AS UnitsSold, Sum(l.LineTotal) AS Revenue, "
    s = s & "Sum(l.LineCost) AS Cost, Sum(l.LineTotal) - Sum(l.LineCost) AS Margin "
    s = s & "FROM qryOrderLines AS l INNER JOIN Orders AS o ON l.OrderID = o.OrderID "
    s = s & "WHERE o.OrderStatus <> 'Cancelled' "
    s = s & "AND o.OrderDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) "
    s = s & "AND o.OrderDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) "
    s = s & "GROUP BY l.Category, l.SKU, l.ItemDescription "
    s = s & "ORDER BY l.Category, Sum(l.LineTotal) DESC;"
    q "qryMonthEnd_SalesByProduct", s

    s = "PARAMETERS [Month (any date in month)] DateTime; "
    s = s & "SELECT p.PaymentDate, o.OrderNumber, c.CompanyName AS CustomerName, p.PaymentMethod, p.PaymentRef, p.Amount "
    s = s & "FROM (Payments AS p INNER JOIN Orders AS o ON p.OrderID = o.OrderID) "
    s = s & "INNER JOIN Customers AS c ON o.CustomerID = c.CustomerID "
    s = s & "WHERE p.PaymentDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) "
    s = s & "AND p.PaymentDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) "
    s = s & "ORDER BY p.PaymentDate;"
    q "qryMonthEnd_CashReceived", s

    s = "PARAMETERS [Month (any date in month)] DateTime; "
    s = s & "SELECT t.QuoteStatus, Count(t.QuoteID) AS Quotes, Sum(t.QuoteTotal) AS PipelineValue "
    s = s & "FROM qryQuoteTotals AS t "
    s = s & "WHERE t.QuoteDate >= DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)]), 1) "
    s = s & "AND t.QuoteDate < DateSerial(Year([Month (any date in month)]), Month([Month (any date in month)])+1, 1) "
    s = s & "GROUP BY t.QuoteStatus;"
    q "qryMonthEnd_QuotePipeline", s

    s = "SELECT b.OrderNumber, c.CompanyName AS CustomerName, b.OrderDate, b.OrderStatus, b.OrderTotal, b.AmountPaid, b.BalanceDue, "
    s = s & "Date() - b.OrderDate AS DaysOutstanding, c.PaymentTerms "
    s = s & "FROM qryOrderBalances AS b INNER JOIN Customers AS c ON b.CustomerID = c.CustomerID "
    s = s & "WHERE b.BalanceDue > 0.005 AND b.OrderStatus <> 'Cancelled' "
    s = s & "ORDER BY b.OrderDate;"
    q "qryOpenReceivables", s

    ' Source for the commission statement report (filtered by month when opened)
    s = "SELECT b.RepID, r.FirstName & ' ' & r.LastName AS RepName, r.Email AS RepEmail, r.CommissionRate, "
    s = s & "b.OrderID, b.OrderNumber, b.OrderDate, b.OrderStatus, c.CompanyName AS CustomerName, "
    s = s & "b.Subtotal AS NetSales, b.Subtotal - b.ProductCost AS GrossMargin, "
    s = s & "CCur(Round(b.Subtotal * r.CommissionRate, 2)) AS Commission, b.AmountPaid, b.BalanceDue "
    s = s & "FROM (qryOrderBalances AS b INNER JOIN SalesReps AS r ON b.RepID = r.RepID) "
    s = s & "INNER JOIN Customers AS c ON b.CustomerID = c.CustomerID "
    s = s & "WHERE b.OrderStatus <> 'Cancelled';"
    q "qryCommissionDetail", s

    ' ---- CRM views ----
    s = "SELECT m.MessageID, m.ReceivedAt, fr.FirstName & ' ' & fr.LastName AS ForRep, "
    s = s & "tb.FirstName & ' ' & tb.LastName AS TakenBy, m.CallerName, Nz(c.CompanyName, m.CallerCompany) AS Company, "
    s = s & "m.CallerPhone, m.Message, m.Urgent, m.PleaseCallBack, m.WillCallAgain, m.MessageStatus "
    s = s & "FROM ((PhoneMessages AS m INNER JOIN SalesReps AS fr ON m.ForRepID = fr.RepID) "
    s = s & "LEFT JOIN SalesReps AS tb ON m.TakenByRepID = tb.RepID) "
    s = s & "LEFT JOIN Customers AS c ON m.CustomerID = c.CustomerID "
    s = s & "WHERE m.MessageStatus NOT IN ('Returned', 'Closed') "
    s = s & "ORDER BY m.Urgent, m.ReceivedAt;"
    q "qryOpenPhoneMessages", s

    s = "SELECT a.ActivityID, a.FollowUpDate, r.FirstName & ' ' & r.LastName AS RepName, c.CompanyName AS CustomerName, "
    s = s & "ct.FirstName & ' ' & ct.LastName AS ContactName, ct.Phone AS ContactPhone, a.ActivityType, a.Subject, a.Details "
    s = s & "FROM ((Activities AS a INNER JOIN Customers AS c ON a.CustomerID = c.CustomerID) "
    s = s & "INNER JOIN SalesReps AS r ON a.RepID = r.RepID) "
    s = s & "LEFT JOIN Contacts AS ct ON a.ContactID = ct.ContactID "
    s = s & "WHERE a.Completed = False AND a.FollowUpDate Is Not Null AND a.FollowUpDate <= Date() + 7 "
    s = s & "ORDER BY a.FollowUpDate;"
    q "qryFollowUpsDue", s

    s = "SELECT c.CompanyName AS CustomerName, a.ActivityDate, a.ActivityType, a.Subject, a.Details, "
    s = s & "r.FirstName & ' ' & r.LastName AS RepName, ct.FirstName & ' ' & ct.LastName AS ContactName, a.FollowUpDate, a.Completed "
    s = s & "FROM ((Activities AS a INNER JOIN Customers AS c ON a.CustomerID = c.CustomerID) "
    s = s & "INNER JOIN SalesReps AS r ON a.RepID = r.RepID) "
    s = s & "LEFT JOIN Contacts AS ct ON a.ContactID = ct.ContactID "
    s = s & "ORDER BY c.CompanyName, a.ActivityDate DESC;"
    q "qryCustomerActivityLog", s
End Sub

'------------------------------------------------------------------------------
'  Remove sample rows (keeps CompanyInfo and Products). Run before going live.
'------------------------------------------------------------------------------
Public Sub ClearSampleData()
    If MsgBox("Delete ALL quotes, orders, payments, customers, contacts, activities, " & _
              "phone messages and sales reps?", vbYesNo + vbExclamation, "Cairn Systems") <> vbYes Then Exit Sub
    Dim db As DAO.Database: Set db = CurrentDb
    If TableExists("ManufacturerOrders") Then db.Execute "DELETE FROM ManufacturerOrders", dbFailOnError
    If TableExists("RMAs") Then db.Execute "DELETE FROM RMAs", dbFailOnError
    If TableExists("Users") Then db.Execute "DELETE FROM Users WHERE RepID Is Not Null", dbFailOnError
    db.Execute "DELETE FROM Payments", dbFailOnError
    db.Execute "DELETE FROM Orders", dbFailOnError        ' cascades OrderLines
    db.Execute "DELETE FROM Quotes", dbFailOnError        ' cascades QuoteLines
    db.Execute "DELETE FROM PhoneMessages", dbFailOnError
    db.Execute "DELETE FROM Customers", dbFailOnError     ' cascades Contacts, Activities
    db.Execute "DELETE FROM SalesReps", dbFailOnError
    MsgBox "Sample data cleared. Update the CompanyInfo row with your real address.", vbInformation
End Sub

'------------------------------------------------------------------------------
'  Helpers
'------------------------------------------------------------------------------
Private Sub x(ByVal sql As String)
    On Error GoTo Fail
    cn.Execute sql
    Exit Sub
Fail:
    MsgBox "Error " & Err.Number & ": " & Err.Description & vbCrLf & vbCrLf & sql, vbCritical, "Build step failed"
    End
End Sub

Private Sub q(ByVal qName As String, ByVal sql As String)
    On Error GoTo Fail
    Dim db As DAO.Database: Set db = CurrentDb
    On Error Resume Next
    db.QueryDefs.Delete qName
    On Error GoTo Fail
    db.CreateQueryDef qName, sql
    Exit Sub
Fail:
    MsgBox "Could not create query " & qName & vbCrLf & Err.Description & vbCrLf & vbCrLf & sql, vbCritical, "Query step failed"
End Sub

Private Function TableExists(ByVal tName As String) As Boolean
    Dim t As DAO.TableDef
    For Each t In CurrentDb.TableDefs
        If t.Name = tName Then TableExists = True: Exit Function
    Next
End Function

Private Sub SetFmt(ByVal tName As String, ByVal fName As String, ByVal fmt As String)
    On Error Resume Next
    Dim f As DAO.Field
    Set f = CurrentDb.TableDefs(tName).Fields(fName)
    f.Properties("Format") = fmt
    If Err.Number <> 0 Then
        Err.Clear
        f.Properties.Append f.CreateProperty("Format", dbText, fmt)
    End If
End Sub

Private Function Sq(ByVal v As Variant) As String
    If IsNull(v) Then
        Sq = "Null"
    Else
        Sq = "'" & Replace(CStr(v), "'", "''") & "'"
    End If
End Function

Private Function Dt(ByVal v As Variant) As String
    If IsNull(v) Then Dt = "Null" Else Dt = Format(v, "\#yyyy\-mm\-dd hh\:nn\:ss\#")
End Function

Private Function nm(ByVal v As Variant) As String
    If IsNull(v) Then nm = "Null" Else nm = Str(v)
End Function

Private Sub AddProduct(sku As String, pName As String, cat As String, price As Currency, cost As Currency, fw As String, lead As Integer)
    x "INSERT INTO Products (SKU, ProductName, Category, UnitPrice, UnitCost, FirmwareVersion, LeadTimeDays) VALUES (" & _
      Sq(sku) & "," & Sq(pName) & "," & Sq(cat) & "," & nm(price) & "," & nm(cost) & "," & _
      IIf(fw = "", "Null", Sq(fw)) & "," & lead & ")"
End Sub

Private Sub AddQuote(qNum As String, custID As Long, contactID As Long, repID As Long, qDate As Date, validTo As Date, _
                     status As String, taxRate As Double, ship As Currency)
    x "INSERT INTO Quotes (QuoteNumber, CustomerID, ContactID, RepID, QuoteDate, ValidUntil, QuoteStatus, TaxRate, ShippingCost) VALUES (" & _
      Sq(qNum) & "," & custID & "," & contactID & "," & repID & "," & Dt(qDate) & "," & Dt(validTo) & "," & _
      Sq(status) & "," & nm(taxRate) & "," & nm(ship) & ")"
End Sub

Private Sub AddLine(tbl As String, parentField As String, parentID As Long, productID As Long, qty As Long, _
                    price As Currency, disc As Double)
    x "INSERT INTO " & tbl & " (" & parentField & ", ProductID, Quantity, UnitPrice, DiscountPct) VALUES (" & _
      parentID & "," & productID & "," & qty & "," & nm(price) & "," & nm(disc) & ")"
End Sub

Private Sub AddActivity(custID As Long, contactID As Long, repID As Long, aDate As Date, aType As String, _
                        subj As String, details As String, followUp As Variant, Done As Boolean)
    x "INSERT INTO Activities (CustomerID, ContactID, RepID, ActivityDate, ActivityType, Subject, Details, FollowUpDate, Completed) VALUES (" & _
      custID & "," & contactID & "," & repID & "," & Dt(aDate) & "," & Sq(aType) & "," & Sq(subj) & "," & _
      Sq(details) & "," & Dt(followUp) & "," & IIf(Done, "-1", "0") & ")"
End Sub

Private Sub AddMsg(rcv As Date, forRep As Long, takenBy As Long, custID As Variant, caller As String, company As String, _
                   phone As String, msg As String, urgent As Boolean, callBack As Boolean, status As String)
    x "INSERT INTO PhoneMessages (ReceivedAt, ForRepID, TakenByRepID, CustomerID, CallerName, CallerCompany, CallerPhone, Message, Urgent, PleaseCallBack, MessageStatus) VALUES (" & _
      Dt(rcv) & "," & forRep & "," & takenBy & "," & nm(custID) & "," & Sq(caller) & "," & Sq(company) & "," & _
      Sq(phone) & "," & Sq(msg) & "," & IIf(urgent, "-1", "0") & "," & IIf(callBack, "-1", "0") & "," & Sq(status) & ")"
End Sub

