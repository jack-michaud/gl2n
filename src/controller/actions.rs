use log::*;
use std::fmt;
use std::collections::HashMap;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::de::{Visitor, MapAccess};
use std::marker::PhantomData;
use serde::ser::SerializeMap;

use percent_encoding::{DEFAULT_ENCODE_SET, percent_encode};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio::time::delay_for;
use std::time::Duration;

use crate::DiscordContext;

use crate::gateway::{GatewayMessage, GatewayMessageType};

#[derive(Clone, Deserialize)]
pub struct WebhookResponseMessage {
    pub content: String,
    pub channel_id: String
}

#[derive(Clone, Deserialize)]
pub struct WebhookResponseReact {
    pub channel_id: String,
    pub message_id: String,
    pub emoji: String,
    pub customEmoji: String
}

#[derive(Clone, Deserialize)]
pub struct WebhookResponse {
    pub message: Option<WebhookResponseMessage>,
    pub react: Option<WebhookResponseReact>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct WebhookOptions {
    pub url: String,
    #[serde(serialize_with="serialize_header_map")]
    #[serde(deserialize_with="deserialize_header_map")]
    pub headers: HeaderMap,
    //body: HashMap<String, String>
}
fn serialize_header_map<S>(http_headers: &HeaderMap, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
    let mut map = serializer.serialize_map(Some(http_headers.len()))?;
    for (key, value) in http_headers.iter() {
        map.serialize_entry(&key.to_string(), value.to_str().unwrap())?;
    }
    map.end()
}
fn deserialize_header_map<'de, D>(deserializer: D) -> Result<HeaderMap, D::Error> where D: Deserializer<'de> {
    deserializer.deserialize_map(HeaderMapVisitor::new())
}

struct HeaderMapVisitor {
    marker: PhantomData<fn() -> HeaderMap>
}
impl HeaderMapVisitor {
    fn new() -> Self {
        HeaderMapVisitor {
            marker: PhantomData
        }
    }
}
impl<'de> Visitor<'de> for HeaderMapVisitor {
    type Value = HeaderMap;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("expect headermap")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: MapAccess<'de>
    {
        let mut header_map = HeaderMap::new();
        while let Some(entry) = map.next_entry::<String, String>()? {
            let (key, value) = entry;
            header_map.insert(
                HeaderName::from_bytes(key.as_bytes()).expect("Could not use header name in header map"),
                HeaderValue::from_str(value.as_str()).expect("Could not insert value into headermap")
            );
        }
        Ok(header_map)
    }
}

impl GatewayMessageHandler for WebhookOptions {
    fn handle(&self, context: &DiscordContext, message: &GatewayMessage) {
        let client = reqwest::Client::new();
        let body = reqwest::Body::from(serde_json::ser::to_string(&message).unwrap());
        let mut headers: HeaderMap = self.headers.clone();
        headers.insert(HeaderName::from_static("content-type"), HeaderValue::from_static("application/json"));
        client.post(self.url.as_str())
            .headers(headers)
            .body(body)
            .send()
            .map_or(None, |mut res| {
                debug!("Got successful status code");
                match res.json::<WebhookResponse>() {
                    Ok(webhook_response) => {
                        if let Some(msg) = webhook_response.message {
                            context.http_client.create_message(&msg.channel_id, &msg.content);
                        }
                    },
                    Err(e) => {
                        error!("Could not parse webhook response");
                    }
                }
                Some(res)
            });
   
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EchoOptions {
    pub text: String
}
impl GatewayMessageHandler for EchoOptions {
    fn handle(&self, context: &DiscordContext, message: &GatewayMessage) {
        if let Some(GatewayMessageType::MessageCreate(msg)) = message.d.clone() {
            context.http_client.create_message(&msg.channel_id, &self.text);
        };
    }
}


#[derive(Clone, Serialize, Deserialize)]
pub struct ReactOptions {
    pub emojis: Vec<String>,
    pub customEmojis: Vec<String>
}
impl GatewayMessageHandler for ReactOptions {
    fn handle(&self, context: &DiscordContext, message: &GatewayMessage) {
        if let Some(GatewayMessageType::MessageCreate(msg)) = message.d.clone() {
            for emoji in self.emojis.iter() {
                context.http_client.create_reaction(
                    &msg.channel_id.clone(),
                    &msg.id.clone(),
                    &percent_encode(emoji.as_bytes(), DEFAULT_ENCODE_SET).collect::<String>()
                );
                delay_for(Duration::from_millis(500));
            }
            for emoji in self.customEmojis.iter() {
                let guild = context.guild_map.get(msg.guild_id.as_ref().unwrap()).unwrap();
                // Search guild emojis
                if let Some(emojis) = guild.emojis.as_ref() {
                    debug!("Getting guild emojis...{}", emoji);
                    for searching_emoji in emojis {
                        debug!("Searching {}", searching_emoji.name);
                        if searching_emoji.name.as_str() == emoji.as_str() {
                            context.http_client.create_reaction(
                                &msg.channel_id.clone(),
                                &msg.id.clone(),
                                &format!("{}:{}", searching_emoji.name, searching_emoji.id)
                            );
                            delay_for(Duration::from_millis(500));
                        }
                    }
                }
            }
        }
    
    }
}


#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "options")]
pub enum Action {
    Webhook(WebhookOptions),
    Echo(EchoOptions),
    React(ReactOptions)
}

pub trait GatewayMessageHandler {
    fn handle(&self, context: &DiscordContext, message: &GatewayMessage);
}

impl GatewayMessageHandler for Action {
    fn handle(&self, context: &DiscordContext, message: &GatewayMessage) {
        match self {
            Action::Webhook(options) => options.handle(context, message),
            Action::Echo(options) => options.handle(context, message),
            Action::React(options) => options.handle(context, message),
        }
    }
}

