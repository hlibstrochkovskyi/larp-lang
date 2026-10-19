//! Explicit tooling contracts; a version string is not a binary identity.

const SCHEMA_VERSION: u32 = 1;
const LANGUAGE_SPEC: &str = "0.1";

fn channel(version: &str) -> &'static str {
    let version = version.split('+').next().unwrap();
    match version.split_once('-').map(|(_, prerelease)| prerelease) {
        None => "release",
        Some("dev") => "development",
        Some(pre) if pre.starts_with("dev.") => "development",
        Some(pre) if pre.starts_with("preview.") => "preview",
        Some(_) => "prerelease",
    }
}

pub fn json() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let readable = larp_artifact::SUPPORTED_VERSIONS
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(",");
    // Cargo validates the version's ASCII SemVer alphabet; no JSON escapes occur.
    format!(
        "{{\"schema_version\":{SCHEMA_VERSION},\"name\":\"larp\",\"version\":\"{version}\",\"channel\":\"{}\",\"language_spec\":\"{LANGUAGE_SPEC}\",\"language_status\":\"experimental\",\"artifact\":{{\"write\":{},\"read\":[{readable}]}},\"diagnostics_schema\":{}}}",
        channel(version),
        larp_artifact::VERSION,
        larp_diagnostics::json::SCHEMA_VERSION,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_uses_only_the_semver_prerelease_component() {
        for (version, expected) in [
            ("0.1.0-dev", "development"),
            ("0.1.0-dev.7+local", "development"),
            ("0.1.0-preview.1", "preview"),
            ("0.1.0-rc.1", "prerelease"),
            ("0.1.0-device", "prerelease"),
            ("0.1.0+preview.1", "release"),
        ] {
            assert_eq!(channel(version), expected, "{version}");
        }
    }
}
