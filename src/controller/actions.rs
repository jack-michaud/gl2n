use std::fmt;
use std::collections::HashMap;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize, Serializer, Deserializer};
use serde::de::{Visitor, MapAccess};
use std::marker::PhantomData;
use serde::ser::SerializeMap;

#[derive(Clone, Deserialize)]
pub struct WebhookResponse {
    pub message: Option<String>,
    pub channel_id: Option<String>
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

#[derive(Clone, Serialize, Deserialize)]
pub struct EchoOptions {
    pub text: String
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ReactOptions {
    pub emojis: Vec<String>,
    pub customEmojis: Vec<String>
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "options")]
pub enum Action {
    Webhook(WebhookOptions),
    Echo(EchoOptions),
    React(ReactOptions)
}

