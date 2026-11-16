//! The Android app. The APK carries the game folder as assets (`game/`, listed in
//! `game/files.txt`); on the first start, and after an update changes that list, the
//! files are copied into the app's internal storage, which is then the game folder.
//! The game runs full screen in landscape with the on-screen controls of
//! `lf2_app::touch`, and reads controllers (`input`).

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod codes;

#[cfg(target_os = "android")]
mod app;
#[cfg(target_os = "android")]
mod input;
