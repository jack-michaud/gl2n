extern crate tokio;
extern crate tokio_tungstenite;
extern crate url;
extern crate reqwest;
extern crate dotenv;
extern crate serde;
extern crate env_logger;
extern crate log;
extern crate strum;
extern crate strum_macros;
extern crate serde_json;

use log::*;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

pub mod discord;
pub mod http;
pub mod gateway;
pub mod controller;


pub struct DiscordContext {
    /// The current bot user
    pub me: discord::Me,
    /// Map of guild ID to Guild object
    pub guild_map: HashMap<String, discord::Guild>,
    /// The discord http client
    pub http_client: http::HttpClient
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().unwrap();
    env_logger::init();
    let token = env::var("DISCORD_BOT_TOKEN").expect("Must supply DISCORD_BOT_TOKEN in env");
    let guild_name = env::var("GUILD_NAME").expect("Must supply GUILD_NAME in env");
    let mut guild_map: HashMap<String, discord::Guild> = HashMap::new();

    let discord = http::HttpClient::new(token.clone());
    let me = if let Ok(me) = discord.get_me() {
        info!("Logged in as {}", me.username);
        me
    } else {
        panic!("Could not initialize discord client");
    };

    discord.get_guilds_with_channels().map_or_else(|err| {
        panic!("Could not fetch guild: {}", err);
    },
    |guilds| {
        for guild in guilds {
            let key = guild.id.clone();
            for channel in guild.clone().channels.unwrap() {
                info!("Found channel: {}", channel.name.unwrap());
            }
            guild_map.insert(key, guild);
        }
    });

    // Load config and start to listen
    let mut config_string = String::new();
    File::open("./config.json").expect("Could not open config").read_to_string(&mut config_string).expect("Could not read config");

    let config = serde_json::de::from_str::<controller::ConfigSchema>(config_string.as_str()).expect("Could not parse config");

    let mut context = DiscordContext {
        guild_map,
        me,
        http_client: discord
    };
    let controller = controller::Controller::new(config);


    loop {
        let mut gw = gateway::GatewayClient::new(token.clone());
        gw.start().await.expect("Could not start bot :(");
        loop {
            if let Some(msg) = gw.next().await {
                if let Some(payload) = msg.d.as_ref() {
                    match payload {
                        gateway::GatewayMessageType::Reconnect(_) => {
                            break;
                        },
                        gateway::GatewayMessageType::GuildCreate(guild) => {
                            let guild_in_map = context.guild_map.get_mut(&guild.id);
                            match guild_in_map {
                                Some(guild_in_map) => {
                                    *guild_in_map = guild.to_owned();
                                },
                                None => {
                                    context.guild_map.insert(guild.id.clone(), guild.clone());
                                }
                            }
                        },
                        _ => {}
                    }
                }
                controller.handle_event(&context, msg).await;
                //match msg {
                //    gateway::GatewayMessageType::READY(ready) => {
                //        info!("READY: {}", ready.d.unwrap().user.username);
                //    },
                //    gateway::GatewayMessageType::MESSAGE_CREATE(message) => {
                //        let message = message.d.unwrap();
                //    },
                //    _ => {}
                //}
            }
        }
    }
}
