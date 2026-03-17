use axiomrunner_adapters::validate_run_command_spec;
const PRODUCT_COMMAND_ALLOWLIST: &[&str] = &[
    "pwd", "git", "cargo", "npm", "node", "python", "python3", "pytest", "rg", "ls", "cat", "pnpm",
    "yarn", "uv", "make",
];

pub(crate) fn product_command_allowlist() -> Vec<String> {
    PRODUCT_COMMAND_ALLOWLIST
        .iter()
        .map(|value| (*value).to_owned())
        .collect()
}

pub(crate) fn effective_command_allowlist(
    configured_allowlist: Option<&Vec<String>>,
) -> Vec<String> {
    configured_allowlist
        .cloned()
        .unwrap_or_else(product_command_allowlist)
}

pub(crate) fn validate_run_command_contract(
    program: &str,
    args: &[String],
    allowlist: &[String],
) -> Result<(), &'static str> {
    validate_run_command_spec(program, args, allowlist)
}
