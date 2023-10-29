use std::{io::Write, sync::RwLock};

use config::{ConfigError, FileFormat};
use serde::{Deserialize, Serialize};

use indoc::indoc;

use lazy_static::lazy_static;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Website {
    pub title: String,
}

impl Default for Website {
    fn default() -> Self {
        Self {
            title: "Synced Drawing".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Host {
    pub ip: String,
    pub port: u16,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            ip: "127.0.0.1".to_string(),
            port: 8439,
        }
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub host: Host,
    pub website: Website,
}

impl Config {
    pub fn new() -> Result<Self, ConfigError> {
        let mut builder = config::Config::builder();

        match std::path::Path::new("config.toml").exists() {
            true => {
                builder = builder.add_source(config::File::new("config.toml", FileFormat::Toml));
            }
            false => {
                let mut file = std::fs::File::create("config.toml").unwrap();

                let config = Config::default();

                let config_string = toml::to_string_pretty(&config).unwrap();

                file.write_all(config_string.as_bytes()).unwrap();

                println!("config.toml created. Please edit config.toml and run again.");
                std::process::exit(0);
            }
        };

        let config: Config = builder.build().unwrap().try_deserialize()?;

        print!(indoc!(
            "
            -------------
            Configuration
            -------------
            "
        ));

        println!("{}", toml::to_string_pretty(&config).unwrap());

        Ok(config)
    }
}

lazy_static! {
    pub static ref CONFIG: RwLock<Config> = RwLock::new(Config::new().unwrap());
}
