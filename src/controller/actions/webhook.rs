use log::*;
use async_trait::async_trait;
use std::error::Error;
use std::future::Future;
use std::fmt;
use serde::{Deserialize, Serialize, Serializer, Deserializer, ser::SerializeMap};
use serde::de::{Visitor, MapAccess};
use std::marker::PhantomData;

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

use crate::DiscordContext;
use crate::controller::actions::{ActionData, RunAction, GatewayMessageHandler, GatewayMessage};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::controller::rules::*;
    use crate::controller::actions::*;
    use crate::controller::*;
    use crate::gateway;
    use reqwest::header::{HeaderMap, HeaderName, HeaderValue};

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
