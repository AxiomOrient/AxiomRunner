use core::fmt;
use core::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl SchemaVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub fn parse(input: &str) -> Result<Self, ParseLegacySpecError> {
        parse_legacy_spec(input)
    }

    pub fn normalized(self) -> String {
        format!("v{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl FromStr for SchemaVersion {
    type Err = ParseLegacySpecError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_legacy_spec(s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseLegacySpecError {
    Empty,
    InvalidFormat,
    InvalidNumber,
    TooManySegments,
}

impl fmt::Display for ParseLegacySpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "legacy spec is empty"),
            Self::InvalidFormat => write!(f, "legacy spec has invalid format"),
            Self::InvalidNumber => write!(f, "legacy spec contains an invalid number"),
            Self::TooManySegments => write!(f, "legacy spec has too many version segments"),
        }
    }
}

impl std::error::Error for ParseLegacySpecError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LegacyPathScope {
    UserHome,
    Workspace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LegacyPathSpec {
    ConfigToml,
    WorkspaceHint,
    MemorySqlite,
    MemoryRootMarkdown,
    MemoryDailyMarkdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LegacyPathRule {
    pub scope: LegacyPathScope,
    pub spec: LegacyPathSpec,
    pub pattern: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyPath {
    pub scope: LegacyPathScope,
    pub spec: LegacyPathSpec,
    pub normalized: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseLegacyPathError {
    Empty,
    InvalidSegment,
    UnknownScope,
    UnknownSpec,
}

impl fmt::Display for ParseLegacyPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "legacy path is empty"),
            Self::InvalidSegment => write!(f, "legacy path has an invalid segment"),
            Self::UnknownScope => write!(f, "legacy path scope is not supported"),
            Self::UnknownSpec => write!(f, "legacy path spec is not supported"),
        }
    }
}

impl std::error::Error for ParseLegacyPathError {}

const LEGACY_PATH_RULES: [LegacyPathRule; 5] = [
    LegacyPathRule {
        scope: LegacyPathScope::UserHome,
        spec: LegacyPathSpec::ConfigToml,
        pattern: "~/.zeroclaw/config.toml",
    },
    LegacyPathRule {
        scope: LegacyPathScope::UserHome,
        spec: LegacyPathSpec::WorkspaceHint,
        pattern: "~/.zeroclaw/workspace",
    },
    LegacyPathRule {
        scope: LegacyPathScope::Workspace,
        spec: LegacyPathSpec::MemorySqlite,
        pattern: "memory/brain.db",
    },
    LegacyPathRule {
        scope: LegacyPathScope::Workspace,
        spec: LegacyPathSpec::MemoryRootMarkdown,
        pattern: "MEMORY.md",
    },
    LegacyPathRule {
        scope: LegacyPathScope::Workspace,
        spec: LegacyPathSpec::MemoryDailyMarkdown,
        pattern: "memory/*.md",
    },
];

pub fn legacy_path_rules() -> &'static [LegacyPathRule] {
    &LEGACY_PATH_RULES
}

pub fn parse_legacy_path(input: &str) -> Result<LegacyPath, ParseLegacyPathError> {
    let normalized = normalize_legacy_path(input)?;
    let segments = split_legacy_path(&normalized)?;
    let (scope, spec) = classify_legacy_path(&segments)?;

    Ok(LegacyPath {
        scope,
        spec,
        normalized,
    })
}

pub fn normalize_legacy_path(input: &str) -> Result<String, ParseLegacyPathError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ParseLegacyPathError::Empty);
    }

    let replaced = trimmed.replace('\\', "/");
    let raw_segments: Vec<&str> = replaced
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    if raw_segments.is_empty() {
        return Err(ParseLegacyPathError::Empty);
    }

    for segment in &raw_segments {
        if *segment == "." || *segment == ".." {
            return Err(ParseLegacyPathError::InvalidSegment);
        }
    }

    Ok(raw_segments.join("/"))
}

pub fn is_legacy_path(input: &str) -> bool {
    parse_legacy_path(input).is_ok()
}

pub fn parse_legacy_spec(input: &str) -> Result<SchemaVersion, ParseLegacySpecError> {
    let mut s = input.trim();
    if s.is_empty() {
        return Err(ParseLegacySpecError::Empty);
    }

    if let Some(rest) = s.strip_prefix("legacy:") {
        s = rest.trim();
    }

    if let Some(rest) = s.strip_prefix('v').or_else(|| s.strip_prefix('V')) {
        s = rest.trim();
    }

    if s.is_empty() {
        return Err(ParseLegacySpecError::Empty);
    }

    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() > 3 {
        return Err(ParseLegacySpecError::TooManySegments);
    }

    let major = parse_part(parts.first().copied())?;
    let minor = match parts.get(1) {
        Some(raw) => parse_part(Some(raw))?,
        None => 0,
    };
    let patch = match parts.get(2) {
        Some(raw) => parse_part(Some(raw))?,
        None => 0,
    };

    Ok(SchemaVersion::new(major, minor, patch))
}

pub fn normalize_legacy_spec(input: &str) -> Result<String, ParseLegacySpecError> {
    Ok(parse_legacy_spec(input)?.normalized())
}

pub fn is_legacy_spec(input: &str) -> bool {
    parse_legacy_spec(input).is_ok_and(|version| version.major < 2)
}

fn parse_part(raw: Option<&str>) -> Result<u16, ParseLegacySpecError> {
    let Some(value) = raw else {
        return Ok(0);
    };

    if value.trim().is_empty() {
        return Err(ParseLegacySpecError::InvalidFormat);
    }

    value
        .trim()
        .parse::<u16>()
        .map_err(|_| ParseLegacySpecError::InvalidNumber)
}

fn split_legacy_path(normalized: &str) -> Result<Vec<&str>, ParseLegacyPathError> {
    let segments: Vec<&str> = normalized.split('/').collect();
    if segments.is_empty() {
        return Err(ParseLegacyPathError::Empty);
    }
    Ok(segments)
}

fn classify_legacy_path(
    segments: &[&str],
) -> Result<(LegacyPathScope, LegacyPathSpec), ParseLegacyPathError> {
    match segments {
        ["~", ".zeroclaw", "config.toml"] => {
            Ok((LegacyPathScope::UserHome, LegacyPathSpec::ConfigToml))
        }
        ["~", ".zeroclaw", "workspace"] => {
            Ok((LegacyPathScope::UserHome, LegacyPathSpec::WorkspaceHint))
        }
        ["memory", "brain.db"] => Ok((LegacyPathScope::Workspace, LegacyPathSpec::MemorySqlite)),
        ["MEMORY.md"] => Ok((
            LegacyPathScope::Workspace,
            LegacyPathSpec::MemoryRootMarkdown,
        )),
        ["memory", file] if is_daily_markdown_spec(file) => Ok((
            LegacyPathScope::Workspace,
            LegacyPathSpec::MemoryDailyMarkdown,
        )),
        ["~", ".zeroclaw", ..] | ["memory", ..] | ["MEMORY.md", ..] => {
            Err(ParseLegacyPathError::UnknownSpec)
        }
        _ => Err(ParseLegacyPathError::UnknownScope),
    }
}

fn is_daily_markdown_spec(file: &str) -> bool {
    file == "*.md" || (file.ends_with(".md") && file != ".md")
}
