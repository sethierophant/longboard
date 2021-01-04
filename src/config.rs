//! App configuration.

use std::convert::TryInto;
use std::ffi::OsStr;
use std::fs::{read_dir, read_to_string, File};
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::string::ToString;

use chrono::Duration;

use mime::Mime;

use pulldown_cmark::{html::push_html, Parser};

use rand::{thread_rng, Rng};

use regex::Regex;

use rocket::request::{FromRequest, Outcome, Request};
use rocket::{uri, State};

use serde::{Deserialize, Deserializer, Serialize};

use crate::{Error, Result};

/// Longboard configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// The global configuration.
    pub global_config: GlobalConfig,
    /// Configuration for extensions.
    pub extension_configs: Vec<ExtensionConfig>,
}

impl Config {
    /// Load global and extension configuraiton from default locaitons.
    pub fn new() -> Result<Config> {
        Self::load(GlobalConfig::default_path(), ExtensionConfig::default_dir())
    }

    /// Load global and extension configuraiton from the given locaitons.
    pub fn load<P1, P2>(config_path: P1, extension_dir: P2) -> Result<Config>
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        let config_path = config_path.as_ref();
        let extension_dir = extension_dir.as_ref();

        let global_config = GlobalConfig::new(config_path)?;

        let iter =
            read_dir(extension_dir).map_err(|cause| Error::IoErrorMsg {
                cause,
                msg: format!(
                    "Couldn't open extension dir at {}",
                    extension_dir.display()
                ),
            })?;

        let mut extension_configs = Vec::new();

        for entry in iter {
            let ext_path = entry?.path();

            if ext_path == config_path {
                continue;
            }

            if ext_path.extension() != Some(OsStr::new("yaml")) {
                continue;
            }

            if ext_path.file_stem().is_some() {
                extension_configs.push(ExtensionConfig::new(ext_path)?)
            }
        }

        Ok(Config {
            global_config,
            extension_configs,
        })
    }

    /// Get the global config.
    pub fn global(&self) -> Conf {
        Conf {
            site_name: self.global_config.site_name.as_ref(),
            favicon_path: self.global_config.favicon_path.as_ref(),
            address: self.global_config.address.as_ref(),
            port: self.global_config.port,
            log_file: self.global_config.log_file.as_deref(),
            database_uri: self.global_config.database_uri.as_ref(),
            resource_dir: self.global_config.resource_dir.as_ref(),
            upload_dir: self.global_config.upload_dir.as_ref(),
            pages_dir: self.global_config.pages_dir.as_deref(),
            names_path: self.global_config.names_path.as_deref(),
            notice_path: self.global_config.notice_path.as_deref(),
            allow_uploads: self.global_config.allow_uploads,
            allow_file_types: self.global_config.allow_file_types.as_ref(),
            rate_limit_same_user: &self.global_config.rate_limit_same_user,
            rate_limit_same_content: &self
                .global_config
                .rate_limit_same_content,
            file_size_limit: self.global_config.file_size_limit,
            filter_rules: self.global_config.filter_rules.as_ref(),
            custom_styles: self.global_config.custom_styles.as_slice(),
            allow_list: self.global_config.allow_list.as_ref(),
            block_list: self.global_config.block_list.as_ref(),
            dns_block_list: self.global_config.dns_block_list.as_slice(),
            extension_name: None,
        }
    }

    /// Get the extension config with the given name, if it exists.
    pub fn extension<S>(&self, name: S) -> Option<Conf>
    where
        S: AsRef<str>,
    {
        let mut ext_conf = None;

        for ext in &self.extension_configs {
            if ext.name == name.as_ref() {
                ext_conf = Some(ext);
            }
        }

        ext_conf.map(|ext_conf| Conf {
            site_name: self.global_config.site_name.as_ref(),
            favicon_path: self.global_config.favicon_path.as_ref(),
            address: self.global_config.address.as_ref(),
            port: self.global_config.port,
            resource_dir: self.global_config.resource_dir.as_ref(),
            upload_dir: self.global_config.upload_dir.as_ref(),
            database_uri: self.global_config.database_uri.as_ref(),
            log_file: self.global_config.log_file.as_deref(),

            pages_dir: ext_conf
                .pages_dir
                .as_deref()
                .or(self.global_config.pages_dir.as_deref()),
            names_path: ext_conf
                .names_path
                .as_deref()
                .or(self.global_config.names_path.as_deref()),
            notice_path: ext_conf
                .notice_path
                .as_deref()
                .or(self.global_config.notice_path.as_deref()),
            allow_uploads: ext_conf
                .allow_uploads
                .unwrap_or(self.global_config.allow_uploads),
            allow_file_types: ext_conf
                .allow_file_types
                .as_ref()
                .unwrap_or(self.global_config.allow_file_types.as_ref()),
            file_size_limit: ext_conf
                .file_size_limit
                .unwrap_or(self.global_config.file_size_limit),
            rate_limit_same_user: ext_conf
                .rate_limit_same_user
                .as_ref()
                .unwrap_or(&self.global_config.rate_limit_same_user),
            rate_limit_same_content: ext_conf
                .rate_limit_same_user
                .as_ref()
                .unwrap_or(&self.global_config.rate_limit_same_content),
            filter_rules: ext_conf
                .filter_rules
                .as_ref()
                .unwrap_or(self.global_config.filter_rules.as_ref()),
            custom_styles: ext_conf
                .custom_styles
                .as_deref()
                .unwrap_or(self.global_config.custom_styles.as_slice()),
            allow_list: ext_conf
                .allow_list
                .as_ref()
                .unwrap_or(self.global_config.allow_list.as_ref()),
            block_list: ext_conf
                .block_list
                .as_ref()
                .unwrap_or(self.global_config.block_list.as_ref()),
            dns_block_list: ext_conf
                .dns_block_list
                .as_deref()
                .unwrap_or(self.global_config.dns_block_list.as_slice()),

            extension_name: Some(ext_conf.name.as_ref()),
        })
    }
}

