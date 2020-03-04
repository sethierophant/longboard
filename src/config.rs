use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize, Serializer};

use rand::{seq::SliceRandom, thread_rng};

use rocket::http::uri::Origin;
use rocket::uri;

use crate::{Error, Result};

/// A banner to be displayed at the top of the page.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
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
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.uri().to_string())
    }
}

/// Configuration for a longboard instance.
#[derive(Debug, Deserialize)]
pub struct Config {
    /// Where the static files are.
    pub static_dir: PathBuf,
    /// Where the user-uploaded files are.
    pub upload_dir: PathBuf,
    /// Where the templates to be rendered are.
    pub template_dir: PathBuf,
    /// A list of banners. These should be in `${static_dir}/banners`.
    // TODO: Autoload these?
    // TODO: Allow banners outside of that directory?
    pub banners: Vec<Banner>,
    /// Address to bind to
    pub address: String,
    /// Port to bind to
    pub port: u16,
    /// URL to connect to the database
    pub database_url: String,
}

impl Config {
    /// Choose a banner at random.
    pub fn choose_banner(&self) -> &Banner {
        let mut rng = thread_rng();
        &self.banners.choose(&mut rng).expect("banner list is empty")
    }
}

impl Config {
    pub fn open<P>(path: P) -> Result<Config>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let msg = format!("Couldn't open config file at {}", path.display());

        let reader = File::open(path)
            .map_err(|err| Error::from_io_error(err, msg))?;

        Ok(serde_yaml::from_reader(reader)?)
    }
}
