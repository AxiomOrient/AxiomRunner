#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PolicyCode {
    #[default]
    Allowed,
    ActorMissing,
    PayloadTooLarge,
}

pub const POLICY_REJECTION_CODES: [PolicyCode; 2] =
    [PolicyCode::ActorMissing, PolicyCode::PayloadTooLarge];

impl PolicyCode {
    pub const fn is_rejection(self) -> bool {
        match self {
            PolicyCode::Allowed => false,
            PolicyCode::ActorMissing | PolicyCode::PayloadTooLarge => true,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            PolicyCode::Allowed => "allowed",
            PolicyCode::ActorMissing => "actor_missing",
            PolicyCode::PayloadTooLarge => "payload_too_large",
        }
    }
}