/// Global site configuration options loaded from a file.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GlobalConfig {
    /// Name of the website.
    pub site_name: String,
    /// Path to the favicon.
    #[serde(rename = "favicon")]
    pub favicon_path: PathBuf,
    /// Address to bind to.
    pub address: String,
    /// Port to bind to.
    pub port: u16,
    /// File to log to.
    pub log_file: Option<PathBuf>,
    /// URL to connect to the database.
    pub database_uri: String,
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
    /// Allow users to upload files.
    pub allow_uploads: bool,
    /// Allow these file types for file uploads.
    #[serde(deserialize_with = "de_allow_file_types")]
    pub allow_file_types: Vec<Mime>,
    /// The file size limit for uploaded files.
    #[serde(deserialize_with = "de_file_size_limit")]
    pub file_size_limit: u64,
    /// How long to rate limit posts with the same IP address.
    #[serde(deserialize_with = "de_duration")]
    pub rate_limit_same_user: Duration,
    /// How long to rate limit posts with identical content.
    #[serde(deserialize_with = "de_duration")]
    pub rate_limit_same_content: Duration,
    /// Filter rules to apply to posts.
    pub filter_rules: Vec<FilterRule>,
    /// Custom styles.
    #[serde(rename = "styles")]
    pub custom_styles: Vec<String>,
    /// The list of IPs to allow unconditionally.
    pub allow_list: Vec<IpAddr>,
    /// The list of IPs to block unconditionally.
    pub block_list: Vec<IpAddr>,
    /// The list of DNSBLs to use.
    pub dns_block_list: Vec<String>,
}

impl GlobalConfig {
    /// Load configuration from a file.
    pub fn new<P>(path: P) -> Result<GlobalConfig>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        let file = File::open(path).map_err(|cause| Error::IoErrorMsg {
            cause,
            msg: format!("Couldn't open config file at {}", path.display()),
        })?;

        let conf: GlobalConfig =
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
                path: conf.upload_dir.display().to_string(),
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

    /// Get the default location of the config file.
    pub fn default_path() -> PathBuf {
        if cfg!(debug_assertions) {
            PathBuf::from("contrib/config/dev.yaml")
        } else {
            let sysconfdir =
                option_env!("sysconfdir").unwrap_or("/usr/local/etc/");

            PathBuf::from(sysconfdir)
                .join("longboard")
                .join("config.yaml")
        }
    }
}

