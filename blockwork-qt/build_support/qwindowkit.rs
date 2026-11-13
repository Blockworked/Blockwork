use std::path::{Path, PathBuf};

pub fn build() -> PathBuf {
    println!("cargo:rerun-if-changed=native/CMakeLists.txt");
    println!("cargo:rerun-if-env-changed=BLOCKWORK_QWINDOWKIT_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=CARGO_NET_OFFLINE");
    let mut config = cmake::Config::new("native");
    if let Some(source) = std::env::var_os("BLOCKWORK_QWINDOWKIT_SOURCE_DIR") {
        let source = PathBuf::from(source)
            .canonicalize()
            .expect("find QWindowKit source override");
        assert!(
            source.join("CMakeLists.txt").is_file(),
            "Invalid QWindowKit source override"
        );
        println!("cargo:rerun-if-changed={}", source.display());
        config.define("FETCHCONTENT_SOURCE_DIR_QWINDOWKIT", source);
    } else {
        // Clear a previous override when returning to the cached upstream dependency.
        config.define("FETCHCONTENT_SOURCE_DIR_QWINDOWKIT", "");
    }
    let offline = std::env::var("CARGO_NET_OFFLINE").is_ok_and(|value| value == "true");
    config.define(
        "FETCHCONTENT_FULLY_DISCONNECTED",
        if offline { "ON" } else { "OFF" },
    );

    // Use the exact Qt installation selected by CXX-Qt, including its private headers.
    let qt = qt_build_utils::QtBuild::new(vec!["Core".into(), "Gui".into(), "Quick".into()])
        .expect("find Qt for QWindowKit");
    let libraries = qt
        .qtpaths()
        .query("QT_INSTALL_LIBS")
        .expect("query Qt library directory");
    let qt_cmake = Path::new(&libraries).join("cmake/Qt6");
    assert!(
        qt_cmake.join("Qt6Config.cmake").is_file(),
        "QWindowKit requires a full Qt SDK with CMake files and private headers. Set QMAKE to that SDK's qmake"
    );

    config
        .out_dir(PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("qwindowkit"))
        // conf.cmake looks Qt up as "QT" first, which Qt6_DIR alone doesn't answer.
        .define("QT_VERSION_MAJOR", "6")
        .define("QT_DIR", &qt_cmake)
        .define("CMAKE_PREFIX_PATH", Path::new(&libraries).join("cmake"))
        .define("Qt6_DIR", qt_cmake)
        .define("CMAKE_INSTALL_LIBDIR", "lib")
        .define("CMAKE_DEBUG_POSTFIX", "")
        .define("QWINDOWKIT_INSTALL", "ON")
        .profile("Release")
        .build()
}

pub fn link(prefix: &Path) {
    println!("cargo:rustc-link-search=native={}/lib", prefix.display());
    println!("cargo:rustc-link-lib=static=QWKQuick");
    println!("cargo:rustc-link-lib=static=QWKCore");
    match std::env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        "windows" => println!("cargo:rustc-link-lib=uxtheme"),
        "macos" => {
            for framework in ["Foundation", "Cocoa", "AppKit"] {
                println!("cargo:rustc-link-lib=framework={framework}");
            }
        }
        _ => {}
    }
}
