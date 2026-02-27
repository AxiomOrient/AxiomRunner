#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ConfigSource {
    Default,
    File,
    Environment,
    Cli,
}

impl ConfigSource {
    pub const fn priority(self) -> u8 {
        match self {
            Self::Default => 0,
            Self::File => 1,
            Self::Environment => 2,
            Self::Cli => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sourced<T> {
    pub value: T,
    pub source: ConfigSource,
}

impl<T> Sourced<T> {
    pub const fn new(value: T, source: ConfigSource) -> Self {
        Self { value, source }
    }
}

pub fn merge_sources<T: Clone>(sources: &[Sourced<T>]) -> Option<Sourced<T>> {
    let mut effective: Option<Sourced<T>> = None;

    for candidate in sources {
        match &effective {
            Some(current) if current.source.priority() > candidate.source.priority() => {}
            _ => {
                effective = Some(Sourced::new(candidate.value.clone(), candidate.source));
            }
        }
    }

    effective
}

pub fn merge_config_sources<T: Clone>(sources: &[Sourced<T>]) -> Option<Sourced<T>> {
    merge_sources(sources)
}

pub fn merge_optional<T>(
    default: Option<T>,
    file: Option<T>,
    environment: Option<T>,
    cli: Option<T>,
) -> Option<Sourced<T>> {
    let sources = [
        default.map(|value| Sourced::new(value, ConfigSource::Default)),
        file.map(|value| Sourced::new(value, ConfigSource::File)),
        environment.map(|value| Sourced::new(value, ConfigSource::Environment)),
        cli.map(|value| Sourced::new(value, ConfigSource::Cli)),
    ];

    let mut effective: Option<Sourced<T>> = None;
    for candidate in sources.into_iter().flatten() {
        match &effective {
            Some(current) if current.source.priority() > candidate.source.priority() => {}
            _ => {
                effective = Some(candidate);
            }
        }
    }

    effective
}