impl Default for GlobalConfig {
    fn default() -> GlobalConfig {
        let datadir = option_env!("datadir").unwrap_or("/usr/local/share/");
        let persistdir = option_env!("persistdir").unwrap_or("/var/lib/");
        let logdir = option_env!("logdir").unwrap_or("/var/log/");

        if cfg!(debug_assertions) {
            GlobalConfig {
                site_name: "LONGBOARD".into(),
                favicon_path: PathBuf::from("res/favicon.png"),
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
                allow_uploads: false,
                allow_file_types: Vec::new(),
                filter_rules: Vec::new(),
                custom_styles: Vec::new(),
                file_size_limit: 2u64.pow(20) * 2, // 2 MiB
                rate_limit_same_user: Duration::zero(),
                rate_limit_same_content: Duration::zero(),
                allow_list: Vec::new(),
                block_list: Vec::new(),
                dns_block_list: Vec::new(),
            }
        } else {
            GlobalConfig {
                site_name: "LONGBOARD".into(),
                favicon_path: PathBuf::from(datadir)
                    .join("longboard")
                    .join("favicon.png"),
                address: "0.0.0.0".into(),
                port: 80,
                resource_dir: PathBuf::from(datadir).join("longboard"),
                upload_dir: PathBuf::from(persistdir).join("longboard"),
                pages_dir: None,
                database_uri: "postgres://longboard:@localhost/longboard"
                    .into(),
                log_file: Some(
                    PathBuf::from(logdir)
                        .join("longboard")
                        .join("longboard.log"),
                ),
                names_path: None,
                notice_path: None,
                allow_uploads: false,
                allow_file_types: Vec::new(),
                filter_rules: Vec::new(),
                custom_styles: Vec::new(),
                file_size_limit: 2u64.pow(20) * 2, // 2 MiB
                rate_limit_same_user: Duration::seconds(10),
                rate_limit_same_content: Duration::minutes(2),
                allow_list: Vec::new(),
                block_list: Vec::new(),
                dns_block_list: Vec::new(),
            }
        }
    }
}

/// Helper for deserializing durations.
fn de_duration<'de, D>(de: D) -> std::result::Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(de)
        .and_then(|s| parse_duration(s).map_err(serde::de::Error::custom))
}

/// Helper for deserializing filter rule patterns.
fn de_pattern<'de, D>(de: D) -> std::result::Result<Regex, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(de)
        .and_then(|s| Regex::new(&s).map_err(serde::de::Error::custom))
}

/// Helper for deserializing allowed file types.
fn de_allow_file_types<'de, D>(
    de: D,
) -> std::result::Result<Vec<Mime>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<String>::deserialize(de).and_then(|types| {
        parse_file_types(types).map_err(serde::de::Error::custom)
    })
}

/// Helper for deserializing the file size limit for uploaded files.
fn de_file_size_limit<'de, D>(de: D) -> std::result::Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(de).and_then(|limit| {
        parse_file_size_limit(limit).map_err(serde::de::Error::custom)
    })
}

/// A partial configuration file for extensions.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ExtensionConfig {
    /// The name of this extension.
    #[serde(skip)]
    pub name: String,
    /// Where the staff-added pages are.
    pub pages_dir: Option<PathBuf>,
    /// The path to a list of user names.
    #[serde(rename = "names")]
    pub names_path: Option<PathBuf>,
    /// The path to a notice file to be displayed at the top of each board.
    #[serde(rename = "notice")]
    pub notice_path: Option<PathBuf>,
    /// Whether to allow user file uploads.
    pub allow_uploads: Option<bool>,
    /// Allowed file types for file uploads.
    #[serde(deserialize_with = "de_option_allow_file_types")]
    pub allow_file_types: Option<Vec<Mime>>,
    /// The file size limit for uploaded files.
    #[serde(deserialize_with = "de_option_file_size_limit")]
    pub file_size_limit: Option<u64>,
    /// How long to rate limit posts with the same IP address.
    #[serde(deserialize_with = "de_option_duration")]
    pub rate_limit_same_user: Option<Duration>,
    /// How long to rate limit posts with identical content.
    #[serde(deserialize_with = "de_option_duration")]
    pub rate_limit_same_content: Option<Duration>,
    /// Filter rules to apply to posts.
    pub filter_rules: Option<Vec<FilterRule>>,
    /// Custom styles.
    #[serde(rename = "styles")]
    pub custom_styles: Option<Vec<String>>,
    /// The list of IPs to allow unconditionally.
    pub allow_list: Option<Vec<IpAddr>>,
    /// The list of IPs to block unconditionally.
    pub block_list: Option<Vec<IpAddr>>,
    /// The list of DNSBLs to use.
    pub dns_block_list: Option<Vec<String>>,
}

impl ExtensionConfig {
    /// Load an `ExtensionConfig` from the given path.
    pub fn new<P>(path: P) -> Result<ExtensionConfig>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        let file = File::open(path)?;

        let mut extension: ExtensionConfig =
            serde_yaml::from_reader(file).map_err(Error::from)?;

        let name = path
            .file_stem()
            .expect("bad extension path")
            .to_os_string()
            .into_string()
            .expect("bad utf8");

        extension.name = name;

