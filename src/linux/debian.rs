use crate::{
    store_data::{ChecksumSeparation, Config, Disk, Distro, Source, WebSource},
    utils::{capture_page, GatherData, GithubAPI},
};
use join_futures::join_futures;
use quickemu::config::{Arch, DiskFormat};
use quickget_core::data_structures::ArchiveFormat;
use regex::Regex;
use std::{collections::HashMap, sync::Arc};

const ANTIX_MIRROR: &str = "https://sourceforge.net/projects/antix-linux/files/Final/";

pub struct Antix;
impl Distro for Antix {
    const NAME: &'static str = "antix";
    const PRETTY_NAME: &'static str = "antiX";
    const HOMEPAGE: Option<&'static str> = Some("https://antixlinux.com/");
    const DESCRIPTION: Option<&'static str> = Some("Fast, lightweight and easy to install systemd-free linux live CD distribution based on Debian Stable for Intel-AMD x86 compatible systems.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let releases = capture_page(ANTIX_MIRROR).await?;

        let releases_regex = Regex::new(r#""name":"antiX-([0-9.]+)""#).unwrap();
        let iso_regex = Arc::new(Regex::new(r#""name":"(antiX-[0-9.]+(?:-runit)?(?:-[^_]+)?_x64-([^.]+).iso)".*?"download_url":"(.*?)""#).unwrap());

        let skip_until_sha256 = |cs_data: String| {
            cs_data
                .lines()
                .skip_while(|l| !l.starts_with("sha256"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let futures = releases_regex.captures_iter(&releases).take(3).map(|c| {
            let release = c[1].to_string();
            let mirror = format!("{ANTIX_MIRROR}antiX-{release}/");
            let checksum_mirror = format!("{mirror}README.txt/download");
            let runit_mirror = format!("{mirror}runit-antiX-{release}/");
            let runit_checksum_mirror = format!("{runit_mirror}README2.txt/download");
            let iso_regex = iso_regex.clone();

            async move {
                let main_checksums = capture_page(&checksum_mirror).await.map(skip_until_sha256).unwrap_or_default();
                let runit_checksums = capture_page(&runit_checksum_mirror).await.map(skip_until_sha256);
                let checksums = main_checksums + "\n" + &runit_checksums.unwrap_or_default();
                let mut checksums = ChecksumSeparation::Whitespace.build_with_data(&checksums);

                let page = capture_page(&mirror).await?;
                let iso_regex = iso_regex.clone();
                let main_releases = iso_regex.captures_iter(&page).zip(std::iter::repeat("-sysv"));
                let runit_page = capture_page(&runit_mirror).await?;
                let runit_releases = iso_regex.captures_iter(&runit_page).zip(std::iter::repeat("-runit"));

                Some(
                    main_releases
                        .chain(runit_releases)
                        .map(|(c, ending)| {
                            let checksum = checksums.remove(&c[1]);
                            let edition = c[2].to_string() + ending;
                            let url = c[3].to_string();
                            Config {
                                release: release.to_string(),
                                edition: Some(edition),
                                iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                                ..Default::default()
                            }
                        })
                        .collect::<Vec<_>>(),
                )
            }
        });

        Some(join_futures!(futures, 2))
    }
}

const BUNSENLABS_MIRROR: &str = "https://ddl.bunsenlabs.org/ddl/";

pub struct BunsenLabs;
impl Distro for BunsenLabs {
    const NAME: &'static str = "bunsenlabs";
    const PRETTY_NAME: &'static str = "BunsenLabs";
    const HOMEPAGE: Option<&'static str> = Some("https://www.bunsenlabs.org/");
    const DESCRIPTION: Option<&'static str> = Some("Light-weight and easily customizable Openbox desktop. The project is a community continuation of CrunchBang Linux.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let html = capture_page(BUNSENLABS_MIRROR).await?;
        let release_regex = Regex::new(r#"href="(([^-]+)-1(:?-[0-9]+)?-amd64.hybrid.iso)""#).unwrap();
        // Gather all possible checksums
        let checksum_regex = Regex::new(r#"href="(.*?.sha256.txt)""#).unwrap();

        let checksum_futures = checksum_regex.captures_iter(&html).map(|c| {
            let url = format!("{BUNSENLABS_MIRROR}{}", &c[1]);
            async move { ChecksumSeparation::Whitespace.build(&url).await }
        });
        let mut checksums = join_futures!(checksum_futures, 2, HashMap<String, String>);

        release_regex
            .captures_iter(&html)
            .map(|c| c.extract())
            .map(|(_, [iso, release])| {
                let checksum = checksums.remove(iso);
                let url = format!("{BUNSENLABS_MIRROR}{iso}");

                Config {
                    release: release.to_string(),
                    iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                    ..Default::default()
                }
            })
            .collect::<Vec<Config>>()
            .into()
    }
}

const CRUNCHBANG_API: &str = "https://api.github.com/repos/CBPP/cbpp/releases";

pub struct CrunchbangPlusPlus;
impl Distro for CrunchbangPlusPlus {
    const NAME: &'static str = "crunchbang++";
    const PRETTY_NAME: &'static str = "Crunchbangplusplus";
    const HOMEPAGE: Option<&'static str> = Some("https://crunchbangplusplus.org/");
    const DESCRIPTION: Option<&'static str> = Some("The classic minimal crunchbang feel, now with debian 12 bookworm.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let mut api_data = GithubAPI::gather_data(CRUNCHBANG_API).await?;
        api_data.retain(|v| !v.prerelease);
        api_data
            .into_iter()
            .take(3)
            .filter_map(|value| {
                let release = value.tag_name;
                let iso = value.assets.into_iter().find(|a| a.name.contains("amd64"))?;
                let url = iso.browser_download_url;
                let checksum_data = value
                    .body
                    .lines()
                    .skip_while(|l| !l.contains("md5sum"))
                    .collect::<Vec<&str>>()
                    .join("\n");
                let checksum = ChecksumSeparation::Whitespace.build_with_data(&checksum_data).remove(&iso.name);
                Some(Config {
                    release,
                    iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                    ..Default::default()
                })
            })
            .collect::<Vec<Config>>()
            .into()
    }
}

const LATEST_DEBIAN_MIRROR: &str = "https://cdimage.debian.org/debian-cd/";
const PREVIOUS_DEBIAN_MIRROR: &str = "https://cdimage.debian.org/cdimage/archive/";

pub struct Debian;
impl Distro for Debian {
    const NAME: &'static str = "debian";
    const PRETTY_NAME: &'static str = "Debian";
    const HOMEPAGE: Option<&'static str> = Some("https://www.debian.org/");
    const DESCRIPTION: Option<&'static str> = Some("Complete Free Operating System with perfect level of ease of use and stability.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let latest_html = capture_page(LATEST_DEBIAN_MIRROR).await?;
        let previous_html = capture_page(PREVIOUS_DEBIAN_MIRROR).await?;
        let releases_regex = Regex::new(r#"href="([0-9.]+)/""#).unwrap();
        let live_regex = Arc::new(Regex::new(">(debian-live-[0-9.]+-amd64-([^.]+).iso)<").unwrap());
        let netinst_regex = Arc::new(Regex::new(">(debian-[0-9].+-(?:amd64|arm64)-(netinst).iso)<").unwrap());

        let latest_full_release = releases_regex.captures(&latest_html)?[1].to_string();
        let latest_release = latest_full_release.split('.').next()?.parse::<u32>().ok()?;

        let mut previous_captures = releases_regex
            .captures_iter(&previous_html)
            .map(|c| (c[1].to_string(), c[1].split('.').next().unwrap().parse::<u32>().unwrap()))
            .fold(HashMap::new(), |mut acc, (full_release, release)| {
                if acc.get(&release).map_or(true, |v: &String| {
                    v.split('.').nth(1).unwrap().parse::<u32>().unwrap() < full_release.split('.').nth(1).unwrap().parse::<u32>().unwrap()
                }) {
                    acc.insert(release, full_release);
                }
                acc
            });

        let releases = (latest_release - 2..latest_release)
            .filter_map(|c| previous_captures.remove(&c).map(|f| (c, f, PREVIOUS_DEBIAN_MIRROR)))
            .chain([(latest_release, latest_full_release, LATEST_DEBIAN_MIRROR)]);

        let futures = releases
            .flat_map(|(release, full_release, mirror)| {
                let live_mirror = format!("{mirror}{full_release}-live/amd64/iso-hybrid/");
                let live_regex = live_regex.clone();
                let live_configs = tokio::spawn(async move {
                    let page = capture_page(&live_mirror).await?;
                    let mut checksums = ChecksumSeparation::Whitespace.build(&format!("{live_mirror}SHA256SUMS")).await;
                    Some(
                        live_regex
                            .captures_iter(&page)
                            .map(|c| c.extract())
                            .map(|(_, [iso, edition])| {
                                let url = format!("{live_mirror}{iso}");
                                let checksum = checksums.as_mut().and_then(|cs| cs.remove(iso));
                                Config {
                                    release: release.to_string(),
                                    edition: Some(edition.to_string()),
                                    iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                                    ..Default::default()
                                }
                            })
                            .collect::<Vec<Config>>(),
                    )
                });
                let netinst_configs = [Arch::x86_64, Arch::aarch64]
                    .iter()
                    .map(|arch| {
                        let arch_text = match arch {
                            Arch::x86_64 => "amd64",
                            Arch::aarch64 => "arm64",
                            _ => unreachable!(),
                        };
                        let netinst_mirror = format!("{mirror}{full_release}/{arch_text}/iso-cd/");
                        let checksum_mirror = format!("{netinst_mirror}SHA256SUMS");
                        let netinst_regex = netinst_regex.clone();
                        tokio::spawn(async move {
                            let page = capture_page(&netinst_mirror).await?;
                            let mut checksums = ChecksumSeparation::Whitespace.build(&checksum_mirror).await;
                            Some(
                                netinst_regex
                                    .captures_iter(&page)
                                    .map(|c| c.extract())
                                    .map(|(_, [iso, edition])| {
                                        let url = format!("{netinst_mirror}{iso}");
                                        let checksum = checksums.as_mut().and_then(|cs| cs.remove(iso));
                                        Config {
                                            release: release.to_string(),
                                            edition: Some(edition.to_string()),
                                            iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                                            arch: arch.clone(),
                                            ..Default::default()
                                        }
                                    })
                                    .collect::<Vec<Config>>(),
                            )
                        })
                    })
                    .collect::<Vec<_>>();
                [vec![live_configs], netinst_configs]
            })
            .flatten();

        Some(join_futures!(futures, 3))
    }
}

const DEVUAN_MIRROR: &str = "https://files.devuan.org/";

pub struct Devuan;
impl Distro for Devuan {
    const NAME: &'static str = "devuan";
    const PRETTY_NAME: &'static str = "Devuan";
    const HOMEPAGE: Option<&'static str> = Some("https://devuan.org/");
    const DESCRIPTION: Option<&'static str> =
        Some("Fork of Debian without systemd that allows users to reclaim control over their system by avoiding unnecessary entanglements and ensuring Init Freedom.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let release_html = capture_page(DEVUAN_MIRROR).await?;
        let release_regex = Regex::new(r#"href="(devuan_[a-zA-Z]+/)""#).unwrap();
        let iso_regex = Arc::new(Regex::new(r#"href="(devuan_[a-zA-Z]+_([0-9.]+)_amd64_desktop-live.iso)""#).unwrap());
        let checksum_url_regex = Arc::new(Regex::new(r#"href="(SHA[^.]+.txt)""#).unwrap());

        let futures = release_regex.captures_iter(&release_html).map(|c| {
            let mirror = DEVUAN_MIRROR.to_string() + &c[1] + "desktop-live/";
            let iso_regex = iso_regex.clone();
            let checksum_url_regex = checksum_url_regex.clone();

            async move {
                let page_data = capture_page(&mirror).await?;
                let mut checksums = match checksum_url_regex.captures(&page_data) {
                    Some(c) => ChecksumSeparation::Whitespace.build(&(mirror.to_string() + &c[1])).await,
                    None => None,
                };

                Some(
                    iso_regex
                        .captures_iter(&page_data)
                        .map(|c| {
                            let release = c[2].to_string();
                            let iso = &c[1];
                            let url = mirror.clone() + iso;
                            let checksum = checksums.as_mut().and_then(|cs| cs.remove(iso));
                            Config {
                                release,
                                iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                                ..Default::default()
                            }
                        })
                        .collect::<Vec<Config>>(),
                )
            }
        });
        Some(join_futures!(futures, 2))
    }
}

const EASYOS_MIRROR: &str = "https://distro.ibiblio.org/easyos/amd64/releases/";

pub struct EasyOS;
impl Distro for EasyOS {
    const NAME: &'static str = "easyos";
    const PRETTY_NAME: &'static str = "EasyOS";
    const HOMEPAGE: Option<&'static str> = Some("https://easyos.org/");
    const DESCRIPTION: Option<&'static str> = Some("Experimental distribution designed from scratch to support containers.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let release_html = capture_page(EASYOS_MIRROR).await?;
        let release_name_regex = Regex::new(r#"href="([a-z]+/)""#).unwrap();
        let subdirectory_regex = Arc::new(Regex::new(r#"href="([0-9]{4}/)""#).unwrap());
        let release_regex = Arc::new(Regex::new(r#"href="([0-9](?:\.[0-9]+)+)/""#).unwrap());
        let img_regex = Arc::new(Regex::new(r#"href="(easy-[0-9.]+-amd64.img(.gz)?)""#).unwrap());

        let release_futures = release_name_regex.captures_iter(&release_html).map(|c| {
            let mirror = EASYOS_MIRROR.to_string() + &c[1];
            let subdirectory_regex = subdirectory_regex.clone();
            let release_regex = release_regex.clone();

            async move {
                let subdirectory_html = capture_page(&mirror).await?;
                let futures = subdirectory_regex.captures_iter(&subdirectory_html).map(|c| {
                    let mirror = mirror.clone() + &c[1];
                    let release_regex = release_regex.clone();
                    async move {
                        let releases_html = capture_page(&mirror).await?;
                        Some(
                            release_regex
                                .captures_iter(&releases_html)
                                .map(|c| {
                                    let release = c[1].to_string();
                                    let mirror = mirror.clone() + &release + "/";
                                    (release, mirror)
                                })
                                .collect::<Vec<_>>(),
                        )
                    }
                });

                Some(join_futures!(futures))
            }
        });
        let mut releases = join_futures!(release_futures, 4, Vec<(String, String)>);

        releases.sort_by(|(a, _), (b, _)| {
            if let (Ok(a), Ok(b)) = (
                a.split('.').take(2).collect::<Vec<&str>>().join(".").parse::<f64>(),
                b.split('.').take(2).collect::<Vec<&str>>().join(".").parse::<f64>(),
            ) {
                a.partial_cmp(&b).unwrap()
            } else {
                std::cmp::Ordering::Equal
            }
        });
        releases.reverse();

        releases.dedup_by(|(a, _), (b, _)| {
            if let (Ok(a), Ok(b)) = (
                a.split('.').take(2).collect::<String>().parse::<u32>(),
                b.split('.').take(2).collect::<String>().parse::<u32>(),
            ) {
                a == b
            } else {
                true
            }
        });
        println!("{:?}", releases);

        let futures = releases.into_iter().take(5).map(|(release, mirror)| {
            let img_regex = img_regex.clone();

            async move {
                let page = capture_page(&mirror).await?;
                let checksum_url = mirror.clone() + "md5sum.txt";
                let checksum = capture_page(&checksum_url)
                    .await
                    .and_then(|cs| cs.split_whitespace().next().map(ToString::to_string));

                let img_capture = img_regex.captures(&page)?;
                let url = mirror + &img_capture[1];
                let archive_format = if img_capture.get(2).is_some() { Some(ArchiveFormat::Gz) } else { None };
                Some(Config {
                    release,
                    disk_images: Some(vec![Disk {
                        source: Source::Web(WebSource::new(url, checksum, archive_format, None)),
                        format: DiskFormat::Raw,
                        ..Default::default()
                    }]),
                    ..Default::default()
                })
            }
        });
        Some(join_futures!(futures, 1))
    }
}

const ENDLESS_DL_MIRROR: &str = "https://images-dl.endlessm.com/release/";
const ENDLESS_DATA_MIRROR: &str = "https://mirror.leitecastro.com/endless/release/";

pub struct EndlessOS;
impl Distro for EndlessOS {
    const NAME: &'static str = "endless";
    const PRETTY_NAME: &'static str = "Endless OS";
    const HOMEPAGE: Option<&'static str> = Some("https://endlessos.org/");
    const DESCRIPTION: Option<&'static str> = Some("Completely Free, User-Friendly Operating System Packed with Educational Tools, Games, and More.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let release_html = capture_page(ENDLESS_DATA_MIRROR).await?;
        let release_regex = Regex::new(r#"href="(\d+(?:.\d+){2})\/""#).unwrap();
        let edition_regex = Arc::new(Regex::new(r#"href="([^./]+)"#).unwrap());
        let iso_regex = Arc::new(Regex::new(r#"href="(eos-eos[\d.]+-amd64-amd64.[-\d]+.[^.]+.iso)""#).unwrap());

        let futures = release_regex.captures_iter(&release_html).map(|c| {
            let release = c[1].to_string();
            let mirror = ENDLESS_DATA_MIRROR.to_string() + &release + "/eos-amd64-amd64/";
            let edition_regex = edition_regex.clone();
            let iso_regex = iso_regex.clone();
            async move {
                let edition_html = capture_page(&mirror).await?;
                let futures = edition_regex.captures_iter(&edition_html).map(|c| {
                    let edition = c[1].to_string();
                    let mirror = mirror.clone() + &edition + "/";
                    let iso_regex = iso_regex.clone();
                    let release = release.clone();
                    async move {
                        let page = capture_page(&mirror).await?;
                        let iso = &iso_regex.captures(&page)?[1];
                        let url = format!("{ENDLESS_DL_MIRROR}{release}/eos-amd64-amd64/{edition}/{iso}");

                        let checksum_url = url.clone() + ".sha256";
                        let checksum = capture_page(&checksum_url)
                            .await
                            .and_then(|cs| cs.split_whitespace().next().map(ToString::to_string));
                        Some(Config {
                            release,
                            edition: Some(edition),
                            iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                            ..Default::default()
                        })
                    }
                });
                Some(join_futures!(futures))
            }
        });

        Some(join_futures!(futures, 3))
    }
}
const LMDE_MIRROR: &str = "https://mirrors.edge.kernel.org/linuxmint/debian/";

pub struct Lmde;
impl Distro for Lmde {
    const NAME: &'static str = "lmde";
    const PRETTY_NAME: &'static str = "Linux Mint Debian Edition";
    const HOMEPAGE: Option<&'static str> = Some("https://linuxmint.com/download_lmde.php");
    const DESCRIPTION: Option<&'static str> = Some("Aims to be as similar as possible to Linux Mint, but without using Ubuntu. The package base is provided by Debian instead.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let page = capture_page(LMDE_MIRROR).await?;
        let mut checksums = ChecksumSeparation::Whitespace
            .build(&format!("{LMDE_MIRROR}sha256sum.txt"))
            .await;
        let iso_regex = Regex::new(r#"href="(lmde-(\d+(?:\.\d+)?)-(\w+)-64bit.iso)""#).unwrap();

        Some(
            iso_regex
                .captures_iter(&page)
                .map(|c| {
                    let iso = &c[1];
                    let checksum = checksums.as_mut().and_then(|cs| cs.remove(&format!("*{iso}")));
                    let url = format!("{LMDE_MIRROR}{iso}");
                    Config {
                        release: c[2].to_string(),
                        edition: Some(c[3].to_string()),
                        iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                        ..Default::default()
                    }
                })
                .collect(),
        )
    }
}
