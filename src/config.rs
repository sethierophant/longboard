use std::path::PathBuf;
use std::default::Default;

use rand::{thread_rng, seq::SliceRandom};

use rocket::uri;
use rocket::http::uri::Origin;

/// Configuration for a longboard instance.
pub struct Config {
    /// Where the static files are.
    pub static_dir: PathBuf,
    /// Where the user-uploaded files are.
    pub upload_dir: PathBuf,
    /// A list of banners. These should be in `static_dir`.
    pub banners: Vec<PathBuf>,
}

impl Config {
    /// Choose a banner at random.
    pub fn choose_banner(&self) -> Origin {
        let mut rng = thread_rng();
        let banner_path = PathBuf::from("/banners")
            .join(&self.banners.choose(&mut rng).unwrap());

        uri!(crate::routes::static_file: banner_path)
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            static_dir: PathBuf::from("static"),
            upload_dir: PathBuf::from("upload"),
            banners: Vec::new(),
        }
    }
}
