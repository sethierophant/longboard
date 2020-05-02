use clap::{App, Arg, SubCommand};

use rand::{thread_rng, Rng};

use longboard::models::staff::Staff;
use longboard::{Config, Database, Result};

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
        .get_matches();

    let conf = if let Some(path) = matches.value_of("config") {
        Config::new(path)?
    } else {
        Config::new_default()?
    };

    let db = Database::new(&conf.database_uri)?;

    if let Some(matches) = matches.subcommand_matches("add-staff") {
        let pass = matches.value_of("pass").unwrap().as_bytes();
        let salt: [u8; 20] = thread_rng().gen();

        let conf = argon2::Config::default();
        let password_hash = argon2::hash_encoded(pass, &salt, &conf)?;

        db.insert_staff(&Staff {
            name: matches.value_of("name").unwrap().to_string(),
            role: matches.value_of("role").unwrap().parse().unwrap(),
            password_hash,
        })?;
    }

    if let Some(matches) = matches.subcommand_matches("remove-staff") {
        db.delete_staff(matches.value_of("name").unwrap())?;
    }

    Ok(())
}

fn main() {
    if let Err(e) = main_res() {
        eprintln!("{}", e);
    }
}
