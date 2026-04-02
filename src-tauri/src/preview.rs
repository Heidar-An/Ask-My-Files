pub fn preview_path_for_kind(path: &str, kind: &str, extension: &str) -> Option<String> {
    if kind == "image" || extension.eq_ignore_ascii_case("pdf") {
        Some(path.to_string())
    } else {
        None
    }
}
