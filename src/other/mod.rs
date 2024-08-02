use crate::store_data::{ArchiveFormat, ChecksumSeparation, Config, Distro, Source, WebSource};
use crate::utils::capture_page;
use quickemu::config::GuestOS;
use regex::Regex;
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
                                release: Some(release.clone()),
                                edition: Some(edition.to_string()),
                                iso: Some(vec![Source::Web(WebSource::new(url, checksum, archive_format, None))]),
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
