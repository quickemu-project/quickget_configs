use quickemu::config::{Arch, DiskFormat, GuestOS};
use quickget::data_structures::{Config, Disk, DockerSource, Source, OS};
use serde::Deserialize;

use crate::utils::capture_page;

const GH_CONTAINER_SOURCE: &str = "ghcr.io/quickemu-project/build-quickget-images";
const TAG_SOURCE: &str = "https://ghcr.io/v2/quickemu-project/build-quickget-images/tags/list";
const TOKEN_SOURCE: &str = r#"https://ghcr.io/token\?scope\="repository:quickemu-project/build-quickget-images:pull""#;

#[derive(Deserialize)]
struct DockerData {
    url: String,
    data: Vec<DockerOsData>,
}

#[derive(Deserialize)]
struct DockerOsData {
    os_name: String,
    #[serde(default)]
    guest_os: GuestOS,
    images: Vec<ImageInfo>,
}

#[derive(Deserialize)]
struct ImageInfo {
    #[serde(default)]
    image_type: ImageType,
    release: String,
    #[serde(default)]
    edition: Option<String>,
    architectures: Vec<Arch>,
    #[serde(default)]
    env: Vec<(String, String)>,
    filename: String,
}

#[derive(Default, Deserialize)]
enum ImageType {
    #[default]
    Iso,
    Img,
    Disk(DiskInfo),
}

#[derive(Deserialize)]
struct DiskInfo {
    #[serde(default)]
    size: Option<u64>,
    #[serde(default)]
    format: DiskFormat,
}

pub async fn add_container_images(distros: &mut [OS]) {
    let Some(container_images) = fetch_container_images().await else { return };
    container_images.into_iter().for_each(|container| {
        container.data.into_iter().for_each(|os_data| {
            let os = distros.iter_mut().find(|d| d.name == os_data.os_name);
            match os {
                Some(os) => os.releases.extend(os_data.images.into_iter().flat_map(|image| {
                    let url = &container.url;
                    let guest_os = os_data.guest_os.clone();
                    let release = image.release + "-build";
                    image.architectures.into_iter().map(move |arch| {
                        let mut config = Config {
                            release: Some(release.clone()),
                            edition: image.edition.clone(),
                            guest_os: guest_os.clone(),
                            arch,
                            ..Default::default()
                        };
                        let source = Source::Docker(DockerSource {
                            url: url.to_string(),
                            env: image.env.clone(),
                            output_filename: image.filename.clone(),
                        });
                        match &image.image_type {
                            ImageType::Iso => config.iso = Some(vec![source]),
                            ImageType::Img => config.img = Some(vec![source]),
                            ImageType::Disk(DiskInfo { size, format }) => {
                                config.disk_images = Some(vec![Disk {
                                    source,
                                    size: *size,
                                    format: format.clone(),
                                }])
                            }
                        }
                        config
                    })
                })),
                None => eprintln!("OS {} not found in distros", os_data.os_name),
            }
        })
    });
}

async fn fetch_container_images() -> Option<Vec<DockerData>> {
    let token_data = capture_page(TOKEN_SOURCE).await?;
    let token = serde_json::from_str::<DockerToken>(&token_data).ok()?.token;

    let client = reqwest::Client::new();
    let data = client
        .get(TAG_SOURCE)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "vnd.docker.distribution.manifest.v2+json")
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;

    let api_data: DockerEntry = serde_json::from_str(&data).ok()?;
    let tags = api_data.tags;

    tags.iter()
        .flat_map(|tag| DockerData::try_from(&**tag).inspect_err(|e| eprintln!("Failed to parse metadata from tag {tag}: {e}")))
        .collect::<Vec<_>>()
        .into()
}

type Tag<'a> = &'a str;
impl<'a> TryFrom<Tag<'a>> for DockerData {
    type Error = String;
    fn try_from(tag: Tag<'a>) -> Result<Self, Self::Error> {
        let file = format!("packages/{tag}/metadata.json");
        let data = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let images: Vec<DockerOsData> = serde_json::from_str(&data).map_err(|e| e.to_string())?;
        Ok(DockerData {
            url: format!("{GH_CONTAINER_SOURCE}:{tag}"),
            data: images,
        })
    }
}

#[derive(Deserialize)]
struct DockerToken {
    token: String,
}

#[derive(Deserialize)]
struct DockerEntry {
    tags: Vec<String>,
}
