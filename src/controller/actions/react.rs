
use log::*;
use async_trait::async_trait;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use tokio::time::delay_for;
use std::time::Duration;

use crate::DiscordContext;
use crate::gateway::{GatewayMessage, GatewayMessageType};
use crate::controller::actions::{RunAction, GatewayMessageHandler};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactOptions {
    pub emojis: Option<Vec<String>>,
    pub custom_emojis: Option<Vec<String>>
}

#[async_trait]
impl GatewayMessageHandler for ReactOptions {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String> {
        if let Some(GatewayMessageType::MessageCreate(msg)) = message.d.clone() {
            let data = ReactData {
                meta: self.to_owned(),
                guild_id: msg.guild_id.unwrap(),
                message_id: msg.id,
                channel_id: msg.channel_id
            };
            data.execute(context).await
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Deserialize)]
pub struct ReactData {
    pub guild_id: String,
    pub channel_id: String,
    pub message_id: String,
    #[serde(flatten)]
    pub meta: ReactOptions
}

#[async_trait]
impl RunAction for ReactData {
    async fn execute(&self, context: &DiscordContext) -> Result<(), String> {
        if let Some(emojis) = &self.meta.emojis {
            for emoji in emojis.iter() {
                context.http_client.create_reaction(
                    self.channel_id.to_owned(),
                    self.message_id.to_owned(),
                    percent_encode(emoji.as_bytes(), NON_ALPHANUMERIC).collect::<String>()
                ).await;
                delay_for(Duration::from_millis(500)).await;
            }
        }
        if let Some(custom_emojis) = &self.meta.custom_emojis {
            for emoji in custom_emojis.iter() {
                let guild = context.guild_map.get(&self.guild_id).unwrap();
                // Search guild emojis
                if let Some(emojis) = guild.emojis.as_ref() {
                    debug!("Getting guild emojis...{}", emoji);
                    for searching_emoji in emojis {
                        debug!("Searching {}", searching_emoji.name);
                        if searching_emoji.name == *emoji {
                            context.http_client.create_reaction(
                                self.channel_id.to_owned(),
                                self.message_id.to_owned(),
                                format!("{}:{}", searching_emoji.name, searching_emoji.id)
                            ).await;
                            delay_for(Duration::from_millis(500)).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}


#[cfg(test)]
mod test {
    use crate::controller::actions::ActionData;
    use super::*;

    #[test]
    fn parse_react() {
        let react = r#"{"React":{"custom_emojis":["yeehaw"],"guild_id":"368933402751008771","channel_id":"705147009761280010","message_id":"736780490568368169"}}"#;
        let action = serde_json::de::from_str::<ActionData>(react).ok();
        if let Some(ActionData::React(react)) = action {
            assert!(react.meta.custom_emojis.unwrap().len() == 1);
        } else {
            assert!(false);
        }

    }
}

