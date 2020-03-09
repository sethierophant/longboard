use std::fs::{read_dir, read_to_string, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::string::ToString;

use serde::{Deserialize, Deserializer, Serialize};

use rand::{seq::SliceRandom, thread_rng};

use regex::Regex;

use rocket::uri;

use crate::{Error, Result};

/// Configuration for a longboard instance.
#[derive(Debug)]
pub struct Config {
    pub options: Options,
    pub banners: Vec<Banner>,
    pub names: Vec<String>,
}

impl Config {
    /// Open a config file at the given path.
    pub fn open<P>(path: P) -> Result<Config>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        let map_err = |err| {
            let msg = format!("Couldn't open config file at {}", path.display());
            Error::from_io_error(err, msg)
        };

        let reader = File::open(path).map_err(map_err)?;
        let options: Options = serde_yaml::from_reader(reader)?;

        let banners = match read_dir(options.resource_dir.join("banners")) {
            Ok(iter) => iter
                .map(|entry| {
                    Ok(Banner {
                        name: entry?.file_name().into_string().unwrap(),
                    })
                })
                .collect::<Result<_>>()?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(Error::from(e)),
        };

        let default_names_path = options.resource_dir.join("names.txt");
        let names_path = options.names_path.as_ref().unwrap_or(&default_names_path);

        let names = match read_to_string(names_path) {
            Ok(s) => s.lines().map(ToString::to_string).collect(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(Error::from(e)),
        };

        Ok(Config {
            options,
            banners,
            names,
        })
    }

    /// Get the default location of the config file.
    pub fn default_path() -> &'static Path {
        if cfg!(debug_assertions) {
            Path::new("contrib/config/dev.yaml")
        } else {
            Path::new("/etc/longboard/config.yaml")
        }
    }

    /// Choose a banner at random.
    pub fn choose_banner(&self) -> &Banner {
        let mut rng = thread_rng();
        &self.banners.choose(&mut rng).expect("banner list is empty")
    }

    /// Choose a name at random.
    pub fn choose_name(&self) -> &str {
        let mut rng = thread_rng();
        &self
            .names
            .choose(&mut rng)
            .map(|s| s.as_str())
            .unwrap_or("Anonymous")
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            options: Options::default(),
            banners: Vec::new(),
            names: Vec::new(),
        }
    }
}

/// Configuration options loaded from a file.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    /// Address to bind to
    pub address: String,
    /// Port to bind to
    pub port: u16,
    /// Where the site resources (styles, templates, ...) are.
    pub resource_dir: PathBuf,
    /// Where the user-uploaded files are.
    pub upload_dir: PathBuf,
    /// The path to a list of user names.
    pub names_path: Option<PathBuf>,
    /// URL to connect to the database
    pub database_url: String,
    /// File to log to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_file: Option<PathBuf>,
    /// Filter rules to apply to posts
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filter_rules: Vec<Rule>,
}

impl Options {
    pub fn generate<W>(mut out: W) -> Result<()>
    where
        W: Write,
    {
        writeln!(&mut out, "# Configuration for longboard")?;
        writeln!(&mut out)?;
        serde_yaml::to_writer(&mut out, &Options::default())?;
        writeln!(&mut out)?;

        Ok(())
    }
}

impl Default for Options {
    fn default() -> Options {
        if cfg!(debug_assertions) {
            Options {
                address: "0.0.0.0".into(),
                port: 8000,
                resource_dir: PathBuf::from("res/"),
                upload_dir: PathBuf::from("uploads"),
                database_url: "postgres://longboard:@localhost/longboard".into(),
                log_file: None,
                filter_rules: Vec::new(),
                names_path: None,
            }
        } else {
            Options {
                address: "0.0.0.0".into(),
                port: 8000,
                resource_dir: PathBuf::from("/etc/longboard/"),
                upload_dir: PathBuf::from("/var/lib/longboard/"),
                database_url: "postgres://longboard:@localhost/longboard".into(),
                log_file: Some(PathBuf::from("/var/log/longboard/longboard.log")),
                filter_rules: Vec::new(),
                names_path: None,
            }
        }
    }
}

fn pattern_de_helper<'de, D>(de: D) -> std::result::Result<String, D::Error>
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
    #[serde(deserialize_with = "pattern_de_helper")]
    pub pattern: String,
    pub replace_with: String,
}

/// A banner to be displayed at the top of the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Banner {
    pub name: String,
}

impl Banner {
    pub fn uri(&self) -> String {
        uri!(crate::routes::banner: PathBuf::from(&self.name)).to_string()
    }
}
