use crate::{
    store_data::{ArchiveFormat, ChecksumSeparation, Config, Distro, Source, WebSource},
    utils::{arch_from_str, capture_page},
};
use join_futures::join_futures;
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

        let releases = releases
            .contents
            .iter()
            .map(|r| &r.key)
            .filter_map(|r| standard_release.captures(r))
            .map(|c| c[1].to_string());

        let futures = releases.rev().take(6).map(|release| {
            let iso_regex = iso_regex.clone();
            async move {
                let page = capture_page(&format!("{NIX_URL}&prefix=nixos-{release}/")).await?;
                let page: NixReleases = quick_xml::de::from_str(&page).ok()?;

                let iso_keys = page.contents.iter().map(|r| &r.key).filter(|r| r.ends_with(".iso"));
                let isos = iso_keys
                    .filter_map(|r| iso_regex.captures(r))
                    .map(|c| c.extract())
                    .filter_map(|(name, [edition, arch])| {
                        let url = format!("{NIX_DOWNLOAD_URL}/nixos-{release}/{name}");
                        let edition = edition.to_string();
                        let arch = arch_from_str(arch)?;
                        Some((url, edition, arch))
                    });

                let futures = isos.map(|(url, edition, arch)| {
                    let release = release.clone();
                    async move {
                        let hash = capture_page(&format!("{url}.sha256"))
                            .await
                            .map(|h| h.split_whitespace().next().unwrap().to_string());
                        Some(Config {
                            release,
                            edition: Some(edition),
                            arch,
                            iso: Some(vec![Source::Web(WebSource::new(url, hash, None, None))]),
                            ..Default::default()
                        })
                    }
                });
                Some(join_futures!(futures, 1))
            }
        });

        Some(join_futures!(futures, 2))
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
        let iso_regex = Regex::new(r#"(?s)iso: (alpine-virt-[0-9]+\.[0-9]+.*?.iso).*? sha256: ([0-9a-f]+)"#).unwrap();

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

        Some(join_futures!(futures, 1))
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
        let iso_regex = Regex::new(r#"<a href="(batocera-x86_64.*?.img.gz)"#).unwrap();

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

        Some(join_futures!(futures, 1))
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
        let iso_regex = Regex::new(r#"href="(chimera-linux-(x86_64|aarch64|riscv64)-LIVE-[0-9]{8}-([^-]+).iso)""#).unwrap();

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

        Some(join_futures!(futures, 2))
    }
}

const GENTOO_MIRROR: &str = "https://distfiles.gentoo.org/releases/";

pub struct Gentoo;
impl Distro for Gentoo {
    const NAME: &'static str = "gentoo";
    const PRETTY_NAME: &'static str = "Gentoo";
    const HOMEPAGE: Option<&'static str> = Some("https://www.gentoo.org/");
    const DESCRIPTION: Option<&'static str> = Some("Highly flexible, source-based Linux distribution.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let iso_regex = Regex::new(r#"\d{8}T\d{6}Z\/(admincd|install|livegui).*?.iso"#).unwrap();
        let futures = [(Arch::x86_64, "amd64"), (Arch::aarch64, "arm64")]
            .into_iter()
            .map(|(arch, arch_str)| {
                let iso_regex = iso_regex.clone();
                let mirror = format!("{GENTOO_MIRROR}{arch_str}/autobuilds/");
                async move {
                    let image_data = capture_page(&(mirror.clone() + "latest-iso.txt")).await?;

                    let futures = iso_regex
                        .captures_iter(&image_data)
                        .map(|c| c.extract())
                        .map(|(iso, [mut edition])| {
                            if edition == "install" {
                                edition = "minimal";
                            }
                            let url = format!("{mirror}{iso}");
                            let checksum_url = url.clone() + ".sha256";
                            let arch = arch.clone();
                            async move {
                                let checksum = capture_page(&checksum_url).await.and_then(|cs| {
                                    cs.lines()
                                        .find(|l| l.contains("iso"))
                                        .and_then(|l| l.split_whitespace().next().map(ToString::to_string))
                                });

                                Config {
                                    release: "latest".to_string(),
                                    edition: Some(edition.to_string()),
                                    iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                                    arch,
                                    ..Default::default()
                                }
                            }
                        });

                    Some(join_futures!(futures))
                }
            });
        Some(join_futures!(futures, 2))
    }
}

const GNOMEOS_MIRROR: &str = "https://download.gnome.org/gnomeos/";

pub struct GnomeOS;
impl Distro for GnomeOS {
    const NAME: &'static str = "gnomeos";
    const PRETTY_NAME: &'static str = "GNOME OS";
    const HOMEPAGE: Option<&'static str> = Some("https://os.gnome.org/");
    const DESCRIPTION: Option<&'static str> = Some("Alpha nightly bleeding edge distro of GNOME");
    async fn generate_configs() -> Option<Vec<Config>> {
        let release_html = capture_page(GNOMEOS_MIRROR).await?;
        let release_regex = Regex::new(r#"href="(\d[^/]+)\/""#).unwrap();
        let iso_regex = Regex::new(r#"href="(gnome_os.*?.iso)""#).unwrap();

        let mut releases = release_regex
            .captures_iter(&release_html)
            .map(|r| (r[1].to_string(), format!("{GNOMEOS_MIRROR}{}/", &r[1])))
            .collect::<Vec<_>>();
        releases.reverse();

        let futures = releases.into_iter().take(6).map(|(release, mirror)| {
            let iso_regex = iso_regex.clone();
            async move {
                let page = capture_page(&mirror).await?;
                let iso = &iso_regex.captures(&page)?[1];
                let url = format!("{mirror}{iso}");
                Some(Config {
                    release,
                    iso: Some(vec![Source::Web(WebSource::url_only(url))]),
                    ..Default::default()
                })
            }
        });

        let mut configs = join_futures!(futures, 1);

        configs.push(Config {
            release: "nightly".to_string(),
            iso: Some(vec![Source::Web(WebSource::url_only(
                "https://os.gnome.org/download/latest/gnome_os_installer.iso",
            ))]),
            ..Default::default()
        });

        Some(configs)
    }
}

const GUIX_MIRROR: &str = "https://mirror.fcix.net/gnu/guix/";
const FINAL_GUIX_MIRROR: &str = "https://ftpmirror.gnu.org/gnu/guix/";

pub struct Guix;
impl Distro for Guix {
    const NAME: &'static str = "guix";
    const PRETTY_NAME: &'static str = "Guix";
    const HOMEPAGE: Option<&'static str> = Some("https://guix.gnu.org/");
    const DESCRIPTION: Option<&'static str> = Some("Distribution of the GNU operating system developed by the GNU Project—which respects the freedom of computer users.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let page = capture_page(GUIX_MIRROR).await?;
        let iso_regex = Regex::new(r#"href="(guix-system-install-(\d(?:\.\d){2})\.(.*?)-linux.iso)""#).unwrap();

        Some(
            iso_regex
                .captures_iter(&page)
                .map(|c| c.extract())
                .filter_map(|(_, [iso, release, arch])| {
                    arch_from_str(arch).map(|arch| Config {
                        release: release.to_string(),
                        iso: Some(vec![Source::Web(WebSource::url_only(format!("{FINAL_GUIX_MIRROR}{iso}")))]),
                        arch,
                        ..Default::default()
                    })
                })
                .collect(),
        )
    }
}
