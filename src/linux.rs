mod arch;
mod debian;
mod fedora_redhat;
mod independent;
mod ubuntu;

pub(crate) use arch::{manjaro::BigLinux, ArchLinux, Archcraft, ArcoLinux, ArtixLinux, AthenaOS, BlendOS, CachyOS};
pub(crate) use debian::{Antix, BunsenLabs, CrunchbangPlusPlus, Debian, Devuan};
pub(crate) use fedora_redhat::{Alma, Bazzite, CentOSStream};
pub(crate) use independent::{Alpine, Batocera, ChimeraLinux, NixOS};
pub(crate) use ubuntu::{Bodhi, Edubuntu, Elementary, Kubuntu, Lubuntu, Ubuntu, UbuntuBudgie, UbuntuCinnamon, UbuntuKylin, UbuntuMATE, UbuntuServer, UbuntuStudio, UbuntuUnity, Xubuntu};
