fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_widget_reload();
    }
    tauri_build::build()
}

// Compile the tiny Swift WidgetKit shim (WidgetCenter.reloadAllTimelines) into a
// static lib and link it + the WidgetKit framework. The Swift runtime is the
// ABI-stable OS one (resolved against the SDK at link time, dyld at runtime).
fn build_widget_reload() {
    use std::process::Command;

    let src = "swift/widget_reload.swift";
    println!("cargo:rerun-if-changed={src}");

    let out = std::env::var("OUT_DIR").unwrap();
    let obj = format!("{out}/widget_reload.o");
    let lib = format!("{out}/libnotefixreload.a");

    let sdk = String::from_utf8(
        Command::new("xcrun")
            .args(["--show-sdk-path"])
            .output()
            .expect("xcrun")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    assert!(
        Command::new("swiftc")
            .args(["-emit-object", "-parse-as-library", "-O", src, "-o", &obj])
            .status()
            .expect("swiftc")
            .success(),
        "swiftc failed to build the widget-reload shim"
    );
    let _ = std::fs::remove_file(&lib);
    assert!(
        Command::new("ar")
            .args(["crus", &lib, &obj])
            .status()
            .expect("ar")
            .success(),
        "ar failed to archive the widget-reload shim"
    );

    println!("cargo:rustc-link-search=native={out}");
    println!("cargo:rustc-link-lib=static=notefixreload");
    println!("cargo:rustc-link-lib=framework=WidgetKit");
    println!("cargo:rustc-link-arg=-L{sdk}/usr/lib/swift");
    println!("cargo:rustc-link-arg=-L/usr/lib/swift");
}
