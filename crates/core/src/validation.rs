/// Validates that `value` is not blank (empty or whitespace-only).
/// Returns `Err(error)` if blank, `Ok(())` otherwise.
pub(crate) fn ensure_not_blank<E>(value: &str, error: E) -> Result<(), E> {
    if value.trim().is_empty() {
        Err(error)
    } else {
        Ok(())
    }
}
