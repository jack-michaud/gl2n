use log::*;
use std::fmt;
use std::default::Default;
use serde::{Serialize, Deserialize, Deserializer};
use serde::de::{Visitor, MapAccess};
use serde_json::{ser, de};
use strum_macros::{EnumIter};
use crate::discord;
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::IntoEnumIterator;
//#[allow(non_camel_case_types)]
//enum GatewayEventName {
//    // GUILDS (1 << 0)
//    GUILD_CREATE,
//    GUILD_UPDATE,
//    //GUILD_DELETE,
//    //GUILD_ROLE_CREATE,
//    //GUILD_ROLE_UPDATE,
//    //GUILD_ROLE_DELETE,
//    //CHANNEL_CREATE,
//    //CHANNEL_UPDATE,
//    //CHANNEL_DELETE,
//    //CHANNEL_PINS_UPDATE,
//
//    // GUILD_MEMBERS (1 << 1)
//    GUILD_MEMBER_ADD,
//    //GUILD_MEMBER_UPDATE,
//    //GUILD_MEMBER_REMOVE,
//
//    // GUILD_BANS (1 << 2)
//    //GUILD_BAN_ADD,
//    //GUILD_BAN_REMOVE,
//
//    // GUILD_EMOJIS (1 << 3)
//    //GUILD_EMOJIS_UPDATE,
//
//    // GUILD_INTEGRATIONS (1 << 4)
//    //GUILD_INTEGRATIONS_UPDATE,
//
//    // GUILD_WEBHOOKS (1 << 5)
//    //WEBHOOKS_UPDATE,
//
//    // GUILD_INVITES (1 << 6)
//    //INVITE_CREATE,
//    //INVITE_DELETE,
//
//    // GUILD_VOICE_STATES (1 << 7)
//    //VOICE_STATE_UPDATE,
//
//    // GUILD_PRESENCES (1 << 8)
//    //PRESENCE_UPDATE,
//
//    // GUILD_MESSAGES (1 << 9)
//    MESSAGE_CREATE,
//    //MESSAGE_UPDATE,
//    //MESSAGE_DELETE,
//    //MESSAGE_DELETE_BULK,
//
//    // GUILD_MESSAGE_REACTIONS (1 << 10)
//    MESSAGE_REACTION_ADD,
//    //MESSAGE_REACTION_REMOVE,
//    //MESSAGE_REACTION_REMOVE_ALL,
//    //MESSAGE_REACTION_REMOVE_EMOJI,
//
//    // GUILD_MESSAGE_TYPING (1 << 11)
//    TYPING_START,
//
//    // DIRECT_MESSAGES (1 << 12)
//    //CHANNEL_CREATE,
//    //MESSAGE_CREATE,
//    //MESSAGE_UPDATE,
//    //MESSAGE_DELETE,
//    //CHANNEL_PINS_UPDATE,
//
//    // DIRECT_MESSAGE_REACTIONS (1 << 13)
//    //MESSAGE_REACTION_ADD,
//    //MESSAGE_REACTION_REMOVE,
//    //MESSAGE_REACTION_REMOVE_ALL,
//    //MESSAGE_REACTION_REMOVE_EMOJI,
//
//    // DIRECT_MESSAGE_TYPING (1 << 14)
//    //TYPING_START,
//}

#[derive(Debug, PartialEq, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum GatewayOpcode {
    /// Recv: An event was dispatched.
    Dispatch = 0,
    /// Send/Recv: Fired periodically by the client to keep the connection alive.
    Heartbeat = 1,
    /// Send: Starts a new session during the initial handshake.
    Identify = 2,
    /// Send: Update the client's presence.
    PresenceUpdate = 3,
    /// Send: Used to join/leave or move between voice channels.
    VoiceStateUpdate = 4,
    /// Send: Resume a previous session that was disconnected.
    Resume = 6,
    /// Recv: You should attempt to reconnect and resume immediately.
    Reconnect = 7,
    /// Send: Request information about offline guild members in a large guild.
    RequestGuildMembers = 8,
    /// Recv: The session has been invalidated. You should reconnect and identify/resume accordingly.
    InvalidSession = 9,
    /// Recv: Sent immediately after connecting, contains the heartbeat_interval to use.
    Hello = 10,
    /// Recv: Sent in response to receiving a heartbeat to acknowledge that it has been received.
    HeartbeatAck = 11,
}

