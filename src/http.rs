use serde::{Deserialize, Serialize};
use serde::de::{DeserializeOwned};
use log::*;
use std::sync::Arc;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Response, Url, Error, Method};

use crate::discord;

const BASE: &'static str = "https://discord.com/api/v7";

pub struct Route {
    path: &'static str,
    pub method: Method,
    meta: RouteInner
}

#[derive(Clone)]
struct RouteInner {
    path: Option<&'static str>,
    pub method: Option<Method>,
    channel_id: Option<String>,
    guild_id: Option<String>,
    message_id: Option<String>,
    emoji: Option<String>,
}


pub struct RouteBuilder {
    inner: RouteInner
}
impl Route {
    pub fn new() -> RouteBuilder {
        RouteBuilder {
            inner: RouteInner {
                path: None,
                method: None,
                channel_id: None,
                guild_id: None,
                message_id: None,
                emoji: None
            }
        }
    }
}

impl RouteBuilder {
    pub fn path(mut self, path: &'static str) -> Self {
        self.inner.path = Some(path);
        self
    }
    pub fn method(mut self, method: Method) -> Self {
        self.inner.method = Some(method);
        self
    }
    pub fn guild_id(mut self, guild_id: String) -> Self {
        self.inner.guild_id = Some(guild_id);
        self
    }
    pub fn channel_id(mut self, channel_id: String) -> Self {
        self.inner.channel_id = Some(channel_id);
        self
    }
    pub fn message_id(mut self, message_id: String) -> Self {
        self.inner.message_id = Some(message_id);
        self
    }
    pub fn emoji(mut self, emoji: String) -> Self {
        self.inner.emoji = Some(emoji);
        self
    }
    pub fn build(self) -> Route {
        if let None = self.inner.method {
            panic!("Must provide .method() to builder")
        }
        if let None = self.inner.path {
            panic!("Must provide .path() to builder")
        }
        let path = self.inner.path.clone().unwrap();
        let method = self.inner.method.clone().unwrap();
        Route {
            meta: self.inner,
            method,
            path
        }
    }
}

impl Into<Url> for Route {
    fn into(self) -> Url {
        let mut before_subst = String::from(format!("{}{}", 
            BASE,
            self.path
        ));
        if let Some(guild_id) = self.meta.guild_id {
            before_subst = before_subst.replace("{guild_id}", guild_id.as_str());
        }
        if let Some(channel_id) = self.meta.channel_id {
            before_subst = before_subst.replace("{channel_id}", channel_id.as_str());
        }
        if let Some(emoji) = self.meta.emoji {
            before_subst = before_subst.replace("{emoji}", emoji.as_str());
        }
        if let Some(message_id) = self.meta.message_id {
            before_subst = before_subst.replace("{message_id}", message_id.as_str());
        }
        let url = before_subst.as_str();
        debug!("{}", url);
        Url::parse(
            url
        ).ok().unwrap()

    }
}

pub struct HttpClient {
    token: Option<Arc<String>>,
    client: Arc<Client>
}

impl HttpClient {
    pub fn new(bot_token: String) -> Self {
        HttpClient {
            token: Some(Arc::new(bot_token)),
            client: Arc::new(Client::new())
        }
    }
    fn request<P: Serialize>(&self, route: Route, payload: Option<P>) -> Result<Response, Error> {
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", HeaderValue::from_str("GlennBot").unwrap());
        headers.insert("X-Ratelimit-Precision", HeaderValue::from_str("millisecond").unwrap());

        if let Some(token) = self.token.clone() {
            headers.insert(
                "Authorization",
                HeaderValue::from_str(format!("Bot {}", token.clone()).as_str()).unwrap()
            );
        }
        debug!("{:?}", headers);

        if let None = payload {
            headers.insert("Content-Length", HeaderValue::from_str("0").unwrap());
        }

        let mut request = self.client.request::<Url>(route.method.clone(), route.into());
        request = request.headers(headers);
        if let Some(payload) = payload {
            request = request.json(&payload);
        }
        match request.send() {
            Ok(resp) => {
                Ok(resp)
            },
            Err(err) => {
                if let Some(status_code) = err.status() {
                    if status_code == 429 {
                        // Rate limited!
                        warn!("(429) Got rate limited...");
                    }
                    else if status_code == 402 {
                        error!("(402) Forbidden")
                    }
                    else if status_code == 403 {
                        error!("(403) Forbidden")
                    }
                    else if status_code == 404 {
                        error!("(404) Not found")
                    }
                }
                Err(err)
            }
        }
    }

    pub fn request_and_parse<T: DeserializeOwned, P: Serialize>(
        &self, route: Route, payload: Option<P>
    ) -> Result<T, Error> {
        let resp = self.request::<P>(route, payload);
        match resp {
            Ok(mut resp) => {
                //debug!("{}", resp.text().unwrap());
                resp.json::<T>()
            },
            Err(err) => {
                //debug!("{}", err.to_string());
                Err(err)
            }
        }
    }

    pub fn get_me(&self) -> Result<discord::Me, Error> {
        self.request_and_parse::<discord::Me, ()>(Route::new()
            .path("/users/@me")
            .method(Method::GET).build(), None)
    }

    pub fn get_guilds(&self) -> Result<Vec<discord::Guild>, Error> {
        self.request_and_parse::<Vec<discord::Guild>, ()>(Route::new()
            .path("/users/@me/guilds").method(Method::GET).build(), None)
    }

    pub fn get_guild_channels(&self, guild_id: String) -> Result<Vec<discord::Channel>, Error> {
        self.request_and_parse::<Vec<discord::Channel>, ()>(Route::new()
            .path("/guilds/{guild_id}/channels")
            .method(Method::GET)
            .guild_id(guild_id)
            .build(), None)
    }

    pub fn get_guilds_with_channels(&self) -> Result<Vec<discord::Guild>, Error> {
        let guilds = self.request_and_parse::<Vec<discord::Guild>, ()>(Route::new()
            .path("/users/@me/guilds").method(Method::GET).build(), None);
        if let Ok(guilds) = guilds {
            let mut new_guilds = Vec::<discord::Guild>::new();
            for guild in guilds {
                let channels = self.get_guild_channels(guild.id.clone()).unwrap();

                let mut new_guild = guild.clone();
                new_guild.channels = Some(channels);
                new_guilds.push(new_guild);
            }
            Ok(new_guilds)
        } else {
            panic!("Unable to fetch guilds :(")
        }
    }

    pub fn get_members(&self, guild_id: String) -> Result<Vec<discord::Member>, Error> {
        self.request_and_parse::<Vec<discord::Member>, ()>(Route::new()
            .path("/guilds/{guild_id}/members?limit=100")
            .method(Method::GET)
            .guild_id(guild_id)
            .build(), None)
    }

    pub fn create_message(&self, channel_id: String, content: String) -> Result<discord::Message, Error> {
        self.request_and_parse::<discord::Message, discord::CreateMessagePayload>(Route::new()
            .path("/channels/{channel_id}/messages")
            .method(Method::POST)
            .channel_id(channel_id)
            .build(), Some(discord::CreateMessagePayload {
                content,
                tts: false
            }))
    }

    pub fn create_reaction(&self, channel_id: String, message_id: String, emoji: String) -> Result<(), Error> {
        self.request_and_parse::<(), ()>(Route::new()
            .path("/channels/{channel_id}/messages/{message_id}/reactions/{emoji}/@me")
            .method(Method::PUT)
            .channel_id(channel_id)
            .emoji(emoji)
            .message_id(message_id)
            .build(), None)
    }
}

