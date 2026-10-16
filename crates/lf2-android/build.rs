//! Link the NDK's static C++ library for the C++ of oboe (cpal's Android sound),
//! after the objects that use it: oboe-sys asks for it before them, which leaves
//! symbols such as `__cxa_pure_virtual` unresolved when Android loads the library.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-lib=static=c++_static");
        println!("cargo:rustc-link-lib=static=c++abi");
    }
}
