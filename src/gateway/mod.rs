use log::*;
use std::sync::{Mutex, Arc};
use std::error::Error;
use serde;
use serde_json::{ser, de};
use serde::{Serialize, Deserialize};
use url::Url;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc::{Sender, Receiver, channel};
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async};

use tokio::time::delay_for;
use std::time::Duration;


pub mod message;
pub use message::{
    GatewayCommand,
    GatewayCommandType,
    GatewayMessageType, 
    GatewayOpcode,
    GatewayMessage,
    IdentifyPayload,
    HelloMessage,
    HelloPayload,
    IdentifyPresencePayload,
    IdentifyPresenceGamePayload,
    IdentifyConnectionPropertiesPayload
};

const GATEWAY_URL: &'static str = "wss://gateway.discord.gg";

#[derive(PartialEq)]
enum GatewayState {
    New,
    Connected,
    Flushing,
    InvalidSession,
}

pub struct GatewayClient {
    token: String,
    session_id: Option<String>,
    seq_num: Option<u64>,
    gateway_message_rx: Receiver<GatewayMessage>,
    gateway_message_tx: Sender<GatewayCommand>,
    state: GatewayState,
    heartbeat_thread: Option<JoinHandle<()>>
}

impl GatewayClient {

    pub fn new(token: String) -> Self {
        let (_, rx) = channel::<GatewayMessage>(1);
        let (tx, _) = channel::<GatewayCommand>(1);
        GatewayClient {
            token,
            state: GatewayState::New,
            session_id: None,
            seq_num: None,
            gateway_message_rx: rx,
            gateway_message_tx: tx,
            heartbeat_thread: None
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

        // Check for messages from the gateway
        let (mut from_local_to_gateway_tx, gateway_message_rx) = channel::<GatewayMessage>(1 << 8);
        tokio::spawn(async move {
            loop {
                // If we're flushing connections, stop
                //if self.state == GatewayState::Flushing {
                //    return;
                //}
                if let Some(msg) = ws_rx.next().await {
                    if let Err(e) = msg {
                        debug!("Error from websocket: {}", e);
                        error!("Could not receive message from websocket. Killing recv thread");
                        from_local_to_gateway_tx.send(GatewayMessage {
                            op: GatewayOpcode::Reconnect,
                            d: Some(GatewayMessageType::Reconnect(())),
                            s: None,
                            t: None
                        }).await;
                        return;
                    }
                    let text = msg.unwrap().into_text().unwrap();
                    debug!("{}", text);
                    let msg = de::from_str::<GatewayMessage>(text.as_str());
                    if let Ok(msg) = msg {
                        let op = msg.op.clone();
                        debug!("Hi {:?}", op);
                        if let Err(err) = from_local_to_gateway_tx.send(msg).await {
                            error!("Unable to communicate message from gateway: {}", err);
                        };
                        // Check if this is a Reconnecting message; we'll kill if so.
                        if op == GatewayOpcode::Reconnect {
                            debug!("Closing gateway->local channel");
                            return;
                        }
                    }
                }
            }
        });

        // Send messages to the gateway
        let (gateway_message_tx, mut from_local_to_gateway_rx) = channel::<GatewayCommand>(1 << 8);
        tokio::spawn(async move {
            loop {
                if let Some(msg) = from_local_to_gateway_rx.next().await {
                    // Check if this is a Reconnecting message; we'll kill if so.
                    match msg.d {
                        GatewayCommandType::Reconnecting(_) => {
                            debug!("Closing local->gateway channel");
                            return;
                        },
                        _ => {}
                    }

                    debug!("Got some {:?}: {:?}", &msg.op, serde_json::ser::to_string(&msg));
                    match ws_tx.send(serde_json::to_string(&msg).unwrap().into()).await {
                        Ok(_) => {
                            debug!("Sent!");
                        },
                        Err(e) => {
                            error!("Got error sending to gateway: {}", e);
                            error!("Killing send thread");
                            return;
                        }
                    }
                }
            }
        });

        self.gateway_message_rx = gateway_message_rx;
        self.gateway_message_tx = gateway_message_tx;
        self.start_heartbeat(heartbeat_interval);
        if let Err(msg) = self.identify().await {
            panic!("Could not identify self; {}", msg);
        };
        self.state = GatewayState::Connected;

        Ok(())
    }

    /// Handles internal state updating.
    /// E.g. updating session IDs, reconnecting if we had a force disconnect
    async fn preprocess_gateway_message(&mut self, msg: &GatewayMessage) {
        if let Some(payload) = msg.d.as_ref() {
            match payload {
                message::GatewayMessageType::Ready(ready_msg) => {
                    self.session_id = Some(ready_msg.session_id.clone());
                },
                message::GatewayMessageType::Reconnect(_) => {
                    self.reconnect().await.unwrap();
                },
                _ => {
                    // Pass it along 
                }
            }
        };
        if let Some(seq_num) = msg.s {
            self.seq_num = Some(seq_num);
        }
    }

    pub async fn next(&mut self) -> Option<GatewayMessage> {
        if let Some(msg) = self.gateway_message_rx.next().await {
            self.preprocess_gateway_message(&msg).await;
            Some(msg)
        } else {
            None
        }
    }

    async fn send(&mut self, message: GatewayCommand) -> Result<(), tokio::sync::mpsc::error::SendError<GatewayCommand>> 
    where 
    {
        let mut sender = self.gateway_message_tx.clone();
        sender.send(message).await
    }

    pub async fn identify(&mut self) -> Result<(), tokio::sync::mpsc::error::SendError<GatewayCommand>> {
        let intents: u32 = 1 // GUILDS
            //+ (1 << 8)   // GUILD_PRESENCES (privileged)
            + (1 << 9)   // GUILD_MESSAGES
            + (1 << 10); // GUILD_MESSAGE_REACTIONS

        self.send(GatewayCommand {
            op: GatewayOpcode::Identify,
            d: GatewayCommandType::Identify(IdentifyPayload {
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
            })
        }).await
    }

    async fn reconnect(&mut self) -> Result<(), Box<dyn Error>> {
        self.state = GatewayState::Flushing;
        debug!("Got reconnect signal...");
        self.send(GatewayCommand {
            op: GatewayOpcode::Reconnect,
            d: GatewayCommandType::Reconnecting(())
        }).await.unwrap();
        Ok(())
    }

    pub fn attempt_resume(&mut self) -> Result<(), Box<dyn Error>> {
        self.gateway_message_tx.send(GatewayCommand {
            op: GatewayOpcode::Resume,
            d: GatewayCommandType::Resume(message::ResumePayload {
                token: self.token.clone(),
                session_id: self.session_id.clone().unwrap(),
                seq: self.seq_num.unwrap()
            })
        });

        Ok(())
    }

    pub fn start_heartbeat(&mut self, heartbeat_interval: u64) {
        debug!("Starting heartbeat thread at {} ms interval", heartbeat_interval);
        let mut gateway_message_tx = self.gateway_message_tx.clone();
        let heartbeat_thread = tokio::spawn(async move {
            loop {
                let heartbeat = GatewayCommand {
                    op: GatewayOpcode::Heartbeat,
                    d: GatewayCommandType::Heartbeat(()),
                };
                if let Ok(_) = gateway_message_tx.send(heartbeat).await {
                    debug!("Sent heartbeat");
                } else {
                    debug!("Failed to send heartbeat. Stopping this thread");
                    return;
                }
                delay_for(Duration::from_millis(heartbeat_interval.into())).await;
            }
        });
        self.heartbeat_thread = Some(heartbeat_thread);
    }
}