pub trait GatewayPayload<'a>: Serialize + Deserialize<'a> + Clone {}


#[derive(Clone, Serialize, Debug)]
pub struct GatewayMessage {
    /// Opcode for the payload
    pub op: GatewayOpcode,
    /// JSON payload
    pub d: Option<GatewayMessageType>,
    /// Sequence number
    pub s: Option<u64>, 
    /// Event Name
    pub t: Option<String>
}

impl<'de> Deserialize<'de> for GatewayMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de> 
    {
        deserializer.deserialize_map(GatewayMessageVisitor::new())
    }
}

struct GatewayMessageVisitor;
impl GatewayMessageVisitor {
    fn new() -> Self {
        GatewayMessageVisitor {}
    }
}
impl<'de> Visitor<'de> for GatewayMessageVisitor {
    type Value = GatewayMessage;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let string: &'static str = "gateway message from discord";
        formatter.write_str(string)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where A: MapAccess<'de> 
    {
        let mut op: Option<GatewayOpcode> = None;
        let mut d_str: Option<String> = None;
        let mut d: Option<GatewayMessageType> = None;
        let mut s: Option<u64> = None;
        let mut t: Option<String> = None;

        while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
            if key == "op" {
                op = Some(de::from_str::<GatewayOpcode>(value.to_string().as_str()).unwrap());
            }
            if key == "d" {
                if !value.is_null() {
                    d_str = Some(value.to_string());
                }
            }
            if key == "t" {
                if !value.is_null() {
                    t = Some(value.to_string());
                }
            }
            if key == "s" {
                if !value.is_null() {
                    s = Some(value.as_u64().unwrap());
                }
            }
        }
        if let None = op {
            panic!("Could not find opcode");
        }

        // Deserialize GatewayMessage Payload (d)
        match op.clone().unwrap() {
            GatewayOpcode::Dispatch => {
                let d_str = d_str.unwrap();
                match &t.clone().expect("Message type is none in dispatch type")[..] {
                    "\"HELLO\"" => {
                        d = Some(GatewayMessageType::Hello(de::from_str::<HelloPayload>(d_str.as_str()).unwrap()));
                    },
                    "\"MESSAGE_REACTION_ADD\"" => {
                        d = Some(GatewayMessageType::MessageReactionAdd(de::from_str::<discord::Reaction>(d_str.as_str()).unwrap()));
                    },
                    "\"MESSAGE_CREATE\"" => {
                        d = Some(GatewayMessageType::MessageCreate(de::from_str::<discord::Message>(d_str.as_str()).unwrap()));
                    },
                    "\"GUILD_CREATE\"" => {
                        d = Some(GatewayMessageType::GuildCreate(de::from_str::<discord::Guild>(d_str.as_str()).unwrap()));
                    },
                    "\"READY\"" => {
                        d = Some(GatewayMessageType::Ready(de::from_str::<discord::Ready>(d_str.as_str()).unwrap()));
                    },
                    _ => {
                        debug!("Unhandled event... {}", t.clone().unwrap());
                    }
                }
            },
            // No payload in Heartbeat
            GatewayOpcode::Heartbeat => {
                d = Some(GatewayMessageType::Heartbeat(()))
            },
            GatewayOpcode::Reconnect => {
                d = Some(GatewayMessageType::Reconnect(()))
            },
            GatewayOpcode::InvalidSession => {
                d = Some(
                    GatewayMessageType::InvalidSession(
                        de::from_str::<bool>(d_str.unwrap().as_str()).unwrap()
                    )
                );
            },
            GatewayOpcode::Hello => {
                d = Some(
                    GatewayMessageType::Hello(
                        de::from_str::<HelloPayload>(d_str.unwrap().as_str()).unwrap()
                    )
                )
            },
            GatewayOpcode::HeartbeatAck => {
                d = Some(GatewayMessageType::HeartbeatAck(()));
            }
            // The rest is a catch all for the other opcodes
            _ => {}
        };

