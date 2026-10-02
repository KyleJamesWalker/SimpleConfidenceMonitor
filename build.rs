fn main() {
    #[cfg(feature = "gui")]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=packaging/windows/app.ico");
        winresource::WindowsResource::new()
            .set_icon("packaging/windows/app.ico")
            .compile()
            .expect("embed the Windows icon");
    }
}
