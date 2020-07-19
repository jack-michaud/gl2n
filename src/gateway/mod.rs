use log::*;
use std::sync::{Mutex, Arc};
use std::error::Error;
use serde;
use serde_json::{ser, de};
use serde::{Serialize, Deserialize};
use url::Url;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio_tungstenite::{connect_async};

use tokio::time::delay_for;
use std::time::Duration;


pub mod message;
pub use message::{
    GatewayMessageType, 
    GatewayOpcode,
    GatewayMessage,
    IdentifyPayload,
    HelloMessage,
    HelloPayload,
    Null,
    IdentifyPresencePayload,
    IdentifyPresenceGamePayload,
    IdentifyConnectionPropertiesPayload
};

const GATEWAY_URL: &'static str = "wss://gateway.discord.gg";

pub struct GatewayClient {
    token: String,
    seq_num: Option<u32>,
    should_kill: Arc<Mutex<bool>>,
    gateway_message_rx: Receiver<GatewayMessage>,
    gateway_message_tx: Sender<GatewayMessage>
}

impl GatewayClient {

    fn set_seq_number(&mut self, seq: u32) {
        // Should store this somewhere
        // For easy resuming
        self.seq_num = Some(seq);
    }

    pub fn new(token: String) -> Self {
        let (tx, rx) = channel::<GatewayMessage>(1);
        GatewayClient {
            token,
            seq_num: None,
            should_kill: Arc::new(Mutex::new(false)),
            gateway_message_rx: rx,
            gateway_message_tx: tx
        }
    }

    pub async fn start(&mut self) -> Result<(), Box<dyn Error>> {
        let (socket, response) = connect_async(
            Url::parse(format!("{}/?v=6&encoding=json", GATEWAY_URL).as_str()).unwrap().into_string()
        ).await.expect("Could not connect to gateway");

        debug!("Connected to gateway server");
        debug!("Response code: {}", response.status());
        // We should receive a Hello payload telling us how often to heartbeat.
        let (mut ws_tx, mut ws_rx) = socket.split();
        let heartbeat_interval = if let Some(msg) = ws_rx.next().await {
            let msg = msg.unwrap();
            if msg.is_text() {
                let text = msg.to_text().unwrap();
                debug!("{}", text);
                match de::from_str::<HelloMessage>(text) {
                    Ok(payload) => {
                        payload.d.heartbeat_interval
                    },
                    Err(err) => {
                        panic!("Bad response from Gateway: {}", err)
                    }
                }

            } else {
                panic!("Bad response from Gateway")
            }
        } else {
            panic!("Bad response from Gateway")
        };

        // Check for messages from the gateway forever
        let (mut from_local_to_gateway_tx, gateway_message_rx) = channel::<GatewayMessage>(1 << 8);
        tokio::spawn(async move {
            loop {
                if let Some(msg) = ws_rx.next().await {
                    let text = msg.unwrap().into_text().unwrap();
                    debug!("{}", text);
                    let msg = de::from_str::<GatewayMessage>(text.as_str());
                    if let Ok(msg) = msg {
                        if let Err(err) = from_local_to_gateway_tx.send(msg).await {
                            error!("Unable to communicate message from gateway: {}", err);
                        };
                    }
                }
            }
        });

        // Send messages to the gateway
        let (gateway_message_tx, mut from_local_to_gateway_rx) = channel::<GatewayMessage>(1 << 8);
        tokio::spawn(async move {
            loop {
                if let Some(msg) = from_local_to_gateway_rx.next().await {
                    debug!("Got some message");
                    if let Ok(_) = ws_tx.send(serde_json::to_string(&msg).unwrap().into()).await {
                        info!("Sent!");
                    }
                }
            }
        });

        debug!("Starting heartbeat thread at {} ms interval", heartbeat_interval);
        self.gateway_message_rx = gateway_message_rx;
        self.gateway_message_tx = gateway_message_tx;
        self.start_heartbeat(heartbeat_interval);
        if let Err(msg) = self.identify().await {
            panic!("Could not identify self; {}", msg);
        };
        Ok(())
    }

    pub async fn next(&mut self) -> Option<GatewayMessage> {
        if let Some(msg) = self.gateway_message_rx.next().await {
            Some(msg)
        } else {
            None
        }
    }

    pub async fn main_loop(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
        }
    }

    async fn send(&mut self, message: GatewayMessage) -> Result<(), tokio::sync::mpsc::error::SendError<GatewayMessage>> 
    where 
    {
        let mut sender = self.gateway_message_tx.clone();
        sender.send(message).await
    }

    pub async fn identify(&mut self) -> Result<(), tokio::sync::mpsc::error::SendError<GatewayMessage>> {
        let intents: u32 = 1 // GUILDS
            //+ (1 << 8)   // GUILD_PRESENCES (privileged)
            + (1 << 9)   // GUILD_MESSAGES
            + (1 << 10); // GUILD_MESSAGE_REACTIONS

        self.send(GatewayMessage {
            op: GatewayOpcode::Identify,
            d: Some(GatewayMessageType::IDENTIFY(IdentifyPayload {
                token: self.token.clone(),
                presence: IdentifyPresencePayload {
                    game: IdentifyPresenceGamePayload {
                        name: String::from("GL2N Prototyping"),
                        _type: 0
                    },
                    afk: false,
                    since: None,
                    status: String::from("Got me a status")
                },
                properties: IdentifyConnectionPropertiesPayload {
                    os: String::from("linux"),
                    browser: String::from("glennbot"),
                    device: String::from("glennbot"),
                },
                intents
            })),
            s: None,
            t: None
        }).await
    }

    //fn attempt_resume(&mut self) -> Result<(), Box<dyn Error>> {
    //    Ok(())
    //}

    pub fn start_heartbeat(&mut self, heartbeat_interval: u64) {
        let mut gateway_message_tx = self.gateway_message_tx.clone();
        let heartbeat_thread = tokio::spawn(async move {
            loop {
                error!("HEARTBEAT");
                let heartbeat = GatewayMessage {
                    op: GatewayOpcode::Heartbeat,
                    d: Some(GatewayMessageType::HEARTBEAT(Null {})),
                    s: None,
                    t: None
                };
                if let Ok(_) = gateway_message_tx.send(heartbeat).await {
                    error!("Sent heartbeat");
                } else {
                    error!("Did not send");
                }
                delay_for(Duration::from_millis(heartbeat_interval.into())).await;
            }
        });
        tokio::spawn(async {
            heartbeat_thread.await.unwrap();
            error!("Stopped heartbeat.")

            // TODO
            // if we arent trying to kill the connection,
            // try to reconnect
            
        });
    }
}


