use std::{env, path::PathBuf, process::Command};

const RESOURCE_XML: &str = "resources/io.github.miiikuuu.termimochi.gresource.xml";
const ICON_FILES: &[&str] = &[
    "resources/fastfetch/LICENSE",
    "resources/fastfetch/README.md",
    "resources/icons/16x16/apps/io.github.miiikuuu.termimochi.png",
    "resources/icons/24x24/apps/io.github.miiikuuu.termimochi.png",
    "resources/icons/32x32/apps/io.github.miiikuuu.termimochi.png",
    "resources/icons/48x48/apps/io.github.miiikuuu.termimochi.png",
    "resources/icons/64x64/apps/io.github.miiikuuu.termimochi.png",
    "resources/icons/128x128/apps/io.github.miiikuuu.termimochi.png",
    "resources/icons/256x256/apps/io.github.miiikuuu.termimochi.png",
    "resources/icons/512x512/apps/io.github.miiikuuu.termimochi.png",
    "resources/icons/scalable/apps/io.github.miiikuuu.termimochi.svg",
    "resources/icons/scalable/apps/termimochi-header-logo.svg",
    "resources/icons/symbolic/actions/termimochi-open-symbolic.svg",
    "resources/icons/symbolic/actions/termimochi-save-symbolic.svg",
    "resources/icons/symbolic/actions/termimochi-layout-symbolic.svg",
    "resources/icons/symbolic/actions/termimochi-prompt-symbolic.svg",
    "resources/icons/symbolic/apps/io.github.miiikuuu.termimochi-symbolic.svg",
];

fn main() {
    let crate_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR"),
    );
    let resource_xml = crate_dir.join(RESOURCE_XML);
    let target = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR"))
        .join("termimochi.gresource");

    let status = Command::new("glib-compile-resources")
        .arg(&resource_xml)
        .arg(format!("--sourcedir={}", crate_dir.display()))
        .arg(format!("--target={}", target.display()))
        .status()
        .expect("glib-compile-resources is required to build the TermiMochi desktop app");
    assert!(status.success(), "failed to compile TermiMochi resources");

    println!("cargo:rerun-if-changed={}", resource_xml.display());
    for icon in ICON_FILES {
        println!("cargo:rerun-if-changed={}", crate_dir.join(icon).display());
    }
}
