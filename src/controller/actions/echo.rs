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
use tokio::time::delay_for;
use std::time::Duration;

use crate::DiscordContext;
use crate::controller::actions::{
    RunAction,
    GatewayMessageHandler,
    GatewayMessageType,
    GatewayMessage
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Base64File {
    /// base 64 encoded content
    pub contents: String,
    /// filename with extension
    pub filename: String 
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EchoOptions {
    pub content: Option<String>,
    pub file: Option<Base64File>
}
#[async_trait]
impl GatewayMessageHandler for EchoOptions {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String> {
        if let Some(payload) = message.d.clone() {
            if let Some(channel_id) = payload.get_channel_id() {
                let data: EchoData = EchoData {
                    meta: self.to_owned(),
                    channel_id
                };
                return data.execute(context).await
            }
        }
        Err(String::from("Could not handle message"))
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


