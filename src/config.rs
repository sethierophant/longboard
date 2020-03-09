use std::fs::File;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize};

use rand::{seq::SliceRandom, thread_rng};

use regex::Regex;

use rocket::http::uri::Origin;
use rocket::uri;

use crate::{Error, Result};

/// A banner to be displayed at the top of the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Configuration for a longboard instance.
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// Address to bind to
    pub address: String,
    /// Port to bind to
    pub port: u16,
    /// Where the static files are.
    pub static_dir: PathBuf,
    /// Where the user-uploaded files are.
    pub upload_dir: PathBuf,
    /// Where the templates to be rendered are.
    pub template_dir: PathBuf,
    /// A list of banners. These should be in `${static_dir}/banners`.
    // TODO: Autoload these?
    // TODO: Allow banners outside of that directory?
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub banners: Vec<Banner>,
    /// URL to connect to the database
    pub database_url: String,
    /// File to log to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_file: Option<PathBuf>,
    /// Filter rules to apply to posts
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filter_rules: Vec<Rule>,
}

impl Config {
    /// Open a config file at the given path.
    pub fn open<P>(path: P) -> Result<Config>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let msg = format!("Couldn't open config file at {}", path.display());

        let reader = File::open(path).map_err(|err| Error::from_io_error(err, msg))?;

        Ok(serde_yaml::from_reader(reader)?)
    }

    /// Generate a new config file from default values.
    pub fn generate<W>(mut out: W) -> Result<()>
    where
        W: std::io::Write,
    {
        writeln!(&mut out, "# Configuration for longboard")?;
        serde_yaml::to_writer(&mut out, &Config::default())?;
        writeln!(&mut out)?;
        Ok(())
    }

    /// Get the default location of the config file.
    pub fn default_path() -> PathBuf {
        if cfg!(debug_assertions) {
            PathBuf::from("contrib/dev-config.yaml")
        } else {
            PathBuf::from("/etc/longboard/config.yaml")
        }
    }

    /// Choose a banner at random.
    pub fn choose_banner(&self) -> &Banner {
        let mut rng = thread_rng();
        &self.banners.choose(&mut rng).expect("banner list is empty")
    }

    /// Dump configuration info to the log.
    pub fn debug_log(&self) {
        use log::debug;

        debug!("  address {}", self.address);
        debug!("  port {}", self.port);
        debug!("  database url {}", self.database_url);
        debug!("  static dir {}", self.static_dir.display());
        debug!("  template dir {}", self.template_dir.display());
        debug!("  upload dir {}", self.upload_dir.display());
        debug!("  banners:");
        for banner in &self.banners {
            debug!("    banner: {}", banner.name);
        }
        debug!("  filter rules:");
        for rule in &self.filter_rules {
            debug!("    filter rule: {} => {}", rule.pattern, rule.replace_with);
        }
        if let Some(ref log_file) = self.log_file {
            debug!("  log file {}", log_file.display());
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        if cfg!(debug_assertions) {
            Config {
                static_dir: PathBuf::from("res/static/"),
                template_dir: PathBuf::from("res/templates/"),
                upload_dir: PathBuf::from("uploads"),
                banners: Vec::new(),
                address: "0.0.0.0".into(),
                port: 8000,
                database_url: "postgres://longboard:@localhost/longboard".into(),
                log_file: None,
                filter_rules: Vec::new(),
            }
        } else {
            Config {
                static_dir: PathBuf::from("/usr/share/longboard/static/"),
                template_dir: PathBuf::from("/usr/share/longboard/templates/"),
                upload_dir: PathBuf::from("/var/lib/longboard/"),
                banners: Vec::new(),
                address: "0.0.0.0".into(),
                port: 8000,
                database_url: "postgres://longboard:@localhost/longboard".into(),
                log_file: Some(PathBuf::from("/var/log/longboard/longboard.log")),
                filter_rules: Vec::new(),
            }
        }
    }
}

fn pattern_deserialize_helper<'de, D>(de: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(de).and_then(|s| {
        // Make sure that the pattern is a valid regex.
        let _ = Regex::new(&s).map_err(serde::de::Error::custom)?;
        Ok(s)
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rule {
    #[serde(deserialize_with = "pattern_deserialize_helper")]
    pub pattern: String,
    pub replace_with: String,
}
