pub fn preview_path_for_kind(path: &str, kind: &str) -> Option<String> {
    if kind == "image" {
        Some(path.to_string())
    } else {
        None
    }
}
