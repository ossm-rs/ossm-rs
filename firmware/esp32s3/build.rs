fn main() {
    ossm_esp_build_support::emit_build_info();
    ossm_esp_build_support::linker_be_nice();
    println!("cargo:rustc-link-arg=-Tlinkall.x");
}
