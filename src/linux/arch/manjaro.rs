use crate::{
    store_data::{Config, Distro, Source, WebSource},
    utils::capture_page,
};
use regex::Regex;

const BIGLINUX_MIRROR: &str = "https://iso.biglinux.com.br/";

pub struct BigLinux;
impl Distro for BigLinux {
    const NAME: &'static str = "biglinux";
    const PRETTY_NAME: &'static str = "BigLinux";
    const HOMEPAGE: Option<&'static str> = Some("https://www.biglinux.com.br/");
    const DESCRIPTION: Option<&'static str> = Some(
        "It's the right choice if you want to have an easy and enriching experience with Linux. It has been perfected over more than 19 years, following our motto: 'In search of the perfect system'",
    );
    async fn generate_configs() -> Option<Vec<Config>> {
        let data = capture_page(BIGLINUX_MIRROR).await?;
        let biglinux_regex = Regex::new(r#"<a href="(biglinux_([0-9]{4}(?:-[0-9]{2}){2})_(.*?).iso)""#).unwrap();

        let mut data = biglinux_regex.captures_iter(&data).collect::<Vec<_>>();
        data.sort_unstable_by_key(|c| c[2].to_string());
        data.reverse();

        let futures = data.into_iter().map(|c| c.extract()).map(|(_, [iso, release, edition])| {
            let url = BIGLINUX_MIRROR.to_string() + iso;
            let checksum_url = url.clone() + ".md5";
            async move {
                let checksum = capture_page(&checksum_url)
                    .await
                    .and_then(|s| s.split_whitespace().next().map(ToString::to_string));
                Config {
                    release: Some(release.to_string()),
                    edition: Some(edition.to_string()),
                    iso: Some(vec![Source::Web(WebSource::new(url, checksum, None, None))]),
                    ..Default::default()
                }
            }
        });

        futures::future::join_all(futures).await.into()
    }
}
