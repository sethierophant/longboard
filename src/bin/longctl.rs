use std::path::PathBuf;
use std::str::FromStr;

use clap::{builder::PossibleValuesParser, Arg, Command};

use rand::{thread_rng, Rng};

use longboard::config::{Config, ExtensionConfig, GlobalConfig};
use longboard::models::staff::{Role, Staff};
use longboard::models::SingleConnection;
use longboard::Result;

fn main_res() -> Result<()> {
    let matches = Command::new("longctl")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Control a longboard server")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .num_args(1)
                .help("Config file to use"),
        )
        .arg(
            Arg::new("extension-dir")
                .short('x')
                .long("extension-dir")
                .value_name("DIR")
                .num_args(1)
                .help("Directory of extension config files to use"),
        )
        .arg(
            Arg::new("database-uri")
                .short('u')
                .long("database-uri")
                .value_name("URI")
                .num_args(1)
                .help("URI to use to connect to the database"),
        )
        .subcommand(
            Command::new("add-staff")
                .about("Add a new staff member")
                .arg(
                    Arg::new("role")
                        .short('r')
                        .long("role")
                        .help("The authority level of the staff member")
                        .required(true)
                        .num_args(1)
                        .value_parser(PossibleValuesParser::new([
                            "j",
                            "m",
                            "a",
                            "janitor",
                            "moderator",
                            "administrator",
                        ])),
                )
                .arg(
                    Arg::new("name")
                        .short('u')
                        .long("name")
                        .help("The login name of the staff member")
                        .required(true)
                        .num_args(1),
                )
                .arg(
                    Arg::new("pass")
                        .short('p')
                        .long("pass")
                        .help("The password for the staff member")
                        .required(true)
                        .num_args(1),
                ),
        )
        .subcommand(
            Command::new("remove-staff")
                .about("Remove a staff member")
                .arg(
                    Arg::new("name")
                        .short('u')
                        .long("name")
                        .help("The login name of the staff member")
                        .required(true)
                        .num_args(1),
                ),
        )
        .subcommand(
            Command::new("check-config")
                .about("Check configuration file for errors"),
        )
        .get_matches();

    let conf_path = GlobalConfig::default_path();
    let mut extension_dir = ExtensionConfig::default_dir();

    if let Some(path) = matches.get_one::<PathBuf>("config") {
        extension_dir = path.parent().expect("bad config path").to_owned();
    }

    if let Some(dir) = matches.get_one::<PathBuf>("extension-dir") {
        extension_dir = dir.to_owned();
    }

    let mut config = Config::load(&conf_path, &extension_dir)?;

    if let Some(uri) = matches.get_one::<String>("database-uri") {
        config.global_config.database_uri = uri.to_owned();
    }

    let mut db =
        SingleConnection::establish(&config.global_config.database_uri)?;

    if let Some(matches) = matches.subcommand_matches("add-staff") {
        let pass = matches.get_one::<String>("pass").unwrap().as_bytes();
        let salt: [u8; 20] = thread_rng().gen();

        let argon_config = argon2::Config::default();
        let password_hash = argon2::hash_encoded(pass, &salt, &argon_config)?;

        db.insert_staff(&Staff {
            name: matches.get_one::<String>("name").unwrap().to_owned(),
            role: Role::from_str(matches.get_one::<String>("role").unwrap())?,
            password_hash,
        })?;
    }

    if let Some(matches) = matches.subcommand_matches("remove-staff") {
        db.delete_staff(matches.get_one::<String>("name").unwrap())?;
    }

    if matches.subcommand_matches("check-config").is_some() {
        // We've already loaded all the config files, so we know they're good.

        println!("Global configuration: {}", conf_path.display());

        if !config.extension_configs.is_empty() {
            println!("Extension configuration:");

            for ext in &config.extension_configs {
                let ext_path =
                    extension_dir.join(&ext.name).with_extension("yaml");

                println!("  - {}", ext_path.display());
            }
        }

        println!("\nAll config files are good.");
    }

    Ok(())
}

fn main() {
    if let Err(e) = main_res() {
        eprintln!("{}", e);
        std::process::exit(-1);
    }
}
