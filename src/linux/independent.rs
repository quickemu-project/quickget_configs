use std::sync::Arc;

use crate::{
    store_data::{ArchiveFormat, ChecksumSeparation, Config, Distro, Source, WebSource},
    utils::{arch_from_str, capture_page},
};
use quickemu::config::Arch;
use regex::Regex;
use serde::Deserialize;

const NIX_URL: &str = "https://nix-channels.s3.amazonaws.com/?delimiter=/";
const NIX_DOWNLOAD_URL: &str = "https://channels.nixos.org";

pub struct NixOS;
impl Distro for NixOS {
    const NAME: &'static str = "nixos";
    const PRETTY_NAME: &'static str = "NixOS";
    const HOMEPAGE: Option<&'static str> = Some("https://nixos.org/");
    const DESCRIPTION: Option<&'static str> = Some("Linux distribution based on Nix package manager, tool that takes a unique approach to package management and system configuration.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let releases = capture_page(NIX_URL).await?;
        let releases: NixReleases = quick_xml::de::from_str(&releases).ok()?;

        let standard_release = Regex::new(r#"nixos-(([0-9]+.[0-9]+|(unstable))(?:-small)?)"#).unwrap();
        let iso_regex = Regex::new(r#"latest-nixos-([^-]+)-([^-]+)-linux.iso"#).unwrap();

        let releases: Vec<String> = releases
            .contents
            .into_iter()
            .map(|r| r.key)
            .filter(|r| standard_release.is_match(r))
            .rev()
            .take(6)
            .map(|r| standard_release.captures(&r).unwrap().get(1).unwrap().as_str().to_string())
            .collect();
        let mut futures = Vec::new();
        for release in releases {
            if let Some(page) = capture_page(&format!("{NIX_URL}&prefix=nixos-{release}/"))
                .await
                .and_then(|p| quick_xml::de::from_str::<NixReleases>(&p).ok())
            {
                let page = page
                    .contents
                    .into_iter()
                    .map(|r| r.key)
                    .filter(|r| iso_regex.is_match(r) && r.ends_with(".iso"))
                    .collect::<Vec<String>>();

                futures.append(
                    &mut page
                        .into_iter()
                        .map(|page| {
                            let release = release.clone();
                            let (name, [edition, arch]) = iso_regex.captures(&page).unwrap().extract();
                            let edition = edition.to_string();
                            let arch = arch_from_str(arch);
                            let url = format!("{NIX_DOWNLOAD_URL}/nixos-{release}/{name}");
                            async move {
                                let hash = capture_page(&format!("{url}.sha256"))
                                    .await
                                    .map(|h| h.split_whitespace().next().unwrap().to_string());
                                Some(Config {
                                    release,
                                    edition: Some(edition),
                                    arch: arch?,
                                    iso: Some(vec![Source::Web(WebSource::new(url, hash, None, None))]),
                                    ..Default::default()
                                })
                            }
                        })
                        .collect(),
                );
            };
        }
        futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<Config>>()
            .into()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NixReleases {
    contents: Vec<NixRelease>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NixRelease {
    key: String,
}

const ALPINE_MIRROR: &str = "https://dl-cdn.alpinelinux.org/alpine/";

pub struct Alpine;
impl Distro for Alpine {
    const NAME: &'static str = "alpine";
    const PRETTY_NAME: &'static str = "Alpine Linux";
    const HOMEPAGE: Option<&'static str> = Some("https://alpinelinux.org/");
    const DESCRIPTION: Option<&'static str> = Some("Security-oriented, lightweight Linux distribution based on musl libc and busybox.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let releases = capture_page(ALPINE_MIRROR).await?;
        let releases_regex = Regex::new(r#"<a href="(v[0-9]+\.[0-9]+)/""#).unwrap();
        let iso_regex = Arc::new(Regex::new(r#"(?s)iso: (alpine-virt-[0-9]+\.[0-9]+.*?.iso).*? sha256: ([0-9a-f]+)"#).unwrap());

        let futures = releases_regex.captures_iter(&releases).flat_map(|r| {
            let release = r[1].to_string();
            [Arch::x86_64, Arch::aarch64]
                .iter()
                .map(|arch| {
                    let release = release.clone();
                    let mirror = format!("{ALPINE_MIRROR}{release}/releases/{arch}/latest-releases.yaml");
                    let iso_regex = iso_regex.clone();

                    async move {
                        let page = capture_page(&mirror).await?;
                        let (_, [iso, checksum]) = iso_regex.captures(&page)?.extract();
                        let url = format!("{ALPINE_MIRROR}{release}/releases/{arch}/{iso}");
                        Some(Config {
                            release: release.to_string(),
                            arch: arch.clone(),
                            iso: Some(vec![Source::Web(WebSource::new(url, Some(checksum.into()), None, None))]),
                            ..Default::default()
                        })
                    }
                })
                .collect::<Vec<_>>()
        });

        futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<Config>>()
            .into()
    }
}

const BATOCERA_MIRROR: &str = "https://mirrors.o2switch.fr/batocera/x86_64/stable/";

pub struct Batocera;
impl Distro for Batocera {
    const NAME: &'static str = "batocera";
    const PRETTY_NAME: &'static str = "Batocera";
    const HOMEPAGE: Option<&'static str> = Some("https://batocera.org/");
    const DESCRIPTION: Option<&'static str> = Some("Retro-gaming distribution with the aim of turning any computer/nano computer into a gaming console during a game or permanently.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let release_data = capture_page(BATOCERA_MIRROR).await?;
        let batocera_regex = Regex::new(r#"<a href="([0-9]{2})/""#).unwrap();
        let iso_regex = Arc::new(Regex::new(r#"<a href="(batocera-x86_64.*?.img.gz)"#).unwrap());

        let mut releases = batocera_regex
            .captures_iter(&release_data)
            .map(|r| r[1].parse::<u32>().unwrap())
            .collect::<Vec<u32>>();
        releases.sort_unstable();
        releases.reverse();

        let futures = releases
            .into_iter()
            .take(3)
            .map(|release| {
                let iso_regex = iso_regex.clone();
                async move {
                    let url = format!("{BATOCERA_MIRROR}{release}/");
                    let page = capture_page(&url).await?;
                    let captures = iso_regex.captures(&page)?;
                    let iso = format!("{url}{}", &captures[1]);
                    Some(Config {
                        release: release.to_string(),
                        img: Some(vec![Source::Web(WebSource::new(iso, None, Some(ArchiveFormat::Gz), None))]),
                        ..Default::default()
                    })
                }
            })
            .collect::<Vec<_>>();

        futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .collect::<Vec<Config>>()
            .into()
    }
}

const CHIMERA_MIRROR: &str = "https://repo.chimera-linux.org/live/";

pub struct ChimeraLinux;
impl Distro for ChimeraLinux {
    const NAME: &'static str = "chimeralinux";
    const PRETTY_NAME: &'static str = "Chimera Linux";
    const HOMEPAGE: Option<&'static str> = Some("https://chimera-linux.org/");
    const DESCRIPTION: Option<&'static str> = Some("Modern, general-purpose non-GNU Linux distribution.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let releases = capture_page(CHIMERA_MIRROR).await?;
        let release_regex = Regex::new(r#"href="([0-9]{8})/""#).unwrap();
        let iso_regex = Arc::new(Regex::new(r#"href="(chimera-linux-(x86_64|aarch64|riscv64)-LIVE-[0-9]{8}-([^-]+).iso)""#).unwrap());

        let releases = {
            let mut releases = release_regex
                .captures_iter(&releases)
                .map(|c| c[1].parse::<u32>().unwrap())
                .collect::<Vec<u32>>();
            releases.sort_unstable();
            releases.reverse();
            let mut releases = releases.iter().map(ToString::to_string).collect::<Vec<String>>();
            if let Some(r) = releases.get_mut(0) {
                *r = "latest".to_string();
            }
            releases
        };

        let futures = releases.iter().map(|release| {
            let url = format!("{CHIMERA_MIRROR}{release}/");
            let checksum_url = url.clone() + "sha256sums.txt";
            let iso_regex = iso_regex.clone();

            async move {
                let page = capture_page(&url).await?;
                let mut checksums = ChecksumSeparation::Whitespace.build(&checksum_url).await;
                Some(
                    iso_regex
                        .captures_iter(&page)
                        .map(|c| c.extract())
                        .map(|(_, [iso, arch, edition])| {
                            let arch = arch_from_str(arch).expect("Chimera Linux: Regex allowed an invalid architecture through");
                            let checksum = checksums.as_mut().and_then(|cs| cs.remove(iso));
                            let url = format!("{url}{iso}");
                            Config {
                                release: release.clone(),
                                edition: Some(edition.to_string()),
                                arch,
                                iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                                ..Default::default()
                            }
                        })
                        .collect::<Vec<Config>>(),
                )
            }
        });

        futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .flatten()
            .collect::<Vec<Config>>()
            .into()
    }
}
