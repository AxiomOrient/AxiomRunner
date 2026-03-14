pub fn answer() -> u32 { 42 }

#[cfg(test)]
mod tests {
    #[test]
    fn ok() {
        assert_eq!(super::answer(), 42);
    }
}
