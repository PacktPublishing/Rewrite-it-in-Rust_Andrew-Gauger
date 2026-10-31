// Tests for table:Contacts
// Schema: contactid SERIAL PK; customerid INT NOT NULL FK->Customers ON DELETE CASCADE;
//         firstname VARCHAR(50) NOT NULL; lastname/title/email/phone/mobile nullable;
//         isprimary BOOLEAN NOT NULL; notes TEXT nullable.
//
// Assumed harness functions (plausible signatures, not implemented here):
//   fn add_contact(customerid: Option<i32>, firstname: &str, lastname: Option<&str>,
//                   title: Option<&str>, email: Option<&str>, phone: Option<&str>,
//                   mobile: Option<&str>, isprimary: bool, notes: Option<&str>) -> i32
//   fn delete_customer(customerid: i32) -> i32            // returns # of contacts removed
//   fn primary_contact(customerid: i32) -> Option<i32>    // mirrors DLookup(...IsPrimary=True)
//   fn contacts_of(customerid: i32) -> Vec<Contact>
//   struct Contact { contactid: i32, customerid: i32, firstname: String,
//                    lastname: Option<String>, title: Option<String>, email: Option<String>,
//                    phone: Option<String>, mobile: Option<String>, isprimary: bool,
//                    notes: Option<String> }

#[test]
fn contact_requires_a_customer() {
    // customerid is NOT NULL: a contact with no customer must be rejected.
    let r = add_contact(None, "Elena", Some("Brooks"), None, None, None, None, true, None);
    assert!(r.is_none(), "contact without a customerid must be rejected (NOT NULL)");
}

#[test]
fn contact_customer_must_exist() {
    // customerid is a FK to Customers: a contact pointing at a missing customer must fail.
    let r = add_contact(Some(999_999), "Ghost", None, None, None, None, None, true, None);
    assert!(r.is_none(), "contact referencing a non-existent customer must be rejected (FK)");
}

#[test]
fn deleting_customer_cascades_to_its_contacts() {
    // FK ON DELETE CASCADE: removing a customer removes every contact that belongs to it.
    let c = add_customer(999);
    let a = add_contact(Some(c), "Elena", Some("Brooks"), None, None, None, None, true, None);
    let b = add_contact(Some(c), "Tom", Some("Reyes"), None, None, None, None, false, None);
    let removed = delete_customer(c);
    assert_eq!(removed, 2, "cascade must delete both of the customer's contacts");
    assert!(contacts_of(c).is_empty(), "no contacts may survive the customer's deletion");
    assert!(get_contact(a).is_none() && get_contact(b).is_none());
}

#[test]
fn only_firstname_is_required() {
    // firstname is the only NOT NULL descriptive field; lastname/title/email/phone/mobile/notes
    // are all nullable, so a contact with just a first name is valid.
    let c = add_customer(998);
    let id = add_contact(Some(c), "Grace", None, None, None, None, None, true, None)
        .expect("a contact with only firstname must be accepted");
    let got = get_contact(id).expect("contact must be retrievable");
    assert_eq!(got.firstname, "Grace");
    assert!(got.lastname.is_none() && got.title.is_none() && got.email.is_none()
        && got.phone.is_none() && got.mobile.is_none() && got.notes.is_none());
}

#[test]
fn exactly_one_primary_contact_per_customer() {
    // Data invariant: each customer has exactly one isprimary=TRUE contact.
    // Mirrors customer 10 -> contacts 14 (primary), 15, 16 (non-primary).
    let c = add_customer(997);
    let p = add_contact(Some(c), "Stephanie", Some("Jackson"), None, None, None, None, true, None);
    add_contact(Some(c), "Stephanie", Some("Harris"), None, None, None, None, false, None);
    add_contact(Some(c), "Eric", None, Some("Operations Manager"), None, None, None, false, None);
    assert_eq!(primary_contact(c), Some(p), "the single primary contact must be returned");
    let primaries = contacts_of(c).iter().filter(|k| k.isprimary).count();
    assert_eq!(primaries, 1, "exactly one contact per customer may be primary");
}

#[test]
fn customer_without_contacts_has_no_primary() {
    // A customer with zero contacts yields no primary (DLookup returns Null).
    let c = add_customer(996);
    assert_eq!(primary_contact(c), None);
}

#[test]
fn contactid_is_auto_incrementing() {
    // contactid is a SERIAL: each new contact gets the next unused id.
    let c = add_customer(995);
    let a = add_contact(Some(c), "Elena", Some("Brooks"), None, None, None, None, true, None);
    let b = add_contact(Some(c), "Marcus", Some("Webb"), None, None, None, None, false, None);
    assert_eq!(b, a + 1, "second contact must get the next serial id");
}

#[test]
fn isprimary_is_required() {
    // isprimary is NOT NULL: a contact must be created with an explicit primary flag.
    // (Represented here by the fact that every valid contact carries a definite flag.)
    let c = add_customer(994);
    let id = add_contact(Some(c), "Hannah", Some("Cole"), None, None, None, None, true, None);
    assert!(get_contact(id).expect("contact exists").isprimary);
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
struct Contact {
    contactid: i32, customerid: i32, firstname: String, lastname: Option<String>,
    title: Option<String>, email: Option<String>, phone: Option<String>,
    mobile: Option<String>, isprimary: bool, notes: Option<String>,
}
#[allow(clippy::too_many_arguments)]
fn add_contact(_customerid: Option<i32>, _firstname: &str, _lastname: Option<&str>, _title: Option<&str>,
               _email: Option<&str>, _phone: Option<&str>, _mobile: Option<&str>, _isprimary: bool,
               _notes: Option<&str>) -> Option<i32> { todo!() }
fn add_customer(_seed: i32) -> i32 { todo!() }
fn delete_customer(_customerid: i32) -> i32 { todo!() }
fn primary_contact(_customerid: i32) -> Option<i32> { todo!() }
fn contacts_of(_customerid: i32) -> Vec<Contact> { todo!() }
fn get_contact(_contactid: i32) -> Option<Contact> { todo!() }
