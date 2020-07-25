use serde::{Deserialize, Serialize, Serializer};
use regex::Regex;


use crate::controller::actions::{Action, GatewayMessageHandler};
use crate::DiscordContext;
use crate::gateway;

#[derive(Clone, Serialize, Deserialize)]
#[allow(non_camel_case_types)]
#[serde(tag = "event")]
pub enum RuleVariant {
    MESSAGE_CREATE(Rule<MessageCreateFilter, Action>)
}

impl GatewayMessageHandler for RuleVariant {
    fn handle(&self, context: &DiscordContext, message: &gateway::GatewayMessage) {
        match self {
            RuleVariant::MESSAGE_CREATE(rule) => {
                rule.handle(context, message)
            }
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Rule<F, A> {
    pub action: A,
    pub filters: F
}
impl<F, A> Rule<F, A>
where F: Filter,
      A: GatewayMessageHandler
{
    fn filter(&self, context: &DiscordContext, msg: &gateway::GatewayMessage) -> bool {
        self.filters.filter(context, msg)
    }
}
impl<F, A> GatewayMessageHandler for Rule<F, A>
where F: Filter,
      A: GatewayMessageHandler {
    fn handle(&self, context: &DiscordContext, msg: &gateway::GatewayMessage) {
        if self.filters.filter(context, msg) {
            self.action.handle(context, msg);
        }
    }
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
