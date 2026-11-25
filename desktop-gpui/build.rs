fn main() {
    println!("cargo:rerun-if-changed=../assets/icons/icon.ico");
    #[cfg(windows)]
    {
        // GPUI supplies its PerMonitorV2/asInvoker manifest. Only add application
        // metadata here so the linker retains one authoritative manifest.
        let mut resources = tauri_winres::WindowsResource::new();
        resources
            .set_icon_with_id("../assets/icons/icon.ico", "1")
            .set("ProductName", "Metra")
            .set("FileDescription", "Metra")
            .set("CompanyName", "metra")
            .set("OriginalFilename", "metra.exe")
            .set("InternalName", "metra")
            .set("FileVersion", env!("CARGO_PKG_VERSION"))
            .set("ProductVersion", env!("CARGO_PKG_VERSION"));
        resources
            .compile()
            .expect("compile Windows application resources");
    }
}
