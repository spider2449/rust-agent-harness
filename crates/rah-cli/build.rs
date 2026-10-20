//! Embeds the RAH icon in the Windows executable, so Explorer and the taskbar
//! show it. Other targets need nothing.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=icon.rc");
    println!("cargo:rerun-if-changed=../../packaging/windows/rah.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // A missing resource compiler only costs the icon, not the build.
    if let Err(error) = embed_resource::compile("icon.rc", embed_resource::NONE).manifest_optional()
    {
        println!("cargo:warning=the icon was not embedded: {error}");
    }
}
