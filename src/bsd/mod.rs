use crate::store_data::{ArchiveFormat, ChecksumSeparation, Config, Disk, Distro, Source, WebSource};
use crate::utils::capture_page;
use quickemu::config::{Arch, GuestOS};
use regex::Regex;
use std::sync::Arc;

const FREEBSD_X86_64_RELEASES: &str = "https://download.freebsd.org/ftp/releases/amd64/amd64/";
const FREEBSD_AARCH64_RELEASES: &str = "https://download.freebsd.org/ftp/releases/arm64/aarch64/";
const FREEBSD_RISCV64_RELEASES: &str = "https://download.freebsd.org/ftp/releases/riscv/riscv64/";
const FREEBSD_EDITIONS: [&str; 2] = ["disc1", "dvd1"];

pub struct FreeBSD;
impl Distro for FreeBSD {
    const NAME: &'static str = "freebsd";
    const PRETTY_NAME: &'static str = "FreeBSD";
    const HOMEPAGE: Option<&'static str> = Some("https://www.freebsd.org/");
    const DESCRIPTION: Option<&'static str> = Some("Operating system used to power modern servers, desktops, and embedded platforms.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let freebsd_regex = Arc::new(Regex::new(r#"href="([0-9\.]+)-RELEASE"#).unwrap());
        let futures = [
            (FREEBSD_X86_64_RELEASES, "amd64", Arch::x86_64),
            (FREEBSD_AARCH64_RELEASES, "arm64-aarch64", Arch::aarch64),
            (FREEBSD_RISCV64_RELEASES, "riscv-riscv64", Arch::riscv64),
        ]
        .iter()
        .map(|(mirror, denom, arch)| {
            let freebsd_regex = freebsd_regex.clone();

            async move {
                if let Some(page) = capture_page(mirror).await {
                    let futures = freebsd_regex
                        .captures_iter(&page)
                        .flat_map(|c| {
                            let release = c[1].to_string();
                            let vm_image_release = release.clone();

                            let vm_image_mirror = {
                                let arch = if *arch == Arch::x86_64 { "amd64" } else { &arch.to_string() };
                                format!("https://download.freebsd.org/ftp/releases/VM-IMAGES/{release}-RELEASE/{arch}/Latest/")
                            };

                            let normal_editions = tokio::spawn(async move {
                                let checksum_url = format!("{mirror}ISO-IMAGES/{release}/CHECKSUM.SHA256-FreeBSD-{release}-RELEASE-{denom}");
                                let mut checksums = ChecksumSeparation::Sha256Regex.build(&checksum_url).await;
                                FREEBSD_EDITIONS
                                    .iter()
                                    .map(|edition| {
                                        let iso = format!("FreeBSD-{release}-RELEASE-{denom}-{edition}.iso.xz");
                                        let checksum = checksums.as_mut().and_then(|cs| cs.remove(&iso));
                                        let url = format!("{mirror}ISO-IMAGES/{release}/{iso}");
                                        Config {
                                            guest_os: GuestOS::FreeBSD,
                                            iso: Some(vec![Source::Web(WebSource::new(url, checksum, Some(ArchiveFormat::Xz), None))]),
                                            release: Some(release.clone()),
                                            edition: Some(edition.to_string()),
                                            arch: arch.clone(),
                                            ..Default::default()
                                        }
                                    })
                                    .collect::<Vec<Config>>()
                            });

                            let vm_image = tokio::spawn(async move {
                                let iso = format!("FreeBSD-{vm_image_release}-RELEASE-{denom}.qcow2.xz");
                                let checksum_url = format!("{vm_image_mirror}CHECKSUM.SHA256");
                                let checksum = ChecksumSeparation::Sha256Regex
                                    .build(&checksum_url)
                                    .await
                                    .and_then(|mut cs| cs.remove(&iso));
                                let url = vm_image_mirror + &iso;

                                vec![Config {
                                    guest_os: GuestOS::FreeBSD,
                                    disk_images: Some(vec![Disk {
                                        source: Source::Web(WebSource::new(url, checksum, Some(ArchiveFormat::Xz), None)),
                                        ..Default::default()
                                    }]),
                                    release: Some(vm_image_release),
                                    edition: Some("vm-image".to_string()),
                                    arch: arch.clone(),
                                    ..Default::default()
                                }]
                            });
                            [normal_editions, vm_image]
                        })
                        .collect::<Vec<_>>();
                    Some(futures::future::join_all(futures).await)
                } else {
                    log::warn!("Failed to fetch FreeBSD {arch} releases");
                    None
                }
            }
        });
        futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .flatten()
            .flatten()
            .flatten()
            .collect::<Vec<Config>>()
            .into()
    }
}

