use crate::utils::all_valid;
use join_futures::join_futures;
use once_cell::sync::Lazy;
pub use quickemu::config::Arch;
pub use quickget_core::data_structures::{ArchiveFormat, Config, Disk, Source, WebSource, OS};
use regex::Regex;
use std::{collections::HashMap, sync::Arc};

pub trait Distro {
    const NAME: &'static str;
    const PRETTY_NAME: &'static str;
    const HOMEPAGE: Option<&'static str>;
    const DESCRIPTION: Option<&'static str>;
    async fn generate_configs() -> Option<Vec<Config>>;
}

pub trait ToOS {
    #![allow(dead_code)]
    async fn to_os() -> Option<OS>;
}

impl<T: Distro + Send> ToOS for T {
    async fn to_os() -> Option<OS> {
        // Any entry containing a URL which isn't reachable needs to be removed
        let Some(releases) = Self::generate_configs().await else {
            log::error!("Failed to generate configs for {}", Self::PRETTY_NAME);
            return None;
        };
        if releases.is_empty() {
            log::error!("No releases found for {}", Self::PRETTY_NAME);
            return None;
        }
        let futures = releases.iter().map(|r| {
            let urls = [&r.iso, &r.img, &r.fixed_iso, &r.floppy]
                .into_iter()
                .flatten()
                .flat_map(filter_web_sources)
                .chain(extract_disk_urls(r.disk_images.as_deref()));
            async move { all_valid(urls).await }
        });
        let results = join_futures!(futures);
        let releases = releases
            .into_iter()
            .zip(results)
            .filter_map(|(config, valid)| {
                if valid {
                    Some(config)
                } else {
                    log::warn!(
                        "Removing {} {} {} {} due to unresolvable URL",
                        Self::PRETTY_NAME,
                        config.release,
                        config.edition.unwrap_or_default(),
                        config.arch
                    );
                    None
                }
            })
            .collect::<Vec<Config>>();

        Some(OS {
            name: Self::NAME.into(),
            pretty_name: Self::PRETTY_NAME.into(),
            homepage: Self::HOMEPAGE.map(Into::into),
            description: Self::DESCRIPTION.map(Into::into),
            releases,
        })
    }
}

pub fn filter_web_sources<'a, S>(sources: S) -> impl Iterator<Item = &'a str>
where
    S: IntoIterator<Item = &'a Source>,
{
    sources.into_iter().filter_map(|s| match s {
        Source::Web(w) => Some(w.url.as_str()),
        _ => None,
    })
}

pub fn extract_disk_urls(disks: Option<&[Disk]>) -> impl Iterator<Item = &str> {
    disks
        .into_iter()
        .map(|disks| disks.iter().map(|d| &d.source))
        .flat_map(filter_web_sources)
}

pub static DEFAULT_SHA256_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r#"SHA256 \(([^)]+)\) = ([0-9a-f]+)"#).unwrap());
pub static DEFAULT_MD5_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r#"MD5 \(([^)]+)\) = ([0-9a-f]+)"#).unwrap());

pub enum ChecksumSeparation {
    Whitespace,
    Sha256Regex,
    Md5Regex,
    CustomRegex(Arc<Regex>, usize, usize),
}

impl ChecksumSeparation {
    pub async fn build(self, url: &str) -> Option<HashMap<String, String>> {
        let data = crate::utils::capture_page(url).await?;
        Some(self.build_with_data(&data))
    }
    pub fn build_with_data(self, data: &str) -> HashMap<String, String> {
        match self {
            Self::Whitespace => data
                .lines()
                .filter_map(|l| {
                    l.split_once(' ')
                        .map(|(hash, file)| (file.trim().to_string(), hash.trim().to_string()))
                })
                .collect(),
            Self::Md5Regex => DEFAULT_MD5_REGEX
                .captures_iter(data)
                .map(|c| (c[1].to_string(), c[2].to_string()))
                .collect(),
            Self::Sha256Regex => DEFAULT_SHA256_REGEX
                .captures_iter(data)
                .map(|c| (c[1].to_string(), c[2].to_string()))
                .collect(),
            Self::CustomRegex(regex, keyindex, valueindex) => regex
                .captures_iter(data)
                .map(|c| (c[keyindex].to_string(), c[valueindex].to_string()))
                .collect(),
        }
    }
}
