use log::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use tokio::time::delay_for;
use std::time::Duration;

use crate::DiscordContext;
use crate::gateway::{GatewayMessage, GatewayMessageType};
use crate::controller::actions::{RunAction, GatewayMessageHandler};

#[derive(Clone, Serialize, Deserialize)]
pub struct AddRoleOptions {
    pub role_name: Option<String>,
    pub role_id: Option<String>
}

#[async_trait]
impl GatewayMessageHandler for AddRoleOptions {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String> {
        if let None = message.d {
            return Ok(());
        }
        if let Some(guild_id) = message.d.clone().unwrap().get_guild_id() {
            let get_role_id = || -> Option<String> {
                if self.role_id == None {
                    if let Some(role_name) = &self.role_name {
                        if let Some(guild) = context.guild_map.get(&guild_id) {
                            if let Some(roles) = &guild.roles {
                                for role in roles {
                                    if &role.name == role_name {
                                        return Some(role.id.clone())
                                    } else {
                                        continue
                                    }
                                }
                            }
                        }
                    }
                    return None
                } else {
                    return Some(self.role_id.clone().unwrap())
                };
            };

            let user_id = match message.d.clone().unwrap() {
                GatewayMessageType::MessageCreate(msg) => {
                    Some(msg.author.id.clone())
                },
                GatewayMessageType::MessageReactionAdd(react) => {
                    Some(react.user_id.clone())
                },
                _ => None
            };

            match (user_id, get_role_id()) {
                (Some(user_id), Some(role_id)) => {
                    let data = AddRoleData {
                        user_id,
                        guild_id: guild_id.clone().to_owned(),
                        role_id,
                    };
                    return data.execute(context).await
                },
                _ => {
                    return Err(String::from("Could not get role_id or user_id for role action"))
                }
            }
        }
        Ok(())

    }
}

#[derive(Clone, Deserialize)]
pub struct AddRoleData {
    pub guild_id: String,
    pub user_id: String,
    pub role_id: String,
}

#[async_trait]
impl RunAction for AddRoleData {
    async fn execute(&self, context: &DiscordContext) -> Result<(), String> {
        context.http_client.add_guild_member_role(
            self.guild_id.clone(),
            self.user_id.clone(),
            self.role_id.clone()
        ).await.map_err(|_| String::from("Could not add guild member role"))
    }
}

