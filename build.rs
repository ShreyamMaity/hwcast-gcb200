// Windows version information for hwcast-gcb200.exe.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set("ProductName", "hwcast-gcb200")
            .set("FileDescription", "hwcast mod for the GCB200 segment display")
            .set("InternalName", "hwcast-gcb200")
            .set("OriginalFilename", "hwcast-gcb200.exe")
            .set("CompanyName", "hwcast")
            .set("LegalCopyright", "Copyright (c) 2026 Shreyam. MIT License.");
        res.compile().expect("failed to embed the Windows version resource");
    }
}
