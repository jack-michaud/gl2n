/// This file governs "IFTTT"-like rules and flow control for events that
/// come in.
///
use std::error::Error;
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

mod rules;
mod actions;

use rules::RuleVariant;
use actions::{Action, WebhookResponse};

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
                },
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
                                    let mut headers: HeaderMap = options.headers.clone();
                                    headers.insert(HeaderName::from_static("content-type"), HeaderValue::from_static("application/json"));
                                    client.post(options.url.as_str())
                                        .headers(headers)
                                        .body(body)
                                        .send()
                                        .map_or(None, |mut res| {
                                            debug!("Got successful status code");
                                            res.json::<WebhookResponse>()
                                                .map_or(None, |webhook_response| {
                                                    if let Some(msg) = webhook_response.message {
                                                        if let Some(channel_id) = webhook_response.channel_id {
                                                            context.http_client.create_message(channel_id, msg);
                                                        }
                                                    };
                                                    Some(())
                                                });
                                            Some(res)
                                        });
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
    use super::rules::*;
    use super::actions::*;

    #[test]
    fn serialize_config() {
        let config = ConfigSchema {
            rules: vec![RuleVariant::MESSAGE_CREATE(Rule {
                filters: MessageCreateFilter {
                    content: Some(String::from("test")),
                    channel_name: None,
                    username: None,
                    attachments: None
                },
                action: Action::Webhook(WebhookOptions {
                    url: String::from("http://localhost"),
                    headers: HeaderMap::new()
                })
            })]
        };

        assert_eq!(
            serde_json::ser::to_string(&config).unwrap(),
            r#"{"rules":[{"event":"MESSAGE_CREATE","action":{"type":"Webhook","options":{"url":"http://localhost","headers":{}}},"filters":{"content":"test","channel_name":null,"username":null,"attachments":null}}]}"#
        )
    }


    #[test]
    fn serialize_header_config() {
        let mut headers = HeaderMap::new();
        headers.insert(HeaderName::from_static("authorization"), HeaderValue::from_static("jwt"));
        let config = ConfigSchema {
            rules: vec![RuleVariant::MESSAGE_CREATE(Rule {
                filters: MessageCreateFilter {
                    content: Some(String::from("test")),
                    channel_name: None,
                    username: None,
                    attachments: None
                },
                action: Action::Webhook(WebhookOptions {
                    url: String::from("http://localhost"),
                    headers
                })
            })]
        };

        assert_eq!(
            serde_json::ser::to_string(&config).unwrap(),
            r#"{"rules":[{"event":"MESSAGE_CREATE","action":{"type":"Webhook","options":{"url":"http://localhost","headers":{"authorization":"jwt"}}},"filters":{"content":"test","channel_name":null,"username":null,"attachments":null}}]}"#
        )
    }

    /// Invalid header name
    #[test]
    #[should_panic]
    fn deserialize_invalid_http_header() {
        let invalid_config = r#"{"rules":[{"event":"MESSAGE_CREATE","action":{"type":"Webhook","options":{"url":"http://localhost","headers":{"/":"jwt"}}},"filters":{"content":"test","channel_name":null,"username":null,"attachments":null}}]}"#;

        let config = serde_json::de::from_str::<ConfigSchema>(invalid_config);
    }


    use strum::IntoEnumIterator;
    #[test]
    fn support_all_gateway_events() {
        for _type in gateway::GatewayMessageType::iter() {
            event_convert(_type);
        }
    }
}
