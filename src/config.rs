use std::default::Default;
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};

use rand::{seq::SliceRandom, thread_rng};

use rocket::http::uri::Origin;
use rocket::uri;

/// A banner to be displayed at the top of the page.
#[derive(Debug, Clone)]
pub struct Banner {
    pub name: String,
}

impl Banner {
    pub fn uri(&self) -> Origin {
        let path = Path::new("/banners").join(&self.name);
        uri!(crate::routes::static_file: path)
    }
}

impl Serialize for Banner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.uri().to_string())
    }
}

/// Configuration for a longboard instance.
#[derive(Debug)]
pub struct Config {
    /// Where the static files are.
    pub static_dir: PathBuf,
    /// Where the user-uploaded files are.
    pub upload_dir: PathBuf,
    /// A list of banners. These should be in `static_dir`.
    pub banners: Vec<Banner>,
}

impl Config {
    /// Choose a banner at random.
    pub fn choose_banner(&self) -> &Banner {
        let mut rng = thread_rng();
        &self.banners.choose(&mut rng).expect("banner list is empty")
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
