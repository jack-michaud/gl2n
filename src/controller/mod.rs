/// This file governs "IFTTT"-like rules and flow control for events that
/// come in.
///
use log::*;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::gateway;
use crate::DiscordContext;

mod rules;
mod actions;

use rules::RuleVariant;
use actions::GatewayMessageHandler;

#[derive(Clone, Serialize, Deserialize)]
pub struct ConfigSchema {
    pub rules: Vec<RuleVariant>,
    pub guild_id: String
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum SupportedGatewayMessages {
    GUILD_CREATE,
    READY,
    IDENTIFY,
    HEARTBEAT,
    MESSAGE_CREATE,
    MESSAGE_REACTION_ADD,
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
        gateway::GatewayMessageType::MessageReactionAdd(_) => SupportedGatewayMessages::MESSAGE_REACTION_ADD,
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
    event_map: HashMap<String, HashMap<SupportedGatewayMessages, Vec<RuleVariant>>>
}
impl Controller {
    pub fn new(schemas: Vec<ConfigSchema>) -> Self {
        let mut event_map = HashMap::<String, HashMap<SupportedGatewayMessages, Vec<RuleVariant>>>::new();
        for schema in schemas {
            let mut guild_map = HashMap::<SupportedGatewayMessages, Vec<RuleVariant>>::new();
            for rule in schema.rules {
                let event_type = match rule.clone() {
                    RuleVariant::MESSAGE_CREATE(_) => {
                        info!("Found MESSAGE_CREATE rule");
                        SupportedGatewayMessages::MESSAGE_CREATE
                    },
                    RuleVariant::MESSAGE_REACTION_ADD(_) => {
                        info!("Found MESSAGE_REACTION_ADD");
                        SupportedGatewayMessages::MESSAGE_REACTION_ADD
                    }
                };

                if let Some(rules) = guild_map.get_mut(&event_type) {
                    rules.push(rule);
                } else {
                    guild_map.insert(event_type, vec![(rule)]);
                }
            }
            event_map.insert(schema.guild_id, guild_map);
        };
        Controller {
            event_map
        }
    }

    pub async fn handle_event(&self, context: &DiscordContext, gateway_message: gateway::GatewayMessage) -> () {
        if let Some(payload) = gateway_message.d.clone() {
            let event_type = event_convert(payload.clone());
            // If we cannot find a guild ID, we cannot route the message
            if let Some(payload) = &gateway_message.d {
                if let Some(guild_id) = payload.get_guild_id() {
                    if let Some(events) = self.event_map.get(&guild_id) {
                        if let Some(rules) = events.get(&event_type) {
                            for rule in rules {
                                rule.handle(context, &gateway_message).await;
                            };
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use super::rules::*;
    use super::actions::*;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

    #[test]
    fn serialize_config() {
        let config = ConfigSchema {
            guild_id: String::from("1"),
            rules: vec![RuleVariant::MESSAGE_CREATE(Rule {
                filters: MessageCreateFilter {
                    content: Some(String::from("test")),
                    channel_name: None,
                    username: None,
                    attachments: None
                },
                action: ActionType::Webhook(WebhookOptions {
                    url: String::from("http://localhost"),
                    headers: HeaderMap::new()
                })
            })]
        };

        assert_eq!(
            serde_json::ser::to_string(&config).unwrap(),
            r#"{"rules":[{"event":"MESSAGE_CREATE","action":{"type":"Webhook","options":{"url":"http://localhost","headers":{}}},"filters":{"content":"test","channel_name":null,"username":null,"attachments":null}}],"guild_id":"1"}"#
        )
    }


    #[test]
    fn serialize_header_config() {
        let mut headers = HeaderMap::new();
        headers.insert(HeaderName::from_static("authorization"), HeaderValue::from_static("jwt"));
        let config = ConfigSchema {
            guild_id: String::from("1"),
            rules: vec![RuleVariant::MESSAGE_CREATE(Rule {
                filters: MessageCreateFilter {
                    content: Some(String::from("test")),
                    channel_name: None,
                    username: None,
                    attachments: None
                },
                action: ActionType::Webhook(WebhookOptions {
                    url: String::from("http://localhost"),
                    headers
                })
            })]
        };

        assert_eq!(
            serde_json::ser::to_string(&config).unwrap(),
            r#"{"rules":[{"event":"MESSAGE_CREATE","action":{"type":"Webhook","options":{"url":"http://localhost","headers":{"authorization":"jwt"}}},"filters":{"content":"test","channel_name":null,"username":null,"attachments":null}}],"guild_id":"1"}"#
        )
    }

    /// Invalid header name
    #[test]
    #[should_panic]
    fn deserialize_invalid_http_header() {
        let invalid_config = r#"{"rules":[{"event":"MESSAGE_CREATE","action":{"type":"Webhook","options":{"url":"http://localhost","headers":{"/":"jwt"}}},"filters":{"content":"test","channel_name":null,"username":null,"attachments":null}}],"guild_id":"1"}"#;

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