        if let Some(path) = &extension.pages_dir {
            if !path.exists() {
                return Err(Error::ConfigPathNotFound {
                    name: "pages dir".to_string(),
                    path: path.display().to_string(),
                });
            }
        }

        if let Some(path) = &extension.names_path {
            if !path.exists() {
                return Err(Error::ConfigPathNotFound {
                    name: "names file".to_string(),
                    path: path.display().to_string(),
                });
            }
        }

        if let Some(path) = &extension.notice_path {
            if !path.exists() {
                return Err(Error::ConfigPathNotFound {
                    name: "notice file".to_string(),
                    path: path.display().to_string(),
                });
            }
        }

        Ok(extension)
    }

    /// Get the default directory where extension configs are stored.
    pub fn default_dir() -> PathBuf {
        if cfg!(debug_assertions) {
            PathBuf::from("contrib/config/")
        } else {
            let sysconfdir =
                option_env!("sysconfdir").unwrap_or("/usr/local/etc/");

            PathBuf::from(sysconfdir).join("longboard")
        }
    }
}

impl Default for ExtensionConfig {
    fn default() -> ExtensionConfig {
        ExtensionConfig {
            name: String::new(),
            pages_dir: None,
            names_path: None,
            notice_path: None,
            allow_uploads: None,
            allow_file_types: None,
            file_size_limit: None,
            rate_limit_same_user: None,
            rate_limit_same_content: None,
            filter_rules: None,
            custom_styles: None,
            allow_list: None,
            block_list: None,
            dns_block_list: None,
        }
    }
}

/// Helper for deserializing durations.
fn de_option_duration<'de, D>(
    de: D,
) -> std::result::Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(de).and_then(|s| match s {
        Some(s) => parse_duration(s)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    })
}

/// Helper for deserializing allowed file types for extension configs.
fn de_option_allow_file_types<'de, D>(
    de: D,
) -> std::result::Result<Option<Vec<Mime>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Vec<String>>::deserialize(de).and_then(|opt_types| match opt_types
    {
        Some(types) => parse_file_types(types)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    })
}

/// Helper for deserializing the file size limit for uploaded files.
fn de_option_file_size_limit<'de, D>(
    de: D,
) -> std::result::Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(de).and_then(|opt_limit| match opt_limit {
        Some(limit) => parse_file_size_limit(limit)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    })
}

#[derive(Debug, Clone, Deserialize)]
/// A rule for filtering/enhancing user posts.
pub struct FilterRule {
    #[serde(deserialize_with = "de_pattern")]
    pub pattern: Regex,
    pub replace_with: String,
}

/// A banner to be displayed at the top of the page.
#[derive(Debug, Clone, Deserialize)]
#[serde(transparent)]
pub struct Banner {
    pub name: String,
}

