use crate::store_data::{ArchiveFormat, ChecksumSeparation, Config, Distro, Source, WebSource};
use crate::utils::capture_page;
use isolang::Language;
use join_futures::join_futures;
use quickemu::config::GuestOS;
use regex::Regex;
use std::str::FromStr;
use std::sync::Arc;

const FREEDOS_MIRROR: &str = "https://www.ibiblio.org/pub/micro/pc-stuff/freedos/files/distributions/";

pub struct FreeDOS;
impl Distro for FreeDOS {
    const NAME: &'static str = "freedos";
    const PRETTY_NAME: &'static str = "FreeDOS";
    const HOMEPAGE: Option<&'static str> = Some("https://www.freedos.org/");
    const DESCRIPTION: Option<&'static str> = Some("DOS-compatible operating system that you can use to play classic DOS games, run legacy business software, or develop embedded systems.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let release_html = capture_page(FREEDOS_MIRROR).await?;
        let release_regex = Regex::new(r#"href="(\d+\.\d+)/""#).unwrap();
        let iso_regex = Arc::new(Regex::new(r#"href="(FD\d+-?(.*?CD)\.(iso|zip))""#).unwrap());
        let checksum_regex = Arc::new(Regex::new(r#"FD\d+.sha|verify.txt"#).unwrap());

        let futures = release_regex.captures_iter(&release_html).map(|c| {
            let release = c[1].to_string();
            let mirror = format!("{FREEDOS_MIRROR}{release}/official/");
            let iso_regex = iso_regex.clone();
            let checksum_regex = checksum_regex.clone();
            async move {
                let page = capture_page(&mirror).await?;

                let mut checksums = match checksum_regex.find(&page) {
                    Some(cs_match) => {
                        let checksum_url = format!("{mirror}{}", cs_match.as_str());
                        ChecksumSeparation::Whitespace.build(&checksum_url).await
                    }
                    None => None,
                };

                Some(
                    iso_regex
                        .captures_iter(&page)
                        .map(|c| c.extract())
                        .map(|(_, [iso, edition, filetype])| {
                            let url = format!("{mirror}{iso}");
                            let checksum = checksums.as_mut().and_then(|cs| cs.remove(iso));
                            let archive_format = match filetype {
                                "iso" => None,
                                "zip" => Some(ArchiveFormat::Zip),
                                _ => panic!("FreeDOS: Regex allowed an invalid filetype"),
                            };
                            Config {
                                guest_os: GuestOS::FreeDOS,
                                release: release.clone(),
                                edition: Some(edition.to_string()),
                                iso: Some(vec![Source::Web(WebSource::new(url, checksum, archive_format, None))]),
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

const HAIKU_MIRROR: &str = "http://mirror.rit.edu/haiku/";

pub struct Haiku;
impl Distro for Haiku {
    const NAME: &'static str = "haiku";
    const PRETTY_NAME: &'static str = "Haiku";
    const HOMEPAGE: Option<&'static str> = Some("https://www.haiku-os.org/");
    const DESCRIPTION: Option<&'static str> = Some("Specifically targets personal computing. Inspired by the BeOS, Haiku is fast, simple to use, easy to learn and yet very powerful.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let page = capture_page(HAIKU_MIRROR).await?;
        let release_regex = Regex::new(r#"href="(r.*?)/""#).unwrap();
        let iso_regex = Regex::new(r#"href="(haiku-r.?*-x86_64-anyboot.iso)""#).unwrap();

        let futures = release_regex.captures_iter(&page).map(|c| {
            let release = c[1].to_string();
            let mirror = format!("{HAIKU_MIRROR}{release}/");
            let iso_regex = iso_regex.clone();
            async move {
                let page = capture_page(&mirror).await?;
                let iso_capture = iso_regex.captures(&page)?;
                let iso = &iso_capture[1];
                let url = format!("{mirror}{iso}");

                let checksum_url = url.clone() + ".sha256";
                let checksum = capture_page(&checksum_url)
                    .await
                    .and_then(|c| c.split_once('=').map(|(_, cs)| cs.trim().to_string()));

                Some(Config {
                    guest_os: GuestOS::Haiku,
                    release,
                    iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                    ..Default::default()
                })
            }
        });
        Some(join_futures!(futures, 1))
    }
}

const KOLIBRIOS_MIRROR: &str = "https://builds.kolibrios.org/";

pub struct KolibriOS;
impl Distro for KolibriOS {
    const NAME: &'static str = "kolibrios";
    const PRETTY_NAME: &'static str = "KolibriOS";
    const HOMEPAGE: Option<&'static str> = Some("https://kolibrios.org/");
    const DESCRIPTION: Option<&'static str> = Some("Tiny yet incredibly powerful and fast operating system.");
    async fn generate_configs() -> Option<Vec<Config>> {
        let page = capture_page(KOLIBRIOS_MIRROR).await?;
        let locale_regex = Regex::new(r#"href="(\w{2}_\w{2})\/""#).unwrap();

        let futures = locale_regex.captures_iter(&page).map(|c| {
            let locale = &c[1];
            let language = locale
                .split('_')
                .next()
                .and_then(|lang| Language::from_str(lang).ok().map(|lang| lang.to_string()))
                .unwrap_or(locale.to_string());

            let mirror = format!("{KOLIBRIOS_MIRROR}{locale}/");
            let checksum_url = mirror.clone() + "sha256sums.txt";
            async move {
                let checksum_page = capture_page(&checksum_url).await?;
                let iso_entry = checksum_page
                    .lines()
                    .find(|line| line.contains("iso"))
                    .and_then(|line| line.split_once("  "));

                let (checksum, iso_name) = match iso_entry {
                    Some((cs, name)) => (Some(cs.to_string()), name),
                    None => (None, "latest-iso.7z"),
                };

                let archive_format = (iso_name.ends_with(".7z")).then_some(ArchiveFormat::SevenZip);

                let url = mirror.clone() + iso_name;
                Some(Config {
                    guest_os: GuestOS::KolibriOS,
                    release: "latest".into(),
                    edition: Some(language.to_string()),
                    iso: Some(vec![Source::Web(WebSource::new(url, checksum, archive_format, None))]),
                    ..Default::default()
                })
            }
        });

        Some(join_futures!(futures, 1))
    }
}
