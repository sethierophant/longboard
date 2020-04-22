//! App configuration.

use std::fs::{read_dir, read_to_string, File};
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::string::ToString;

use pulldown_cmark::{html::push_html, Parser};

use serde::{Deserialize, Deserializer, Serialize};

use rand::{thread_rng, Rng};

use regex::Regex;

use rocket::uri;

use crate::{Error, Result};

/// Configuration options loaded from a file.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Address to bind to.
    pub address: String,
    /// Port to bind to.
    pub port: u16,
    /// Where the site resources (styles, templates, ...) are.
    pub resource_dir: PathBuf,
    /// Where the user-uploaded files are.
    pub upload_dir: PathBuf,
    /// Where the staff-added pages are.
    pub pages_dir: Option<PathBuf>,
    /// The path to a list of user names.
    #[serde(rename = "names")]
    pub names_path: Option<PathBuf>,
    /// The path to a notice file to be displayed at the top of each board.
    #[serde(rename = "notice")]
    pub notice_path: Option<PathBuf>,
    /// URL to connect to the database.
    pub database_uri: String,
    /// File to log to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_file: Option<PathBuf>,
    /// Filter rules to apply to posts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filter_rules: Vec<Rule>,
    /// Custom styles.
    #[serde(rename = "styles")]
    pub custom_styles: Vec<String>,
    /// The file size limit for uploaded files.
    #[serde(deserialize_with = "de_file_size_limit")]
    pub file_size_limit: u64,
    /// The list of IPs to allow unconditionally.
    pub allow_list: Vec<IpAddr>,
    /// The list of IPs to block unconditionally.
    pub block_list: Vec<IpAddr>,
    /// The list of DNSBLs to use.
    pub dns_block_list: Vec<String>,
}

fn de_file_size_limit<'de, D>(de: D) -> std::result::Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(de).and_then(|s| {
        // Make sure that the pattern is a valid regex.
        let re = Regex::new("(\\d+)([kKmMgG])?").unwrap();

        if let Some(captures) = re.captures(&s) {
            let size: u64 = captures.get(0).unwrap().as_str().parse().unwrap();
            let multiplier = match captures.get(1) {
                Some(m) => match m.as_str() {
                    "K" | "k" => 2u64.pow(10),
                    "M" | "m" => 2u64.pow(20),
                    "G" | "g" => 2u64.pow(30),
                    _ => unreachable!(),
                },
                None => 1,
            };

            Ok(size * multiplier)
        } else {
            Err(serde::de::Error::custom("expected file size"))
        }
    })
}

impl Config {
    /// Load configuration from a file.
    pub fn new<P>(path: P) -> Result<Config>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        let file = File::open(path).map_err(|cause| Error::IoErrorMsg {
            cause,
            msg: format!("Couldn't open config file at {}", path.display()),
        })?;

        let conf: Config =
            serde_yaml::from_reader(file).map_err(Error::from)?;

        if !conf.resource_dir.exists() {
            return Err(Error::ConfigPathNotFound {
                name: "resource dir".to_string(),
                path: conf.resource_dir.display().to_string(),
            });
        }

        if !conf.upload_dir.exists() {
            return Err(Error::ConfigPathNotFound {
                name: "uploads dir".to_string(),
                path: conf.resource_dir.display().to_string(),
            });
        }

        if let Some(path) = &conf.pages_dir {
            if !path.exists() {
                return Err(Error::ConfigPathNotFound {
                    name: "pages dir".to_string(),
                    path: path.display().to_string(),
                });
            }
        }

        if let Some(path) = &conf.names_path {
            if !path.exists() {
                return Err(Error::ConfigPathNotFound {
                    name: "names file".to_string(),
                    path: path.display().to_string(),
                });
            }
        }

        if let Some(path) = &conf.notice_path {
            if !path.exists() {
                return Err(Error::ConfigPathNotFound {
                    name: "notice file".to_string(),
                    path: path.display().to_string(),
                });
            }
        }

