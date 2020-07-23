/// This file governs "IFTTT"-like rules and flow control for events that
/// come in.
///
use regex::Regex;
use percent_encoding::{DEFAULT_ENCODE_SET, percent_encode};
use log::*;
use reqwest::header::{AUTHORIZATION};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json;
use serde_repr::*;

use tokio::time::delay_for;
use std::time::Duration;

use crate::gateway;

use crate::DiscordContext;

#[derive(Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    pub rules: Vec<RuleVariant>
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum SupportedGatewayMessages {
    GUILD_CREATE,
    READY,
    IDENTIFY,
    HEARTBEAT,
    MESSAGE_CREATE,
    HELLO,

    OTHER
}


#[derive(Clone, Serialize, Deserialize)]
pub struct WebhookOptions {
    url: String,
    // TODO Create remote serialize/deserialize definition for headermap
    //headers: HashMap<HeaderName, String>,
    //body: HashMap<String, String>
}
#[derive(Clone, Serialize, Deserialize)]
pub struct EchoOptions {
    text: String
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ReactOptions {
    emojis: Vec<String>,
    customEmojis: Vec<String>
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "options")]
pub enum Action {
    Webhook(WebhookOptions),
    Echo(EchoOptions),
    React(ReactOptions)
}

pub trait Filter {
    fn filter(&self, context: &DiscordContext, msg: &gateway::GatewayMessage) -> bool;
}

fn regex_match(reg_str: &String, string: &String) -> bool {
    Regex::new(reg_str.as_str()).unwrap().is_match(string.as_str()) 
}

