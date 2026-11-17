//! Windows resources for the GPUI build.
//!
//! Without this the binary is a console program (a cmd window flashes next to
//! the app) and carries no icon at all. The `.rc` below embeds the shared GD
//! icon as resource id 1, which is exactly the id GPUI's `load_icon()` passes
//! to `LoadImageW` for the window icon.

fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../assets/githubdeck.ico");
        resource.set("ProductName", "GithubDeck");
        resource.set("FileDescription", "GithubDeck — GitHub CLI 账号管理");
        resource.set(
            "LegalCopyright",
            "Copyright (c) GithubDeck contributors",
        );
        resource
            .compile()
            .expect("compile Windows resources (icon and manifest)");
        println!("cargo:rerun-if-changed=../../assets/githubdeck.ico");
        println!("cargo:rerun-if-changed=build.rs");
    }
}
