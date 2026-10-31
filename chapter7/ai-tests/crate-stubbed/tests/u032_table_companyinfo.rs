// Tests for table:CompanyInfo — the single-row company-wide settings table.
// Evidence: data row CompanyInfoID=1; CairnForms L712 (tax rate), L859-863
// (manufacturer URL guard); CairnBuild L529-532 (quote print), L573-575
// (receipt print), L588-590 (invoice print).

#[test]
fn company_info_has_exactly_one_row() {
    // Every consumer joins CompanyInfo with no WHERE clause (CairnBuild L532,
    // L575, L590) or uses an unqualified DLookup (CairnForms L712, L859),
    // so the table is assumed to contain exactly one row.
    assert_eq!(company_info_row_count(), 1);
}

#[test]
fn company_name_is_cairn_systems() {
    assert_eq!(company_name(), "Cairn Systems");
}

#[test]
fn default_tax_rate_is_8_25_percent() {
    // Data: defaulttaxrate = 0.0825 (DDL default is 0, CairnBuild L224).
    assert!((default_tax_rate() - 0.0825).abs() < 1e-12);
}

#[test]
fn default_tax_rate_falls_back_to_zero_when_unset() {
    // frm!taxRate = Nz(DLookup("DefaultTaxRate","CompanyInfo"),0) (CairnForms L712):
    // a missing/null value yields 0, a present value passes through.
    assert_eq!(default_tax_rate_or_zero(None), 0.0);
    assert_eq!(default_tax_rate_or_zero(Some(0.0825)), 0.0825);
}

#[test]
fn quote_terms_fall_back_to_company_default_when_quote_terms_null() {
    // Nz(b.Terms, ci.QuoteTerms) (CairnBuild L529): a quote with its own
    // Terms keeps them; a null Terms takes the company-wide default.
    assert_eq!(
        quote_terms_fallback(None),
        "Prices valid 30 days from quote date. Net 30 on approved credit. Standard lead time 3-4 weeks ARO. Firmware updates included for 12 months."
    );
    assert_eq!(quote_terms_fallback(Some("Custom terms")), "Custom terms");
}

#[test]
fn receipt_footer_text_is_the_company_default() {
    // Appended to every receipt print (CairnBuild L574).
    assert_eq!(
        receipt_footer(),
        "Thank you for your business! Questions about this receipt? Contact your account representative."
    );
}

#[test]
fn manufacturer_api_url_is_configured() {
    assert_eq!(manufacturer_api_url(), "http://localhost:8765/orders");
}

#[test]
fn manufacturer_order_aborts_with_message_when_url_empty() {
    // url = Nz(DLookup("ManufacturerApiUrl","CompanyInfo"), ""); if Len(url)=0
    // show "No manufacturer address is set up in Company Info." and exit
    // (CairnForms L859-863). A non-empty URL proceeds (guard returns None).
    assert_eq!(
        manufacturer_url_guard(""),
        Some("No manufacturer address is set up in Company Info.")
    );
    assert_eq!(manufacturer_url_guard("http://localhost:8765/orders"), None);
}

#[test]
fn seller_city_state_zip_joins_city_comma_space_state_space_zip() {
    // ci.City & ', ' & ci.State & ' ' & ci.Zip (CairnBuild L530, L573, L588).
    assert_eq!(seller_city_state_zip("[City]", "[ST]", "[Zip]"), "[City], [ST] [Zip]");
}

#[test]
fn quote_print_seller_block_uses_company_info_fields() {
    // SellerName, SellerAddress, SellerCityStateZip, SellerPhone, SellerEmail,
    // SellerWebsite (CairnBuild L530-531), from the single CompanyInfo row.
    let s = quote_print_seller_block();
    assert_eq!(s.name, "Cairn Systems");
    assert_eq!(s.address, "[Street Address]");
    assert_eq!(s.city_state_zip, "[City], [ST] [Zip]");
    assert_eq!(s.phone, "[Phone]");
    assert_eq!(s.email, "sales@cairnsystems.example");
    assert_eq!(s.website, "www.cairnsystems.example");
}

// ---- Added for the book (2026-10-28). Not part of the AI's output. ----
// Stubs with the signatures the tests above assume, so the file compiles.
// Each body is todo!() until the rule is implemented.
struct SellerBlock { name: String, address: String, city_state_zip: String, phone: String, email: String, website: String }
fn company_info_row_count() -> usize { todo!() }
fn company_name() -> String { todo!() }
fn default_tax_rate() -> f64 { todo!() }
fn default_tax_rate_or_zero(_r: Option<f64>) -> f64 { todo!() }
fn quote_terms_fallback(_terms: Option<&str>) -> String { todo!() }
fn receipt_footer() -> String { todo!() }
fn manufacturer_api_url() -> String { todo!() }
fn manufacturer_url_guard(_url: &str) -> Option<&'static str> { todo!() }
fn seller_city_state_zip(_city: &str, _state: &str, _zip: &str) -> String { todo!() }
fn quote_print_seller_block() -> SellerBlock { todo!() }
