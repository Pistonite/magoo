//! Check git version

use semver::{Version, VersionReq};

/// Parse the output of `git --version` to a semver
pub fn parse_git_version(version: &str) -> Option<Version> {
    let version = version.trim().trim_start_matches("git version ");
    let mut parts = version.split(".windows.");
    let cleaned = parts.next().unwrap_or("").trim();
    Version::parse(cleaned).ok()
}

/// The semver notation of the officially supported git versions
pub const SUPPORTED_GIT_VERSIONS: [&str; 3] = [">=2.45.1", "~2.44.1", "~2.43.4"];

/// Get string representation of supported git versions
pub fn get_supported_versions() -> String {
    SUPPORTED_GIT_VERSIONS.join(", ")
}

pub fn is_supported(version: &Version) -> bool {
    SUPPORTED_GIT_VERSIONS
        .iter()
        .any(|v| VersionReq::parse(v).unwrap().matches(version))
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_git_version() {
        assert_eq!(
            parse_git_version("git version 2.40.0"),
            Some(Version::new(2, 40, 0))
        );
        assert_eq!(
            parse_git_version("git version 2.30.1"),
            Some(Version::new(2, 30, 1))
        );
        assert_eq!(
            parse_git_version("git version 2.43.0.windows.1"),
            Some(Version::new(2, 43, 0))
        );
        assert_eq!(parse_git_version("gi version 2.43.0.windows.1"), None);
    }

    #[test]
    fn supported_version_parses() {
        for version in SUPPORTED_GIT_VERSIONS {
            assert!(VersionReq::parse(version).is_ok());
        }
        assert!(is_supported(
            &parse_git_version("git version 2.45.1").unwrap()
        ));
        assert!(!is_supported(
            &parse_git_version("git version 2.45.0").unwrap()
        ));
        assert!(is_supported(
            &parse_git_version("git version 2.44.1").unwrap()
        ));
        assert!(!is_supported(
            &parse_git_version("git version 2.44.0").unwrap()
        ));
        assert!(is_supported(
            &parse_git_version("git version 2.43.4").unwrap()
        ));
        assert!(!is_supported(
            &parse_git_version("git version 2.43.3").unwrap()
        ));
    }
}
