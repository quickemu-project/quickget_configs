use crate::store_data::{ArchiveFormat, Config, Disk, Distro, Source, WebSource};
use crate::utils::capture_page;
use quickemu::config::{Arch, GuestOS};
use regex::Regex;
use std::collections::HashMap;
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
        let checksum_regex = Arc::new(Regex::new(r#"SHA256 \(([^)]+)\) = ([0-9a-f]+)"#).unwrap());
        let futures = [
            (FREEBSD_X86_64_RELEASES, "amd64", Arch::x86_64),
            (FREEBSD_AARCH64_RELEASES, "arm64-aarch64", Arch::aarch64),
            (FREEBSD_RISCV64_RELEASES, "riscv-riscv64", Arch::riscv64),
        ]
        .iter()
        .map(|(mirror, denom, arch)| {
            let checksum_regex = checksum_regex.clone();
            let freebsd_regex = freebsd_regex.clone();

            let build_checksums = |cs_url: String, cs_regex: Arc<Regex>| async move {
                let checksum_page = capture_page(&cs_url).await;
                checksum_page.map(|cs| {
                    cs_regex
                        .captures_iter(&cs)
                        .map(|c| (c[1].to_string(), c[2].to_string()))
                        .collect::<HashMap<String, String>>()
                })
            };

            async move {
                if let Some(page) = capture_page(mirror).await {
                    let futures = freebsd_regex
                        .captures_iter(&page)
                        .flat_map(|c| {
                            let release = c[1].to_string();
                            let vm_image_release = release.clone();
                            let normal_checksum_regex = checksum_regex.clone();
                            let vm_checksum_regex = checksum_regex.clone();

                            let vm_image_mirror = {
                                let arch = if *arch == Arch::x86_64 { "amd64" } else { &arch.to_string() };
                                format!("https://download.freebsd.org/ftp/releases/VM-IMAGES/{release}-RELEASE/{arch}/Latest/")
                            };

                            let normal_editions = tokio::spawn(async move {
                                let checksum_url = format!("{mirror}ISO-IMAGES/{release}/CHECKSUM.SHA256-FreeBSD-{release}-RELEASE-{denom}");
                                let mut checksums = build_checksums(checksum_url, normal_checksum_regex).await;
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
                                let checksum = build_checksums(checksum_url, vm_checksum_regex)
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
