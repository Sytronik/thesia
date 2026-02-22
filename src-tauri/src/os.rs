#[inline(always)]
pub const fn os_label<'a>(macos: &'a str, other: &'a str) -> &'a str {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        macos
    } else {
        other
    }
}