        Ok(GatewayMessage {
            op: op.unwrap(),
            d,
            s,
            t
        })
    }
}

#[derive(Clone, Serialize, Deserialize, EnumIter, Debug)]
//#[serde(tag = "t")]
//#[serde(untagged)]
pub enum GatewayMessageType {
    MessageReactionAdd(discord::Reaction),
    MessageCreate(discord::Message),
    GuildCreate(discord::Guild),
    Ready(discord::Ready),
    Hello(HelloPayload),
    InvalidSession(bool),
    Reconnect(()),
    Heartbeat(()),
    Resumed(()),
    HeartbeatAck(())
}
impl Default for GatewayMessageType {
    fn default() -> Self {
        GatewayMessageType::Heartbeat(())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum GatewayCommandType {
    Identify(IdentifyPayload),
    Resume(ResumePayload),
    Heartbeat(()),
    RequestGuildMembers(GuildRequestPayload),

    /// Hack. If we send this message, we'll kill the sender thread
    Reconnecting(())
}



#[derive(Debug, Clone, Serialize)]
pub struct GatewayCommand {
    pub d: GatewayCommandType,
    pub op: GatewayOpcode
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HelloMessage {
    /// Opcode for the payload
    pub op: GatewayOpcode,
    /// JSON payload
    pub d: HelloPayload,
    /// Sequence number
    pub s: Option<()>, 
    /// Event Name
    pub t: Option<()>
}

#[derive(Clone, Serialize, Deserialize, Default, Debug)]
pub struct HelloPayload {
    pub heartbeat_interval: u64
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdentifyConnectionPropertiesPayload {
    #[serde(rename = "$os")]
    pub os: String,
    #[serde(rename = "$browser")]
    pub browser: String,
    #[serde(rename = "$device")]
    pub device: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdentifyPresenceGamePayload {
    pub name: String,
    #[serde(rename = "type")]
    pub _type: u32
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdentifyPresencePayload {
    pub game: IdentifyPresenceGamePayload,
    pub status: String,
    pub since: Option<u64>,
    pub afk: bool
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IdentifyPayload {
    pub token: String,
    pub properties: IdentifyConnectionPropertiesPayload,
    pub presence: IdentifyPresencePayload,
    /// https://discord.com/developers/docs/topics/gateway#gateway-intents
    pub intents: u32
}
impl<'a> GatewayPayload<'a> for IdentifyPayload {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResumePayload {
    pub token: String,
    pub session_id: String,
    pub seq: u64
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuildRequestPayload {
    /// id of the guild(s) to get members for
    pub guild_id: Vec<String>,
    /// string that username starts with, or an empty string to return all members
    pub query: Option<String>,
    /// maximum number of members to send matching the query; a limit of 0 
    /// can be used with an empty string query to return all members	
    pub limit: u32,
    /// used to specify if we want the presences of the matched members	
    pub presences: Option<bool>,
    /// used to specify which users you wish to fetch
    pub user_ids: Option<Vec<String>>,
    /// nonce to identify the Guild Members Chunk response	
    pub nonce: Option<String>
}


#[cfg(test)]
mod test {
    use super::*;
    use serde_json::ser;


    #[test]
    fn deserialize_hello_from_gateway() {
        let hello_str = r#"{"t":null,"s":null,"op":10,"d":{"heartbeat_interval":41250,"_trace":["[\"gateway-prd-main-t2rl\",{\"micros\":0.0}]"]}}"#;
        
        let hello = de::from_str::<HelloMessage>(hello_str).unwrap();
        assert_eq!(hello.t, None);
        assert_eq!(hello.s, None);
        assert_eq!(hello.op, GatewayOpcode::Hello);
        assert_eq!(hello.d.heartbeat_interval, 41250);
    }

    #[test]
    fn deserialize_ready_from_gateway() {
        let ready_str = r#"{"t":"READY","s":1,"op":0,"d":{"v":6,"user_settings":{},"user":{"verified":true,"username":"GlennLeuteritz","mfa_enabled":false,"id":"368952148962181124","flags":0,"email":null,"discriminator":"8867","bot":true,"avatar":"0d1621c897fb531fa0295ed8ddefbc2d"},"session_id":"f386e7b70a22eec7cd795e37128be79a","relationships":[],"private_channels":[],"presences":[],"guilds":[{"unavailable":true,"id":"368933402751008771"}],"application":{"id":"368952148962181124","flags":0},"_trace":["[\"gateway-prd-main-rws7\",{\"micros\":54307,\"calls\":[\"discord-sessions-prd-1-51\",{\"micros\":52470,\"calls\":[\"start_session\",{\"micros\":49274,\"calls\":[\"api-prd-main-pmtg\",{\"micros\":45988,\"calls\":[\"get_user\",{\"micros\":7725},\"add_authorized_ip\",{\"micros\":1967},\"get_guilds\",{\"micros\":4030},\"coros_wait\",{\"micros\":1}]}]},\"guilds_connect\",{\"micros\":2,\"calls\":[]},\"presence_connect\",{\"micros\":915,\"calls\":[]}]}]}]"]}}"#;
        let ready = de::from_str::<GatewayMessage>(ready_str).unwrap();

        match ready.d.unwrap() {
            GatewayMessageType::Ready(ready) => {
                assert_eq!(ready.user.username, "GlennLeuteritz");
            },
            _ => panic!("Deserialized incorrectly")
        }
    
    }

    #[test]
    fn deserialize_message_create_from_gateway() {
        let message_str = r#"{"t":"MESSAGE_CREATE","s":3,"op":0,"d":{"type":0,"tts":false,"timestamp":"2020-07-19T20:42:30.904000+00:00","pinned":false,"nonce":"734510507435753472","mentions":[],"mention_roles":[],"mention_everyone":false,"member":{"roles":["437773472324911115"],"premium_since":null,"nick":null,"mute":false,"joined_at":"2017-10-15T01:29:37.754000+00:00","hoisted_role":null,"deaf":false},"id":"734510504860450826","flags":0,"embeds":[],"edited_timestamp":null,"content":"aaa","channel_id":"705147009761280010","author":{"username":"lomz","public_flags":0,"id":"228347641120030731","discriminator":"2555","avatar":"a4cd28fe90118475114437f18a4f7d56"},"attachments":[],"guild_id":"368933402751008771"}}"#;

        let message = de::from_str::<GatewayMessage>(message_str).unwrap();

        match message.d.unwrap() {
            GatewayMessageType::MessageCreate(msg) => {
                assert_eq!(msg.content, "aaa");
            },
            _ => panic!("Deserialized incorrectly")
        }
    }

    #[test]
    fn deserialize_message_reaction_add_from_gateway() {
        let message_str = r#"{"t":"MESSAGE_REACTION_ADD","s":5,"op":0,"d":{"user_id":"228347641120030731","message_id":"753807148777209886","member":{"user":{"username":"lomz","id":"228347641120030731","discriminator":"2555","avatar":"a4cd28fe90118475114437f18a4f7d56"},"roles":["437773472324911115"],"premium_since":null,"nick":"json michaud","mute":false,"joined_at":"2017-10-15T01:29:37.754000+00:00","hoisted_role":null,"deaf":false},"emoji":{"name":"Doggo","id":"437783545490964482"},"channel_id":"705147009761280010","guild_id":"368933402751008771"}}"#;

        let message = de::from_str::<GatewayMessage>(message_str).unwrap();

        match message.d.unwrap() {
            GatewayMessageType::MessageReactionAdd(reaction) => {
                assert_eq!(reaction.emoji.name, "Doggo");
            },
            _ => panic!("Deserialized incorrectly")
        }
    }
}


