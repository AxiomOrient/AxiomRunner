#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PolicyCode {
    #[default]
    Allowed,
    ActorMissing,
    RuntimeHalted,
    ReadOnlyMutation,
    UnauthorizedControl,
    PayloadTooLarge,
}

pub const POLICY_REJECTION_CODES: [PolicyCode; 5] = [
    PolicyCode::ActorMissing,
    PolicyCode::RuntimeHalted,
    PolicyCode::ReadOnlyMutation,
    PolicyCode::UnauthorizedControl,
    PolicyCode::PayloadTooLarge,
];

impl PolicyCode {
    pub const fn is_rejection(self) -> bool {
        match self {
            PolicyCode::Allowed => false,
            PolicyCode::ActorMissing
            | PolicyCode::RuntimeHalted
            | PolicyCode::ReadOnlyMutation
            | PolicyCode::UnauthorizedControl
            | PolicyCode::PayloadTooLarge => true,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            PolicyCode::Allowed => "allowed",
            PolicyCode::ActorMissing => "actor_missing",
            PolicyCode::RuntimeHalted => "runtime_halted",
            PolicyCode::ReadOnlyMutation => "readonly_mutation",
            PolicyCode::UnauthorizedControl => "unauthorized_control",
            PolicyCode::PayloadTooLarge => "payload_too_large",
        }
    }
}
