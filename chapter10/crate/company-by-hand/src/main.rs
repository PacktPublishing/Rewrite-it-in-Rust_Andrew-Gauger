use company_by_hand::{CompanyInfo, CompanyScreen};

fn main() -> eframe::Result {
    eframe::run_native(
        "Company Info",
        eframe::NativeOptions::default(),
        Box::new(|_cc| {
            Ok(Box::new(CompanyScreen::new(CompanyInfo::v1())))
        }),
    )
}
