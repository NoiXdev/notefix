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

    // Compile the shim for the arch Cargo is currently targeting, not the host
    // arch — otherwise a `universal-apple-darwin` build produces an arm64-only
    // object and the x86_64 slice fails to link (`_notefix_reload_widgets`
    // undefined). The deployment target is 14.0 (matching the widget extension):
    // at 13.0+ swiftc no longer force-loads the Swift back-deployment
    // compatibility static libs, which are not on the link line and would
    // otherwise fail to resolve.
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x86_64".to_string(),
        Ok("aarch64") => "arm64".to_string(),
        Ok(other) => other.to_string(),
        Err(_) => "arm64".to_string(),
    };
    let target = format!("{arch}-apple-macosx14.0");

    assert!(
        Command::new("swiftc")
            .args([
                "-emit-object",
                "-parse-as-library",
                "-O",
                "-target",
                &target,
                "-sdk",
                &sdk,
                src,
                "-o",
                &obj,
            ])
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
