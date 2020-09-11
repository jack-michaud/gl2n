use serde::{Deserialize, Serialize};
use serde_repr::*;

#[derive(Deserialize, Default)]
pub struct Me {
  pub id: String,
  pub username: String,
  pub avatar: String,
  pub discriminator: String,
  pub public_flags: i32,
  pub flags: i32,
  pub bot: bool,
  pub email: Option<String>,
  pub verified: bool,
  pub locale: String,
  pub mfa_enabled: bool
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Emoji {
  pub roles: Vec<()>,
  pub require_colors: Option<bool>,
  pub name: String,
  pub managed: bool,
  pub id: String,
  pub available: bool,
  pub animated: bool
}
// Raw {"name":"👍","id":null}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Guild {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub owner: Option<bool>,
    pub permissions: Option<i32>,
    pub features: Vec<()>,
    pub permissions_new: Option<String>,
    pub channels: Option<Vec<Channel>>,
    pub emojis: Option<Vec<Emoji>>
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct UnavailableGuild {
    pub id: String,
    pub unavailable: bool
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
    //pub bot: bool,
    //pub mfa_enabled: bool,
    //pub locale: String,
    //pub verified: bool,
    //pub email: String,
    //pub flags: i32,
    //pub premium_type: i32,
    //pub public_falgs: i32,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Member {
    pub user: User,
    pub nick: Option<String>,
    pub roles: Vec<String>,
    pub joined_at: String,
    pub mute: bool,
    pub hoisted_role: Option<String>,
    pub deaf: bool
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Ready {
    /// Gateway version
    pub v: i32,
    /// info about the user including email
    pub user: User,
    /// empy array
    pub private_channels: Vec<()>,
    /// the guilds the user is in
    pub guilds: Vec<UnavailableGuild>,
    /// used for resuming connections
    pub session_id: String,
    /// (shard_id, num_shards)
    /// shard information associated with this session.
    pub shard: Option<(u32, u32)>


}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Attachment {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub url: String,
    pub size: u32,
    pub proxy_url: String,
    pub id: String,
    pub filename: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub author: User,
    //member: GuildMember,
    pub content: String,
    pub timestamp: String,
    pub edited_timestamp: Option<String>,
    pub tts: bool,
    pub mention_everyone: bool,
    pub mentions: Vec<User>,
    //mention_roles: Vec<Role>
    //mention_channels: Vec<ChannelMention>
    pub attachments: Vec<Attachment>,
    //embeds: Vec<Embed>
    pub reactions: Option<Vec<Reaction>>
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Reaction {
    pub user_id: String,
    pub message_id: String,
    pub channel_id: String,
    pub guild_id: String,
    pub member: Member,
    pub emoji: ReactionEmoji
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct ReactionEmoji {
    pub name: String,
    pub id: Option<String>
}


#[derive(Serialize, Default)]
pub struct CreateMessagePayload {
    pub content: String,
    pub tts: bool,
    //embed:
}

/// https://discord.com/developers/docs/resources/channel#channel-object
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct Channel {
    pub id: String,
    #[serde(rename = "type")]
    pub _type: ChannelType,
    pub guild_id: Option<String>,
    pub position: Option<u32>,
    //permission_overwrites:
    /// name of the channel (2-100 characters)
    pub name: Option<String>,
    /// the channel topic (0-1024 characters)
    pub topic: Option<String>,
    /// whether the channel is nsfw
    pub nsfw: Option<bool>,
    /// the id of the last message sent in this channel (may not point to an existing or valid message)
    pub last_message_id: Option<String>,
    /// the bitrate (in bits) of the voice channel
    pub bitrate: Option<u64>,
    /// the user limit of the voice channel
    pub user_limit: Option<u32>,
    /// amount of seconds a user has to wait before sending another message (0-21600).
    /// bots, as well as users with the permission manage_messages or manage_channel, are unaffected
    pub rate_limit_per_user: Option<u32>,
    /// the recipients of the DM
    pub recipients: Option<Vec<User>>,
    /// icon hash
    pub icon: Option<String>,
    /// id of the DM creator
    pub owner_id: Option<String>,
    /// application id of the group DM creator if it is bot-created
    pub application_id: Option<String>,
    /// id of the parent category for a channel (each parent category can contain up to 50 channels)
    pub parent_id: Option<String>,
    /// ISO8601 timestam when the last pinned message was pinned
    pub last_pin_timestamp: Option<String>,
}

#[derive(Clone, Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum ChannelType {
    INVALID = 10,
    /// a text channel within a server
    GUILD_TEXT = 0,
    /// a direct message between users
    DM = 1,
    /// a voice channel within a server
    GUILD_VOICE = 2,
    /// a direct message between multiple users
    GROUP_DM = 3,
    /// an organizational category that contains up to 50 channels
    GUILD_CATEGORY = 4,
    /// a channel that users can follow and crosspost into their own server
    GUILD_NEWS = 5,
    /// a channel in which game developers can sell their game on Discord
    GUILD_STORE = 6,
}
impl Default for ChannelType {
    fn default() -> Self {
        ChannelType::INVALID
    }
}


/// https://discord.com/developers/docs/resources/channel#embed-object
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Embed {
    /// title of embed
    title: Option<String>,
    /// type of embed (always "rich" for webhook embeds)
    #[serde(rename = "type")]
    _type: Option<String>,
    /// description of embed
    description: Option<String>,
    /// url of embed
    url: Option<String>,
    /// timestamp of embed content
    timestamp: Option<String>,
    /// color code of the embed
    color: Option<u32>,
    /// footer information
    footer: Option<()>,
    //footer: Option<EmbedFooter>,
    /// image information
    image: Option<()>,
    //image: Option<EmbedImage>,
    /// thumbnail information
    thumbnail: Option<()>,
    //thumbnail: Option<EmbedThumbnail>,
    /// video object	video information
    video: Option<()>,
    //video: Option<EmbedVideo>,
    /// provider information
    provider: Option<()>,
    //provider: Option<EmbedProvider>,
    /// author information
    author: Option<()>,
    //author: Option<EmbedAuthor>,
    /// fields information
    fields: Option<Vec<()>>,
    //fields: Option<Vec<EmbedField>>,

}
