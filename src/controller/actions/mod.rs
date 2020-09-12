use log::*;
use base64;
use async_trait::async_trait;
use std::error::Error;
use std::future::Future;
use std::fmt;
use serde::{Deserialize, Serialize};

use crate::DiscordContext;

mod webhook;
use webhook::{WebhookData, WebhookOptions};

mod echo;
use echo::{EchoData, EchoOptions};

mod react;
use react::{ReactOptions, ReactData};

mod addrole;
use addrole::{AddRoleOptions, AddRoleData};

use crate::gateway::{GatewayMessage, GatewayMessageType};

/// Each action has Options, Data


/// For executing an action
#[async_trait]
trait RunAction {
    async fn execute(&self, context: &DiscordContext) -> Result<(), String>;
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "options")]
pub enum ActionType {
    Webhook(WebhookOptions),
    Echo(EchoOptions),
    React(ReactOptions),
    AddRole(AddRoleOptions)
}

#[async_trait]
pub trait GatewayMessageHandler {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String>;
}

#[async_trait]
impl GatewayMessageHandler for ActionType {
    async fn handle(&self, context: &DiscordContext, message: &GatewayMessage) -> Result<(), String> {
        (match self {
            ActionType::Webhook(options) => options.handle(context, message),
            ActionType::Echo(options) => options.handle(context, message),
            ActionType::React(options) => options.handle(context, message),
            ActionType::AddRole(options) => options.handle(context, message)
        }).await
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


