use std::sync::Once;

use serde::{Deserialize, Serialize};

use super::CommandErrorDto;

pub(super) const UPDATE_API_URL: &str = "https://api.github.com/repos/isunky/QZip/releases/latest";
pub(super) const UPDATE_RELEASE_URL: &str = "https://github.com/isunky/QZip/releases/latest";
pub(super) const UPDATE_REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
pub(super) static TLS_PROVIDER_INIT: Once = Once::new();

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateCheckResult {
    pub(super) configured: bool,
    pub(super) status: String,
    pub(super) current_version: String,
    pub(super) latest_version: String,
    pub(super) release_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) release_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GitHubRelease {
    pub(super) tag_name: String,
    pub(super) name: Option<String>,
    pub(super) published_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
struct ReleaseVersion {
    major: u64,
    minor: u64,
    patch: u64,
}

fn parse_release_version(value: &str) -> Option<(String, ReleaseVersion)> {
    let value = value.trim();
    let value = value
        .strip_prefix('v')
        .or_else(|| value.strip_prefix('V'))
        .unwrap_or(value);
    let core = value.split(['-', '+']).next()?.trim();
    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let version = ReleaseVersion {
        major: parts[0].parse().ok()?,
        minor: parts[1].parse().ok()?,
        patch: parts[2].parse().ok()?,
    };
    Some((
        format!("{}.{}.{}", version.major, version.minor, version.patch),
        version,
    ))
}

pub(super) fn update_check_error(code: &str, message: impl Into<String>) -> CommandErrorDto {
    CommandErrorDto {
        code: code.to_owned(),
        message: message.into(),
        recoverable: true,
    }
}

pub(super) fn update_result_for_release(
    current_version: &str,
    release: GitHubRelease,
) -> Result<UpdateCheckResult, CommandErrorDto> {
    let Some((current_version, current)) = parse_release_version(current_version) else {
        return Err(update_check_error(
            "UPDATE_CHECK_INVALID_VERSION",
            "当前应用版本号无效，无法比较更新。",
        ));
    };
    let Some((latest_version, latest)) = parse_release_version(&release.tag_name) else {
        return Err(update_check_error(
            "UPDATE_CHECK_INVALID_RESPONSE",
            "GitHub 返回的版本号无效。",
        ));
    };
    Ok(UpdateCheckResult {
        configured: true,
        status: if latest > current {
            "update_available"
        } else {
            "up_to_date"
        }
        .to_owned(),
        current_version,
        latest_version,
        release_url: UPDATE_RELEASE_URL.to_owned(),
        release_name: release.name,
        published_at: release.published_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_v_prefixed_release_versions() {
        let (display, version) = parse_release_version("v1.2.3").expect("version parses");
        assert_eq!(display, "1.2.3");
        assert_eq!(
            version,
            ReleaseVersion {
                major: 1,
                minor: 2,
                patch: 3
            }
        );
    }

    #[test]
    fn rejects_malformed_release_versions() {
        assert!(parse_release_version("latest").is_none());
        assert!(parse_release_version("1.2").is_none());
        assert!(parse_release_version("1.2.3.4").is_none());
    }

    #[test]
    fn reports_when_github_release_is_newer() {
        let result = update_result_for_release(
            "1.1.2",
            GitHubRelease {
                tag_name: "v1.2.0".to_owned(),
                name: Some("QZip v1.2.0".to_owned()),
                published_at: Some("2026-09-01T00:00:00Z".to_owned()),
            },
        )
        .expect("release parses");
        assert_eq!(result.status, "update_available");
        assert_eq!(result.latest_version, "1.2.0");
        assert_eq!(result.release_url, UPDATE_RELEASE_URL);
    }

    #[test]
    fn reports_when_already_on_latest_release() {
        let result = update_result_for_release(
            "1.1.2",
            GitHubRelease {
                tag_name: "1.1.2".to_owned(),
                name: None,
                published_at: None,
            },
        )
        .expect("release parses");
        assert_eq!(result.status, "up_to_date");
        assert_eq!(result.current_version, result.latest_version);
    }
}
