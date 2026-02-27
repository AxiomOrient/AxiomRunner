use axiom_core::ExecutionMode;

pub fn mode_name(mode: ExecutionMode) -> &'static str {
    match mode {
        ExecutionMode::Active => "active",
        ExecutionMode::ReadOnly => "read_only",
        ExecutionMode::Halted => "halted",
    }
}
