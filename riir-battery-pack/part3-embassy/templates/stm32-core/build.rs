fn main() {
    // Places memory.x (from the chip-specific HAL feature) on the linker
    // search path. A real project also needs a `.cargo/config.toml`
    // pointing the linker at `link.x`, which this template doesn't
    // generate -- see the Embassy book's project-setup chapter.
    println!("cargo:rustc-link-search=.");
}
