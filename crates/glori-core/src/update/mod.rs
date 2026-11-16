mod apply;
mod release;
mod version;

use crate::{
    download::{self, Download, Progress},
    error::Result,
};

pub use apply::{Origin, extract_binary, origin_of, replace};
pub use release::{Asset, Release, asset_name};
pub use version::Version;

pub const RELEASES_URL: &str = "https://api.github.com/repos/pozii/glori/releases/latest";
const MAX_METADATA_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    UpToDate,
    Available(Release),
}

pub fn latest(url: &str) -> Result<Release> {
    let body = download::get(url, MAX_METADATA_BYTES)?;
    Release::parse(&body)
}

pub fn check(current: &Version, url: &str) -> Result<Status> {
    let release = latest(url)?;
    if release.version > *current {
        Ok(Status::Available(release))
    } else {
        Ok(Status::UpToDate)
    }
}

pub fn fetch(release: &Release, arch: &str, progress: Progress) -> Result<Download> {
    let asset = release.asset_for(arch)?;
    download::fetch(&asset.url, progress, false)
}
