//! Shared, tool-agnostic library. Must never depend on tauri.

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
