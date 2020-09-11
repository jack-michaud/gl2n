use log::*;
use base64;
use async_trait::async_trait;
use std::error::Error;
use std::future::Future;
use std::fmt;
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::de::{Visitor, MapAccess};
use std::marker::PhantomData;
use serde::ser::SerializeMap;

use percent_encoding::{NON_ALPHANUMERIC, percent_encode};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio::time::delay_for;
use std::time::Duration;

use crate::DiscordContext;

use crate::gateway::{GatewayMessage, GatewayMessageType};



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

#[async_trait]
impl GatewayMessageHandler for WebhookOptions {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String> {
        let data = WebhookData {
            meta: self.to_owned(),
            payload: message.to_owned()
        };
        data.execute(context).await
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Base64File {
    /// base 64 encoded content
    pub contents: String,
    /// filename with extension
    pub filename: String 
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EchoOptions {
    pub content: Option<String>,
    pub file: Option<Base64File>
}
#[async_trait]
impl GatewayMessageHandler for EchoOptions {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String> {
        match message.d.clone().unwrap() {
            GatewayMessageType::MessageCreate(msg) => {
                info!("Got message create");
                let data: EchoData = EchoData {
                    meta: self.to_owned(),
                    channel_id: msg.channel_id
                };
                data.execute(context).await
            },
            GatewayMessageType::MessageReactionAdd(react) => {
                info!("Got react add");
                let data: EchoData = EchoData {
                    meta: self.to_owned(),
                    channel_id: react.channel_id
                };
                data.execute(context).await
            },
            _ => {
                Ok(())
            }
        }
    }
}


#[derive(Clone, Deserialize)]
pub struct EchoData {
    #[serde(flatten)]
    pub meta: EchoOptions,
    pub channel_id: String
}
#[async_trait]
impl RunAction for EchoData {
    async fn execute(&self, context: &DiscordContext) -> Result<(), String> {
        info!("Executing echo data action...");
        if let Some(content) = &self.meta.content {
            context.http_client.create_message(self.channel_id.to_owned(), content.to_owned()).await;
        }
        if let Some(file) = &self.meta.file {
            match base64::decode(file.contents.as_bytes()) {
                Ok(result) => {
                    match context.http_client.send_file(self.channel_id.to_owned(), file.filename.to_owned(), result).await {
                        Ok(_) => {
                            info!("Sent!")
                        },
                        Err(e) => {
                            error!("Unable to send");
                            return Err(e.to_string());
                        }
                    };
                },
                Err(_) => {
                    error!("Unable to decode file in echo data");
                }
            }
        }
        Ok(())
    }
}


#[derive(Clone, Serialize, Deserialize)]
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

#[derive(Clone, Deserialize)]
pub struct WebhookData {
    #[serde(flatten)]
    meta: WebhookOptions,
    payload: GatewayMessage
}

#[async_trait]
impl RunAction for WebhookData {
    async fn execute(&self, context: &DiscordContext) -> Result<(), String> {
        let client = reqwest::Client::new();
        let body = reqwest::Body::from(serde_json::ser::to_string(&self.payload).unwrap());
        let mut headers: HeaderMap = self.meta.headers.clone();
        headers.insert(HeaderName::from_static("content-type"), HeaderValue::from_static("application/json"));
        let res = client.post(self.meta.url.as_str())
            .headers(headers)
            .body(body)
            .send().await;
        if let Ok(res) = res {
            debug!("Got successful status code");
            if let Ok(webhook_response) = res.json::<ActionData>().await {
                match webhook_response {
                    ActionData::Webhook(_) => Err(String::from("Webhook not allowed as action response")),
                    action => {
                        action.execute(context).await
                    }
                }
            } else {
                Err(String::from("Could not parse webhook response"))
            }
        } else {
            Err(res.err().unwrap().to_string())
        }
 
    }
}

#[derive(Clone, Deserialize)]
pub enum ActionData {
    Webhook(WebhookData),
    Echo(EchoData),
    React(ReactData)
}

#[async_trait]
impl RunAction for ActionData {
    async fn execute(&self, context: &DiscordContext) -> Result<(), String> {
        (match self {
            ActionData::Webhook(data) => data.execute(context),
            ActionData::Echo(data) => data.execute(context),
            ActionData::React(data) => data.execute(context),
        }).await
    }
}


/// For executing an action
#[async_trait]
trait RunAction {
    async fn execute(&self, context: &DiscordContext) -> Result<(), String>;
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "options")]
pub enum Action {
    Webhook(WebhookOptions),
    Echo(EchoOptions),
    React(ReactOptions)
}

#[async_trait]
pub trait GatewayMessageHandler {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String>;
}

#[async_trait]
impl GatewayMessageHandler for Action {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String> {
        (match self {
            Action::Webhook(options) => options.handle(context, message),
            Action::Echo(options) => options.handle(context, message),
            Action::React(options) => options.handle(context, message),
        }).await
    }
}



#[cfg(test)]
mod test {
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
