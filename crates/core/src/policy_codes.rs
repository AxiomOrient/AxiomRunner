#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PolicyCode {
    #[default]
    Allowed,
    ActorMissing,
    PayloadTooLarge,
    ConstraintPathScope,
    ConstraintDestructiveCommands,
    ConstraintExternalCommands,
}

pub const POLICY_REJECTION_CODES: [PolicyCode; 5] = [
    PolicyCode::ActorMissing,
    PolicyCode::PayloadTooLarge,
    PolicyCode::ConstraintPathScope,
    PolicyCode::ConstraintDestructiveCommands,
    PolicyCode::ConstraintExternalCommands,
];

impl PolicyCode {
    pub const fn is_rejection(self) -> bool {
        match self {
            PolicyCode::Allowed => false,
            PolicyCode::ActorMissing
            | PolicyCode::PayloadTooLarge
            | PolicyCode::ConstraintPathScope
            | PolicyCode::ConstraintDestructiveCommands
            | PolicyCode::ConstraintExternalCommands => true,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            PolicyCode::Allowed => "allowed",
            PolicyCode::ActorMissing => "actor_missing",
            PolicyCode::PayloadTooLarge => "payload_too_large",
            PolicyCode::ConstraintPathScope => "constraint_path_scope",
            PolicyCode::ConstraintDestructiveCommands => "constraint_destructive_commands",
            PolicyCode::ConstraintExternalCommands => "constraint_external_commands",
        }
    }
}