        Ok(conf)
    }

    /// Load configuration from the default location.
    pub fn new_default() -> Result<Config> {
        Config::new(Config::default_path())
    }

    /// Get the default location of the config file.
    pub fn default_path() -> &'static Path {
        if cfg!(debug_assertions) {
            Path::new("contrib/config/dev.yaml")
        } else {
            Path::new("/etc/longboard/config.yaml")
        }
    }

    /// Get all of the banners.
    pub fn banners(&self) -> Result<Vec<Banner>> {
        let path = self.resource_dir.join("banners");
        let iter = read_dir(&path).map_err(|cause| Error::IoErrorMsg {
            cause,
            msg: format!("Couldn't open banners dir at {}", path.display()),
        });

        let mut banners = Vec::new();

        for entry in iter? {
            banners.push(Banner {
                name: entry?.file_name().into_string().unwrap(),
            });
        }

        Ok(banners)
    }

    /// Choose a banner at random.
    pub fn choose_banner(&self) -> Result<Banner> {
        let mut rng = thread_rng();
        let mut banners = self.banners()?;

        if !banners.is_empty() {
            Ok(banners.remove(rng.gen_range(0, banners.len())))
        } else {
            Err(Error::BannerDirEmpty)
        }
    }

    /// Get all of the added pages.
    pub fn pages(&self) -> Result<Vec<Page>> {
        if let Some(path) = &self.pages_dir {
            let iter = read_dir(path).map_err(|cause| Error::IoErrorMsg {
                cause,
                msg: format!("Couldn't open pages dir at {}", path.display()),
            });

            let mut pages = Vec::new();

            for entry in iter? {
                let entry = entry?;

                pages.push(Page {
                    name: entry
                        .path()
                        .file_stem()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                    path: entry.path(),
                })
            }

            Ok(pages)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get all of the default names for anonymous posts.
    pub fn names(&self) -> Result<Vec<String>> {
        if let Some(path) = &self.names_path {
            let file = File::open(path).map_err(|cause| Error::IoErrorMsg {
                cause,
                msg: format!("Couldn't open names file at {}", path.display()),
            });

            Ok(BufReader::new(file?)
                .lines()
                .collect::<std::io::Result<_>>()?)
        } else {
            Ok(vec!["Anonymous".to_string()])
        }
    }

    /// Choose a name at random.
    pub fn choose_name(&self) -> Result<String> {
        let mut rng = thread_rng();
        let mut names = self.names()?;

        if !names.is_empty() {
            Ok(names.remove(rng.gen_range(0, names.len())))
        } else {
            Err(Error::NamesFileEmpty)
        }
    }

    /// Get the site notice, if it exists.
    pub fn notice(&self) -> Result<Option<String>> {
        if let Some(path) = &self.notice_path {
            let contents =
                read_to_string(path).map_err(|cause| Error::IoErrorMsg {
                    cause,
                    msg: format!(
                        "Couldn't open notice file at {}",
                        path.display()
                    ),
                });

            let mut notice_html = String::new();
            push_html(&mut notice_html, Parser::new(&contents?));

            Ok(Some(notice_html))
        } else {
            Ok(None)
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        if cfg!(debug_assertions) {
            Config {
                address: "0.0.0.0".into(),
                port: 8000,
                resource_dir: PathBuf::from("res"),
                upload_dir: PathBuf::from("uploads"),
                pages_dir: None,
                database_uri: "postgres://longboard:@localhost/longboard"
                    .into(),
                log_file: None,
                names_path: None,
                notice_path: None,
                filter_rules: Vec::new(),
                custom_styles: Vec::new(),
                file_size_limit: 2u64.pow(20) * 2,
                allow_list: Vec::new(),
                block_list: Vec::new(),
                dns_block_list: Vec::new(),
            }
        } else {
            Config {
                address: "0.0.0.0".into(),
                port: 80,
                resource_dir: PathBuf::from("/var/lib/longboard"),
                upload_dir: PathBuf::from("/var/lib/longboard/uploads"),
                pages_dir: None,
                database_uri: "postgres://longboard:@localhost/longboard"
                    .into(),
                log_file: Some(PathBuf::from(
                    "/var/log/longboard/longboard.log",
                )),
                names_path: None,
                notice_path: None,
                filter_rules: Vec::new(),
                custom_styles: Vec::new(),
                file_size_limit: 2u64.pow(20) * 2,
                allow_list: Vec::new(),
                block_list: Vec::new(),
                dns_block_list: Vec::new(),
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
/// A rule for filtering/enhancing user posts.
pub struct Rule {
    #[serde(deserialize_with = "de_pattern")]
    pub pattern: String,
    pub replace_with: String,
}

fn de_pattern<'de, D>(de: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(de).and_then(|s| {
        // Make sure that the pattern is a valid regex.
        let _ = Regex::new(&s).map_err(serde::de::Error::custom)?;
        Ok(s)
    })
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

/// A custom page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub name: String,
    pub path: PathBuf,
}
