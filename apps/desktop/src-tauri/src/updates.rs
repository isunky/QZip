use std::sync::Once;

use serde::{Deserialize, Serialize};

use super::CommandErrorDto;

pub(super) const UPDATE_API_URL: &str = "https://api.github.com/repos/isunky/QZip/releases/latest";
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
    pub(super) release_notes: String,
    pub(super) release_tag: String,
    pub(super) download_size: Option<u64>,
    pub(super) download_available: bool,
    pub(super) manual_download_url: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct GitHubRelease {
    pub(super) tag_name: String,
    pub(super) name: Option<String>,
    pub(super) published_at: Option<String>,
    #[serde(default)]
    pub(super) body: Option<String>,
    #[serde(default)]
    pub(super) assets: Vec<ReleaseAsset>,
    #[serde(default)]
    pub(super) draft: bool,
    #[serde(default)]
    pub(super) prerelease: bool,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReleaseAsset {
    pub(super) name: String,
    pub(super) browser_download_url: String,
    pub(super) size: u64,
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

pub(super) fn valid_release_tag(tag: &str) -> bool {
    parse_release_version(tag)
        .is_some_and(|(version, _)| tag == version || tag == format!("v{version}"))
}

pub(super) fn release_download_assets(
    release: &GitHubRelease,
) -> Result<(&ReleaseAsset, &ReleaseAsset), CommandErrorDto> {
    if !valid_release_tag(&release.tag_name) || release.draft || release.prerelease {
        return Err(update_check_error(
            "UPDATE_INVALID_RELEASE",
            "无法下载该版本。",
        ));
    }
    let expected_name = format!("QZip-{}-windows-x64-setup.exe", release.tag_name);
    let find = |name: &str| {
        release.assets.iter().find(|asset| {
            asset.name == name
                && asset.size > 0
                && asset.size <= 512 * 1024 * 1024
                && asset.browser_download_url
                    == format!(
                        "https://github.com/isunky/QZip/releases/download/{}/{name}",
                        release.tag_name
                    )
        })
    };
    let setup = find(&expected_name).ok_or_else(|| {
        update_check_error(
            "UPDATE_ASSET_MISSING",
            "此版本尚未提供 Windows x64 安装包，请稍后重试。",
        )
    })?;
    let checksum = find("checksums-sha256.txt").ok_or_else(|| {
        update_check_error(
            "UPDATE_CHECKSUM_MISSING",
            "此版本缺少校验文件，暂时无法在应用内下载。",
        )
    })?;
    Ok((setup, checksum))
}

pub(super) fn http_client() -> Result<reqwest::Client, CommandErrorDto> {
    TLS_PROVIDER_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    reqwest::Client::builder()
        .user_agent(format!("QZip/{}", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(UPDATE_REQUEST_TIMEOUT)
        .read_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(30 * 60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            let allowed = attempt.url().scheme() == "https"
                && matches!(
                    attempt.url().host_str(),
                    Some(
                        "github.com"
                            | "api.github.com"
                            | "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                    )
                );
            if !allowed || attempt.previous().len() >= 5 {
                attempt.error("Untrusted update redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| update_check_error("UPDATE_NETWORK", "无法初始化更新连接。"))
}

pub(super) async fn get_response(
    client: &reqwest::Client,
    url: &str,
    metadata: bool,
) -> Result<reqwest::Response, CommandErrorDto> {
    let mut request = client.get(url);
    if metadata {
        request = request.timeout(std::time::Duration::from_secs(20));
    }
    let response = request.send().await.map_err(|_| {
        update_check_error(
            "UPDATE_NETWORK",
            "连接 GitHub 失败或超时，请检查网络后重试。",
        )
    })?;
    if !response.status().is_success() {
        return Err(update_check_error(
            "UPDATE_HTTP",
            if matches!(response.status().as_u16(), 403 | 429) {
                "GitHub 请求次数已达到限制，请稍后重试。".into()
            } else {
                format!(
                    "GitHub 返回 HTTP {}，请稍后重试。",
                    response.status().as_u16()
                )
            },
        ));
    }
    Ok(response)
}

pub(super) async fn read_limited(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, CommandErrorDto> {
    let mut data = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| update_check_error("UPDATE_NETWORK", "下载中断，请重试。"))?
    {
        if data.len() + chunk.len() > limit {
            return Err(update_check_error(
                "UPDATE_INVALID_RESPONSE",
                "更新信息超过允许大小。",
            ));
        }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

pub(super) async fn fetch_release(tag: Option<&str>) -> Result<GitHubRelease, CommandErrorDto> {
    let url = match tag {
        Some(tag) if valid_release_tag(tag) => {
            format!("https://api.github.com/repos/isunky/QZip/releases/tags/{tag}")
        }
        Some(_) => return Err(update_check_error("UPDATE_INVALID_RELEASE", "版本号无效。")),
        None => UPDATE_API_URL.to_owned(),
    };
    let response = get_response(&http_client()?, &url, true).await?;
    serde_json::from_slice(&read_limited(response, 2 * 1024 * 1024).await?)
        .map_err(|_| update_check_error("UPDATE_INVALID_RESPONSE", "无法读取 GitHub 版本信息。"))
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
    if release.draft || release.prerelease {
        return Err(update_check_error(
            "UPDATE_CHECK_INVALID_RESPONSE",
            "该版本不是正式发布版本。",
        ));
    }
    let assets = release_download_assets(&release).ok();
    let download_size = if cfg!(target_os = "windows") {
        assets.as_ref().map(|(setup, _)| setup.size)
    } else {
        None
    };
    let manual_download_url =
        mac_download_url(&release, std::env::consts::OS, std::env::consts::ARCH);
    let download_available =
        cfg!(all(target_os = "windows", target_arch = "x86_64")) && assets.is_some();
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
        release_url: format!(
            "https://github.com/isunky/QZip/releases/tag/{}",
            release.tag_name
        ),
        release_name: release.name,
        published_at: release.published_at,
        release_notes: release.body.unwrap_or_default(),
        release_tag: release.tag_name,
        download_size,
        download_available,
        manual_download_url,
    })
}

fn mac_download_url(release: &GitHubRelease, os: &str, arch: &str) -> Option<String> {
    if os != "macos" {
        return None;
    }
    let arch = match arch {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        _ => return None,
    };
    let name = format!("QZip-{}-macos-{arch}.dmg", release.tag_name);
    let url = format!(
        "https://github.com/isunky/QZip/releases/download/{}/{name}",
        release.tag_name
    );
    release
        .assets
        .iter()
        .find(|asset| asset.name == name && asset.browser_download_url == url)
        .map(|_| url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_download_matches_architecture_and_trusted_exact_url() {
        let url =
            "https://github.com/isunky/QZip/releases/download/v1.3.0/QZip-v1.3.0-macos-arm64.dmg";
        let mut release = GitHubRelease {
            tag_name: "v1.3.0".into(),
            assets: vec![ReleaseAsset {
                name: "QZip-v1.3.0-macos-arm64.dmg".into(),
                browser_download_url: url.into(),
                size: 100,
            }],
            ..Default::default()
        };
        assert_eq!(
            mac_download_url(&release, "macos", "aarch64"),
            Some(url.into())
        );
        assert!(mac_download_url(&release, "macos", "x86_64").is_none());
        assert!(mac_download_url(&release, "windows", "aarch64").is_none());
        release.assets[0].browser_download_url = "https://example.com/installer.dmg".into();
        assert!(mac_download_url(&release, "macos", "aarch64").is_none());
    }

    #[test]
    fn only_accepts_official_assets_for_the_selected_stable_version() {
        let mut release = GitHubRelease {
            tag_name: "v1.2.0".into(),
            assets: ["QZip-v1.2.0-windows-x64-setup.exe", "checksums-sha256.txt"]
                .into_iter()
                .map(|name| ReleaseAsset {
                    name: name.into(),
                    size: 100,
                    browser_download_url: format!(
                        "https://github.com/isunky/QZip/releases/download/v1.2.0/{name}"
                    ),
                })
                .collect(),
            ..Default::default()
        };
        assert!(release_download_assets(&release).is_ok());
        release.prerelease = true;
        assert!(release_download_assets(&release).is_err());
        release.prerelease = false;
        release.assets[0].browser_download_url = "https://example.com/setup.exe".into();
        assert!(release_download_assets(&release).is_err());
        for tag in ["../v1.2.0", "v1.2.0-rc.1", "v1.2.0?download=1"] {
            assert!(!valid_release_tag(tag));
        }
    }

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
                ..Default::default()
            },
        )
        .expect("release parses");
        assert_eq!(result.status, "update_available");
        assert_eq!(result.latest_version, "1.2.0");
        assert_eq!(
            result.release_url,
            "https://github.com/isunky/QZip/releases/tag/v1.2.0"
        );
    }

    #[test]
    fn reports_when_already_on_latest_release() {
        let result = update_result_for_release(
            "1.1.2",
            GitHubRelease {
                tag_name: "1.1.2".to_owned(),
                name: None,
                published_at: None,
                ..Default::default()
            },
        )
        .expect("release parses");
        assert_eq!(result.status, "up_to_date");
        assert_eq!(result.current_version, result.latest_version);
    }
}