#[derive(Clone, Serialize, Deserialize)]
pub struct MessageCreateFilter {
    /// Message content regex
    pub content: Option<String>,
    /// Channel name regex
    pub channel_name: Option<String>,
    /// Username regex (include # or not)
    pub username: Option<String>,
    /// Are there image attachments?
    pub attachments: Option<bool>
}
impl Filter for MessageCreateFilter {
    fn filter(&self, context: &DiscordContext, msg: &gateway::GatewayMessage) -> bool {
        match msg.d.clone().unwrap() {
            gateway::GatewayMessageType::MessageCreate(msg) => {
                if context.me.id == msg.author.id {
                    return false;
                }
                // check username 
                let author = format!("{}#{}", msg.author.username, msg.author.discriminator);
                if let Some(searched_user) = &self.username {
                    if !regex_match(&searched_user, &author) {
                        return false
                    }
                }
                // Check message content
                let content = msg.content;
                if let Some(re_content) = &self.content {
                    if !regex_match(&re_content, &content) {
                        return false;
                    }
                }
                // Check if there is an attachment
                if let Some(attachments) = &self.attachments {
                    let count = msg.attachments.len();
                    if *attachments {
                        if count == 0 {
                            return false;
                        }
                    } else {
                        if count > 0 {
                            return false;
                        }
                    }
                }

                // Check channel_name
                if let Some(searched_channel_name) = self.channel_name.as_ref() {
                    if let Some(channels) = context.guild_map.get(&msg.guild_id.clone().unwrap()).unwrap().channels.as_ref() {
                        for channel in channels {
                            if channel.id == msg.channel_id {
                                if let Some(channel_name) = channel.name.as_ref() {
                                    if !regex_match(&searched_channel_name, channel_name) {
                                        return false
                                    }
                                }
                                break
                            }
                        }
                    }
                }
                true
            },
            _ => false
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Rule<F> {
    pub action: Action,
    pub filters: F
}
impl<F> Rule<F>
where F: Filter
{
    pub fn filter(&self, context: &DiscordContext, msg: &gateway::GatewayMessage) -> bool {
        self.filters.filter(context, msg)
    }
}


#[derive(Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
#[serde(tag = "event")]
pub enum RuleVariant {
    MESSAGE_CREATE(Rule<MessageCreateFilter>)
}


// Allowed so we can panic during tests
#[allow(unreachable_patterns)]
fn event_convert<'a>(msg: gateway::GatewayMessageType) -> SupportedGatewayMessages  {
    match msg {
        gateway::GatewayMessageType::GuildCreate(_) => SupportedGatewayMessages::GUILD_CREATE,
        gateway::GatewayMessageType::Ready(_) => SupportedGatewayMessages::READY,
        gateway::GatewayMessageType::MessageCreate(_) => SupportedGatewayMessages::MESSAGE_CREATE,
        gateway::GatewayMessageType::Hello(_) => SupportedGatewayMessages::HELLO,
        gateway::GatewayMessageType::InvalidSession(_) => SupportedGatewayMessages::OTHER,
        gateway::GatewayMessageType::Reconnect(_) => SupportedGatewayMessages::OTHER,
        gateway::GatewayMessageType::Heartbeat(_) => SupportedGatewayMessages::OTHER,
        gateway::GatewayMessageType::Resumed(_) => SupportedGatewayMessages::OTHER,
        gateway::GatewayMessageType::HeartbeatAck(_) => SupportedGatewayMessages::OTHER,
        _ => panic!("Unsupported event in controller")
    }
}

pub struct Controller {
    event_map: HashMap<SupportedGatewayMessages, Vec<RuleVariant>>
}
impl Controller {
    pub fn new(schema: ConfigSchema) -> Self {
        let mut event_map = HashMap::<SupportedGatewayMessages, Vec<RuleVariant>>::new();
        for rule in schema.rules {
            let event_type = match rule.clone() {
                RuleVariant::MESSAGE_CREATE(_) => {
                    info!("Found MESSAGE_CREATE rule");
                    SupportedGatewayMessages::MESSAGE_CREATE
                }
            };

            if let Some(rules) = event_map.get_mut(&event_type) {
                rules.push(rule);
            } else {
                event_map.insert(event_type, vec![(rule)]);
            }
        }
        Controller {
            event_map
        }
    }

    pub async fn handle_event(&self, context: &DiscordContext, gateway_message: gateway::GatewayMessage) -> () {
        if let Some(payload) = gateway_message.d.clone() {
            let event_type = event_convert(payload.clone());
            if let Some(rules) = self.event_map.get(&event_type) {
                for rule in rules {
                    match rule {
                        RuleVariant::MESSAGE_CREATE(rule) => {
                            if !rule.filter(context, &gateway_message) {
                                continue;
                            };
                            match rule.action.clone() {
                                Action::Webhook(options) => {
                                    let client = reqwest::Client::new();
                                    let body = reqwest::Body::from(serde_json::ser::to_string(&gateway_message).unwrap());
                                    client.post(options.url.as_str())
                                        .body(body)
                                        .send();
                                    },
                                Action::Echo(options) => {
                                    if let gateway::GatewayMessageType::MessageCreate(msg) = payload.clone() {
                                        context.http_client.create_message(msg.channel_id, options.text);
                                    };
                                },
                                Action::React(options) => {
                                    if let gateway::GatewayMessageType::MessageCreate(msg) = payload.clone() {
                                        for emoji in options.emojis {
                                            context.http_client.create_reaction(
                                                msg.channel_id.clone(),
                                                msg.id.clone(),
                                                percent_encode(emoji.as_bytes(), DEFAULT_ENCODE_SET).collect::<String>()
                                            );
                                            delay_for(Duration::from_millis(500)).await;
                                        }
                                        for emoji in options.customEmojis {
                                            let guild = context.guild_map.get(msg.guild_id.as_ref().unwrap()).unwrap();
                                            // Search guild emojis
                                            if let Some(emojis) = guild.emojis.as_ref() {
                                                debug!("Getting guild emojis...{}", emoji);
                                                for searching_emoji in emojis {
                                                    debug!("Searching {}", searching_emoji.name);
                                                    if searching_emoji.name == emoji {
                                                        context.http_client.create_reaction(
                                                            msg.channel_id.clone(),
                                                            msg.id.clone(),
                                                            format!("{}:{}", searching_emoji.name, searching_emoji.id)
                                                        );
                                                        delay_for(Duration::from_millis(500)).await;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    };
                };
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn deserialize_config() {
        let config = ConfigSchema {
            rules: vec![RuleVariant::MESSAGE_CREATE(Rule {
                filters: MessageCreateFilter {
                    content: Some(String::from("test")),
                    channel_name: None,
                    username: None,
                    attachments: None
                },
                action: Action::Webhook(WebhookOptions {
                    url: String::from("http://localhost")
                })
            })]
        };

        assert_eq!(
            serde_json::ser::to_string(&config).unwrap(),
            r#"{"rules":[{"event":"MESSAGE_CREATE","action":{"type":"Webhook","options":{"url":"http://localhost"}},"filters":{"content":"test","channel_name":null,"username":null,"attachments":null}}]}"#
        )
    }


    use strum::IntoEnumIterator;
    #[test]
    fn support_all_gateway_events() {
        for _type in gateway::GatewayMessageType::iter() {
            event_convert(_type);
        }
    }
}
