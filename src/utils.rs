#![allow(dead_code)]
use join_futures::join_futures;
use once_cell::sync::Lazy;
use quickemu::config::Arch;
use quickget_core::data_structures::ArchiveFormat;
use reqwest::{StatusCode, Url};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::Semaphore;

pub async fn capture_page(input: &str) -> Option<String> {
    let url: Url = input.parse().ok()?;
    let url_permit = match CLIENT.url_permits.get(url.host_str()?) {
        Some(semaphore) => Some(semaphore.acquire().await.ok()?),
        None => None,
    };

    let permit = CLIENT.semaphore.acquire().await.ok()?;
    let response = CLIENT.client.get(url).send().await.ok()?;

    let status = response.status();
    let output = if status.is_success() {
        response.text().await.ok().filter(|text| !text.is_empty())
    } else {
        log::warn!("Failed to capture page: {}, {}", input, status);
        None
    };

    drop(permit);
    if let Some(url_permit) = url_permit {
        drop(url_permit);
    }
    output
}

pub async fn all_valid(urls: Vec<String>) -> bool {
    let futures = urls.into_iter().map(|input| async move {
        let url: Url = input.parse().ok()?;
        let url_permit = match CLIENT.url_permits.get(url.host_str()?) {
            Some(semaphore) => Some(semaphore.acquire().await.ok()?),
            None => None,
        };
        let permit = CLIENT.semaphore.acquire().await.ok()?;

        let response = CLIENT
            .client
            .get(url)
            .send()
            .await
            .inspect_err(|e| {
                log::error!("Failed to make request to URL {}: {}", input, e);
            })
            .ok()?;
        let status = response.status();
        let successful = status.is_success() || status == StatusCode::TOO_MANY_REQUESTS;

        if !successful {
            log::warn!("Failed to resolve URL {}: {}", input, status);
        }
        drop(permit);
        if let Some(url_permit) = url_permit {
            drop(url_permit);
        }
        Some(successful)
    });
    join_futures!(futures).into_iter().all(|r| r.unwrap_or(true))
}

pub fn arch_from_str(arch: &str) -> Option<Arch> {
    match arch {
        "x86_64" | "amd64" => Some(Arch::x86_64),
        "aarch64" | "arm64" => Some(Arch::aarch64),
        "riscv64" | "riscv" => Some(Arch::riscv64),
        _ => None,
    }
}

struct ReqwestClient {
    client: ClientWithMiddleware,
    semaphore: Semaphore,
    url_permits: HashMap<&'static str, Semaphore>,
}

static CLIENT: Lazy<ReqwestClient> = Lazy::new(|| {
    let retries = ExponentialBackoff::builder().build_with_max_retries(3);
    let client = reqwest::ClientBuilder::new().user_agent("quickemu-rs/1.0").build().unwrap();
    let client = ClientBuilder::new(client)
        .with(RetryTransientMiddleware::new_with_policy(retries))
        .build();
    let semaphore = Semaphore::new(150);
    let url_permits = HashMap::from([("sourceforge.net", Semaphore::new(5))]);
    ReqwestClient { client, semaphore, url_permits }
});

pub trait GatherData {
    type Output;
    async fn gather_data(url: &str) -> Option<Self::Output>;
}

pub struct GithubAPI;
impl GatherData for GithubAPI {
    type Output = Vec<GithubAPIValue>;
    async fn gather_data(url: &str) -> Option<Self::Output> {
        let data = capture_page(url).await?;
        serde_json::from_str(&data).ok()
    }
}
#[derive(Deserialize)]
pub struct GithubAPIValue {
    pub tag_name: String,
    pub assets: Vec<GithubAsset>,
    pub prerelease: bool,
    pub body: String,
}
#[derive(Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
}

impl GatherData for FedoraRelease {
    type Output = Vec<FedoraRelease>;
    async fn gather_data(url: &str) -> Option<Self::Output> {
        let data = capture_page(url).await?;
        serde_json::from_str(&data).ok()
    }
}

#[derive(Deserialize)]
pub struct FedoraRelease {
    #[serde(rename = "version")]
    pub release: String,
    pub arch: String,
    pub link: String,
    #[serde(rename = "subvariant")]
    pub edition: String,
    pub sha256: Option<String>,
    // This is not contained within Fedora's data, we'll add it ourselves based on the file extension
    pub archive_format: Option<ArchiveFormat>,
}

#[macro_export]
macro_rules! spawn_distros {
    ($( $distro:ty ),* $(,)? ) => {{
        let mut handles = Vec::new();
        $(
            let handle = spawn(<$distro>::to_os());
            handles.push(handle);
        )*
        handles
    }};
}
