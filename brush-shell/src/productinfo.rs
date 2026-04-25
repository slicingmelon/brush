//! Information about this shell project.

/// The formal name of this product.
pub const PRODUCT_NAME: &str = "brush";

const PRODUCT_HOMEPAGE: &str = env!("CARGO_PKG_HOMEPAGE");
const PRODUCT_REPO: &str = env!("CARGO_PKG_REPOSITORY");

/// The URI to display as the product's homepage.
#[allow(clippy::const_is_empty)]
pub const PRODUCT_DISPLAY_URI: &str = if !PRODUCT_HOMEPAGE.is_empty() {
    PRODUCT_HOMEPAGE
} else {
    PRODUCT_REPO
};

/// The version of the product, in string form.
pub const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Info regarding the specific version of sources used to build this product.
pub const PRODUCT_GIT_VERSION: &str = git_version::git_version!(
    prefix = "git:",
    cargo_prefix = "cargo:",
    fallback = "unknown:",
    args = ["--always", "--dirty=-modified", "--match", ""]
);

/// Returns the file stem of the currently running executable (e.g. `bash`
/// when this binary was installed/renamed to `bash[.exe]`), falling back to
/// [`PRODUCT_NAME`] when the executable path is unavailable or empty.
pub fn invoked_name() -> String {
    std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(std::path::Path::file_stem)
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| PRODUCT_NAME.to_string())
}

/// Returns the display name to use in version banners. When the binary is
/// invoked under its canonical name, returns just `"brush"`. When invoked
/// under an alias (e.g. `bash`), returns `"bash (brush)"` so the underlying
/// implementation is still discoverable.
pub fn display_name() -> String {
    let invoked = invoked_name();
    if invoked == PRODUCT_NAME {
        PRODUCT_NAME.to_string()
    } else {
        std::format!("{invoked} ({PRODUCT_NAME})")
    }
}

pub(crate) fn get_product_display_str() -> String {
    let name = display_name();
    std::format!(
        "{name} version {PRODUCT_VERSION} ({PRODUCT_GIT_VERSION}) - {PRODUCT_DISPLAY_URI}"
    )
}
