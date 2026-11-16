use serde::Deserialize;

use super::version::Version;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    pub tag: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct RawRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<RawAsset>,
}

#[derive(Deserialize)]
struct RawAsset {
    name: String,
    browser_download_url: String,
}

impl Release {
    pub fn parse(json: &[u8]) -> Result<Self> {
        let raw: RawRelease = serde_json::from_slice(json)
            .map_err(|error| Error::Update(format!("unexpected release data: {error}")))?;
        let version = raw.tag_name.parse().map_err(Error::Update)?;
        let assets = raw
            .assets
            .into_iter()
            .map(|asset| Asset {
                name: asset.name,
                url: asset.browser_download_url,
            })
            .collect();
        Ok(Release {
            version,
            tag: raw.tag_name,
            assets,
        })
    }

    pub fn asset_for(&self, arch: &str) -> Result<&Asset> {
        let wanted = asset_name(&self.version, arch);
        self.assets
            .iter()
            .find(|asset| asset.name == wanted)
            .ok_or_else(|| Error::Update(format!("release {} has no {wanted}", self.tag)))
    }
}

pub fn asset_name(version: &Version, arch: &str) -> String {
    format!("glori-{version}-{arch}-unknown-linux-musl.tar.gz")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "tag_name": "v0.2.0",
        "name": "Glori 0.2.0",
        "assets": [
            {"name": "glori-0.2.0-x86_64-unknown-linux-musl.tar.gz", "browser_download_url": "https://h/x86", "size": 1},
            {"name": "glori-0.2.0-aarch64-unknown-linux-musl.tar.gz", "browser_download_url": "https://h/arm", "size": 1},
            {"name": "SHA256SUMS", "browser_download_url": "https://h/sums"}
        ]
    }"#;

    #[test]
    fn parses_a_github_release() {
        let release = Release::parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(release.version.to_string(), "0.2.0");
        assert_eq!(release.tag, "v0.2.0");
        assert_eq!(release.assets.len(), 3);
    }

    #[test]
    fn picks_the_asset_for_the_architecture() {
        let release = Release::parse(SAMPLE.as_bytes()).unwrap();
        assert_eq!(release.asset_for("x86_64").unwrap().url, "https://h/x86");
        assert_eq!(release.asset_for("aarch64").unwrap().url, "https://h/arm");
    }

    #[test]
    fn a_missing_architecture_is_an_error() {
        let release = Release::parse(SAMPLE.as_bytes()).unwrap();
        assert!(
            release
                .asset_for("riscv64")
                .unwrap_err()
                .to_string()
                .contains("riscv64")
        );
    }

    #[test]
    fn a_release_without_assets_is_still_parsed() {
        let release = Release::parse(br#"{"tag_name":"v1.0.0"}"#).unwrap();
        assert!(release.assets.is_empty());
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(Release::parse(b"<html>").is_err());
        assert!(Release::parse(br#"{"tag_name":"nightly"}"#).is_err());
        assert!(Release::parse(br#"{"message":"Not Found"}"#).is_err());
    }
}
