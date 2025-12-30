use std::env;

fn main() {
    cxx_build::bridge("src/lib.rs")
        .cpp(true)
        .std("c++14")
        .compile("rs_dfu");

    if env::var("TARGET").is_ok_and(|s| s.contains("windows-msvc")) {
        // MSVC compiler suite
        if env::var("CFLAGS").is_ok_and(|s| s.contains("/MDd")) {
            // debug runtime flag is set

            // Don't link the default CRT
            println!("cargo::rustc-link-arg=/nodefaultlib:msvcrt");
            // Link the debug CRT instead
            println!("cargo::rustc-link-arg=/defaultlib:msvcrtd");
        }
    }
}
