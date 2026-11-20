//! Embeds the app icon as resource id 1 (GPUI's Windows backend loads icon id
//! 1 from the exe). GPUI's own build script already embeds the application
//! manifest, so this one does not.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon = "../../apps/desktop/src-tauri/icons/icon.ico";
        println!("cargo:rerun-if-changed={icon}");
        winresource::WindowsResource::new()
            .set_icon(icon)
            .compile()
            .expect("embedding the icon");
    }
}
