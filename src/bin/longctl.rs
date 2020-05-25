use std::path::PathBuf;

use clap::{App, Arg, SubCommand};

use rand::{thread_rng, Rng};

use longboard::config::{Config, ExtensionConfig, GlobalConfig};
use longboard::models::{staff::Staff, SingleConnection};
use longboard::Result;

fn main_res() -> Result<()> {
    let matches = App::new("longctl")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about("Control a longboard server")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("FILE")
                .takes_value(true)
                .help("Config file to use"),
        )
        .arg(
            Arg::with_name("extension-dir")
                .short("x")
                .long("extension-dir")
                .value_name("DIR")
                .takes_value(true)
                .help("Directory of extension configs to use"),
        )
        .arg(
            Arg::with_name("database-uri")
                .short("u")
                .long("database-uri")
                .value_name("URI")
                .takes_value(true)
                .help("URI to use to connect to the database"),
        )
        .subcommand(
            SubCommand::with_name("add-staff")
                .about("Add a new staff member")
                .arg(
                    Arg::with_name("role")
                        .short("r")
                        .long("role")
                        .help("The authority level of the staff member")
                        .required(true)
                        .takes_value(true)
                        .possible_values(&[
                            "janitor",
                            "moderator",
                            "administrator",
                        ]),
                )
                .arg(
                    Arg::with_name("name")
                        .short("u")
                        .long("name")
                        .help("The login name of the staff member")
                        .required(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::with_name("pass")
                        .short("p")
                        .long("pass")
                        .help("The password for the staff member")
                        .required(true)
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("remove-staff")
                .about("Remove a staff member")
                .arg(
                    Arg::with_name("name")
                        .short("u")
                        .long("name")
                        .help("The login name of the staff member")
                        .required(true)
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("check-config")
                .about("Check configuration file for errors"),
        )
        .get_matches();

    let mut conf_path = GlobalConfig::default_path();
    let mut extension_dir = ExtensionConfig::default_dir();

    if let Some(path) = matches.value_of("config") {
        conf_path = PathBuf::from(path);
        extension_dir = conf_path.parent().expect("bad config path").to_owned();
    }

    if let Some(dir) = matches.value_of("extension-dir") {
        extension_dir = PathBuf::from(dir);
    }

    let mut config = Config::load(&conf_path, &extension_dir)?;

    if let Some(uri) = matches.value_of("database-uri") {
        config.global_config.database_uri = uri.to_string();
    }

    let db = SingleConnection::establish(&config.global_config.database_uri)?;

    if let Some(matches) = matches.subcommand_matches("add-staff") {
        let pass = matches.value_of("pass").unwrap().as_bytes();
        let salt: [u8; 20] = thread_rng().gen();

        let argon_config = argon2::Config::default();
        let password_hash = argon2::hash_encoded(pass, &salt, &argon_config)?;

        db.insert_staff(&Staff {
            name: matches.value_of("name").unwrap().to_string(),
            role: matches.value_of("role").unwrap().parse().unwrap(),
            password_hash,
        })?;
    }

    if let Some(matches) = matches.subcommand_matches("remove-staff") {
        db.delete_staff(matches.value_of("name").unwrap())?;
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