const DRAGONFLYBSD_MIRROR: &str = "https://mirror-master.dragonflybsd.org/iso-images/";

pub struct DragonFlyBSD;
impl Distro for DragonFlyBSD {
    const NAME: &'static str = "dragonflybsd";
    const PRETTY_NAME: &'static str = "DragonFlyBSD";
    const HOMEPAGE: Option<&'static str> = Some("https://www.dragonflybsd.org/");
    const DESCRIPTION: Option<&'static str> =
        Some("Provides an opportunity for the BSD base to grow in an entirely different direction from the one taken in the FreeBSD, NetBSD, and OpenBSD series.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let mirror_html = capture_page(DRAGONFLYBSD_MIRROR).await?;
        let iso_regex = Regex::new(r#"href="(dfly-x86_64-([0-9.]+)_REL.iso.bz2)""#).unwrap();
        let mut checksums = ChecksumSeparation::Md5Regex
            .build(&(DRAGONFLYBSD_MIRROR.to_string() + "md5.txt"))
            .await;

        let mut releases = iso_regex.captures_iter(&mirror_html).collect::<Vec<_>>();
        // Remove duplicate versions, ignoring patch releases
        releases.dedup_by(|a, b| {
            if let (Ok(a), Ok(b)) = (
                a[2].split('.').take(2).collect::<String>().parse::<u32>(),
                b[2].split('.').take(2).collect::<String>().parse::<u32>(),
            ) {
                a == b
            } else {
                true
            }
        });

        releases
            .into_iter()
            .take(4)
            .map(|c| {
                let iso = &c[1];
                let release = c[2].to_string();
                let checksum = checksums.as_mut().and_then(|cs| cs.remove(iso));
                let url = DRAGONFLYBSD_MIRROR.to_string() + iso;

                Config {
                    guest_os: GuestOS::DragonFlyBSD,
                    iso: Some(vec![Source::Web(WebSource::new(url, checksum, Some(ArchiveFormat::Bz2), None))]),
                    release: Some(release),
                    ..Default::default()
                }
            })
            .collect::<Vec<Config>>()
            .into()
    }
}

const GHOSTBSD_MIRROR: &str = "https://download.ghostbsd.org/releases/amd64/";

pub struct GhostBSD;
impl Distro for GhostBSD {
    const NAME: &'static str = "ghostbsd";
    const PRETTY_NAME: &'static str = "GhostBSD";
    const HOMEPAGE: Option<&'static str> = Some("https://www.ghostbsd.org/");
    const DESCRIPTION: Option<&'static str> = Some("Simple, elegant desktop BSD Operating System.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let release_html = capture_page(GHOSTBSD_MIRROR).await?;
        let release_regex = Regex::new(r#"href="(latest|[\d\.]+)\/""#).unwrap();
        let iso_regex = Arc::new(Regex::new(r#"href="(GhostBSD-[\d\.]+(-[\w]+)?.iso)""#).unwrap());

        let mut releases = release_regex
            .captures_iter(&release_html)
            .map(|r| (r[1].to_string(), format!("{GHOSTBSD_MIRROR}{}/", &r[1])))
            .collect::<Vec<_>>();
        releases.reverse();

        let futures = releases.into_iter().take(4).map(|(release, mirror)| {
            let iso_regex = iso_regex.clone();

            async move {
                let iso_html = capture_page(&mirror).await?;
                let futures = iso_regex
                    .captures_iter(&iso_html)
                    .map(|c| {
                        let release = release.clone();
                        let edition = match c.get(2) {
                            Some(edition) => edition.as_str()[1..].to_string(),
                            None => "MATE".to_string(),
                        };

                        let iso = &c[1];
                        let url = mirror.clone() + iso;
                        let checksum_url = format!("{mirror}{iso}.sha256");

                        async move {
                            let checksum = capture_page(&checksum_url)
                                .await
                                .and_then(|cs| cs.split_once('=').map(|(_, checksum)| checksum.trim().to_string()));

                            Config {
                                guest_os: GuestOS::GhostBSD,
                                iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                                release: Some(release),
                                edition: Some(edition),
                                ..Default::default()
                            }
                        }
                    })
                    .collect::<Vec<_>>();
                Some(futures::future::join_all(futures).await)
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
