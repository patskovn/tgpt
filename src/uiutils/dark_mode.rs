extern crate cocoa;
extern crate objc;

#[cfg(target_os = "macos")]
extern "C" {
    pub static NSAppearanceNameDarkAqua: cocoa::base::id;
}

#[cfg(target_os = "macos")]
pub fn is_dark_mode() -> bool {
    use cocoa::appkit::NSApp;
    use cocoa::base::id;
    use objc::sel;
    use objc::sel_impl;

    unsafe {
        let app = NSApp();
        let appearance: id = objc::msg_send![app, effectiveAppearance];
        let appearance_name: id = objc::msg_send![appearance, name];
        let is_dark_mode: cocoa::base::BOOL =
            objc::msg_send![appearance_name, isEqualToString:NSAppearanceNameDarkAqua];

        is_dark_mode
    }
}

#[cfg(not(target_os = "macos"))]
pub fn is_dark_mode() -> bool {
    false
}
