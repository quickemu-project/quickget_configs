use crate::store_data::{Config, Distro};

pub struct Windows;
impl Distro for Windows {
    const NAME: &'static str = "windows";
    const PRETTY_NAME: &'static str = "Windows";
    const HOMEPAGE: Option<&'static str> = Some("https://www.microsoft.com/en-us/windows/");
    const DESCRIPTION: Option<&'static str> = Some("Whether you’re gaming, studying, running a business, or running a household, Windows helps you get it done.");
    async fn generate_configs() -> Option<Vec<Config>> {
        Some(vec![])
    }
}