impl Banner {
    /// The URI of the banner.
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

/// Parse a list of MIME types.
fn parse_file_types<S>(types: Vec<S>) -> std::result::Result<Vec<Mime>, String>
where
    S: Into<String>,
{
    types
        .into_iter()
        .map(|s| {
            s.into()
                .parse::<Mime>()
                .map_err(|err| format!("couldn't parse MIME type: {}", err))
        })
        .collect::<std::result::Result<_, String>>()
}

/// Parse a file size limit.
///
/// The limit is number of bytes and an optional suffix K, M, or G for KiB, MiB,
/// or GiB.
fn parse_file_size_limit<S>(limit: S) -> std::result::Result<u64, String>
where
    S: AsRef<str>,
{
    let re = Regex::new("(\\d+)([kKmMgG])?").unwrap();

    if let Some(captures) = re.captures(limit.as_ref()) {
        let size: u64 = captures
            .get(1)
            .ok_or(String::from("invalid file size limit"))?
            .as_str()
            .parse()
            .map_err(|err| format!("invalid file size limit: {}", err))?;

        let multiplier = match captures.get(2) {
            Some(m) => match &*m.as_str().to_uppercase() {
                "K" => 2u64.pow(10),
                "M" => 2u64.pow(20),
                "G" => 2u64.pow(30),
                _ => unreachable!(),
            },
            None => 1,
        };

        Ok(size * multiplier)
    } else {
        Err(String::from("expected file size"))
    }
}

/// Parse a duration.
///
/// The duration is number followed by a suffix S, M, H, or D for seconds,
/// minutes, hours, or days.
fn parse_duration<S>(limit: S) -> std::result::Result<Duration, String>
where
    S: AsRef<str>,
{
    if limit.as_ref() == "0" {
        return Ok(Duration::zero());
    }

    let re = Regex::new("(\\d+)([sSmMhHdD])").unwrap();

    if let Some(captures) = re.captures(limit.as_ref()) {
        let size: u64 = captures
            .get(1)
            .ok_or(String::from("invalid duration"))?
            .as_str()
            .parse()
            .map_err(|err| format!("invalid duration: {}", err))?;

        Ok(match &*captures.get(2).unwrap().as_str().to_uppercase() {
            "S" => Duration::seconds(size.try_into().unwrap()),
            "M" => Duration::minutes(size.try_into().unwrap()),
            "H" => Duration::hours(size.try_into().unwrap()),
            "D" => Duration::days(size.try_into().unwrap()),
            _ => unreachable!(),
        })
    } else {
        Err(String::from("expected duration"))
    }
}

/// Like `Config`, but with borrowed values.
///
/// These values are mostly borrowed from the global config. If an extension is
/// loaded, any options that the extension sets will be borrowed from that
/// extension's config.
#[derive(Debug, Clone)]
pub struct Conf<'a> {
    /// The name of the site.
    pub site_name: &'a str,
    /// The path to the favicon.
    pub favicon_path: &'a Path,
    /// Address to bind to.
    pub address: &'a str,
    /// Port to bind to.
    pub port: u16,
    /// File to log to.
    pub log_file: Option<&'a Path>,
    /// URL to connect to the database.
    pub database_uri: &'a str,
    /// Where the site resources (styles, templates, ...) are.
    pub resource_dir: &'a Path,
    /// Where the user-uploaded files are.
    pub upload_dir: &'a Path,
    /// Where the staff-added pages are.
    pub pages_dir: Option<&'a Path>,
    /// The path to a list of user names.
    pub names_path: Option<&'a Path>,
    /// The path to a notice file to be displayed at the top of each board.
    pub notice_path: Option<&'a Path>,
    /// Whether to allow user file uploads.
    pub allow_uploads: bool,
    /// Allowed file types for file uploads.
    pub allow_file_types: &'a [Mime],
    /// The file size limit for uploaded files.
    pub file_size_limit: u64,
    /// How long to rate limit posts with the same IP address.
    pub rate_limit_same_user: &'a Duration,
    /// How long to rate limit posts with identical content.
    pub rate_limit_same_content: &'a Duration,
    /// Filter rules to apply to posts.
    pub filter_rules: &'a [FilterRule],
    /// Custom styles.
    pub custom_styles: &'a [String],
    /// The list of IPs to allow unconditionally.
    pub allow_list: &'a [IpAddr],
    /// The list of IPs to block unconditionally.
    pub block_list: &'a [IpAddr],
    /// The list of DNSBLs to use.
    pub dns_block_list: &'a [String],
    /// The extension that is loaded, if any.
    pub extension_name: Option<&'a str>,
}

impl<'a> Conf<'a> {
    /// Get all of the page banners.
    pub fn banners(&self) -> Result<Vec<Banner>> {
        let path = self.resource_dir.join("banners");
        let iter = read_dir(&path).map_err(|cause| Error::IoErrorMsg {
            cause,
            msg: format!("Couldn't open banners dir at {}", path.display()),
        })?;

        let mut banners = Vec::new();

        for entry in iter {
            banners.push(Banner {
                name: entry?.file_name().into_string().unwrap(),
            });
        }

        Ok(banners)
    }

    /// Choose a page banner at random.
    pub fn choose_banner(&self) -> Result<Banner> {
        let mut rng = thread_rng();
        let mut banners = self.banners()?;

        if !banners.is_empty() {
            Ok(banners.remove(rng.gen_range(0..banners.len())))
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
            })?;

            let mut pages = Vec::new();

            for entry in iter {
                let entry = entry?;

                let name = entry
                    .path()
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();

                if name.to_lowercase() == "home" {
                    continue;
                }

                pages.push(Page {
                    name,
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
            Ok(names.remove(rng.gen_range(0..names.len())))
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

impl<'a, 'r> FromRequest<'a, 'r> for Conf<'r> {
    type Error = Error;

    fn from_request(request: &'a Request<'r>) -> Outcome<Self, Self::Error> {
        let config = request
            .guard::<State<Config>>()
            .expect("expected config to be initialized")
            .inner();

        match request.headers().get_one("X-LONGBOARD-EXTENSION") {
            None => Outcome::Success(config.global()),

            Some(ext_name) => {
                if let Some(ext_conf) = config.extension(ext_name) {
                    Outcome::Success(ext_conf)
                } else {
                    log::warn!(
                        "Requested extension {} which doesn't exist.",
                        ext_name,
                    );

                    Outcome::Success(config.global())
                }
            }
        }
    }
}
