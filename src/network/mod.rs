pub mod file_exchange;
mod request_response;

use self::file_exchange::{
    FileExchangeCodec, FileExchangeProtocol, FileExchangeRequest, FileExchangeResponse,
};
use super::wallet;
use async_std::prelude::StreamExt;
use ethers::types::Address;
use libp2p::{
    core::{
        muxing::StreamMuxerBox,
        transport::{upgrade::Version, Boxed},
        upgrade::SelectUpgrade,
    },
    dns::TokioDnsConfig,
    identity::Keypair,
    mplex::MplexConfig,
    noise,
    request_response::{
        ProtocolSupport, RequestId, RequestResponse, RequestResponseEvent, RequestResponseMessage,
        ResponseChannel,
    },
    swarm::{ConnectionHandlerUpgrErr, SwarmBuilder, SwarmEvent},
    tcp::TokioTcpConfig,
    yamux::YamuxConfig,
    Multiaddr, NetworkBehaviour, PeerId, Swarm, Transport,
};
use log::{debug, error};
use std::{collections::HashMap, time::Duration};
use tokio::{
    io, select,
    sync::{broadcast, mpsc, oneshot},
};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourEvent")]
struct Behaviour {
    file_exchange: RequestResponse<FileExchangeCodec>,
}

impl Behaviour {
    pub async fn new(keypair: &Keypair) -> Result<Self, anyhow::Error> {
        let peer_id = keypair.public().to_peer_id();

        let file_exchange = RequestResponse::new(
            FileExchangeCodec::default(),
            std::iter::once((FileExchangeProtocol, ProtocolSupport::Full)),
            Default::default(),
        );

        Ok(Behaviour { file_exchange })
    }
}

pub enum BehaviourEvent {
    FileExchange(RequestResponseEvent<FileExchangeRequest, FileExchangeResponse>),
}

impl From<RequestResponseEvent<FileExchangeRequest, FileExchangeResponse>> for BehaviourEvent {
    fn from(event: RequestResponseEvent<FileExchangeRequest, FileExchangeResponse>) -> Self {
        BehaviourEvent::FileExchange(event)
    }
}

pub struct CustomExecutor;
impl libp2p::core::Executor for CustomExecutor {
    fn exec(
        &self,
        future: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'static + Send>>,
    ) {
        tokio::task::spawn(future);
    }
}

/// Uses TCP encrypted using noise DH and MPlex for multiplexing
pub fn build_transport(identity_keypair: &Keypair) -> io::Result<Boxed<(PeerId, StreamMuxerBox)>> {
    // noise config
    let keypair = noise::Keypair::<noise::X25519>::new()
        .into_authentic(identity_keypair)
        .unwrap();
    let noise_config = noise::NoiseConfig::xx(keypair).into_authenticated();

    Ok(TokioDnsConfig::system(TokioTcpConfig::new())?
        .upgrade(Version::V1)
        .authenticate(noise_config)
        .multiplex(SelectUpgrade::new(
            YamuxConfig::default(),
            MplexConfig::new(),
        ))
        .timeout(Duration::from_secs(20))
        .map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)))
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
        .boxed())
}

pub struct Network {
    /// keypair of the node
    pub keypair: Keypair,
    /// Multiaddr of the node
    pub node_address: Option<Multiaddr>,

    swarm: Swarm<Behaviour>,

    command_sender: mpsc::Sender<Command>,
    command_receiver: mpsc::Receiver<Command>,
    network_event_sender: broadcast::Sender<NetworkEvent>,
    network_event_receiver: broadcast::Receiver<NetworkEvent>,

    pending_exchange_outbound_requests:
        HashMap<(PeerId, RequestId), oneshot::Sender<Result<FileExchangeResponse, anyhow::Error>>>,
    pending_exchange_inbound_response:
        HashMap<(PeerId, RequestId), oneshot::Sender<Result<(), anyhow::Error>>>,
    // exchange_inbound_response_channels:
    //     HashMap<(PeerId, RequestId), ResponseChannel<FileExchangeResponse>>,
}

impl Network {
    pub async fn new(keypair: Keypair, listen_on: Multiaddr) -> Result<Self, anyhow::Error> {
        // Build swarm
        let transport = build_transport(&keypair)?;
        let behaviour = Behaviour::new(&keypair).await?;
        let mut swarm = SwarmBuilder::new(transport, behaviour, keypair.public().to_peer_id())
            .executor(Box::new(CustomExecutor))
            .build();

        // Start listening on default addr
        if let Err(e) = swarm.listen_on(listen_on) {
            error!(
                "Failed to start listening on default address with error {}",
                e
            );
        }

        let (command_sender, command_receiver) = mpsc::channel(10);
        let (network_event_sender, network_event_receiver) = broadcast::channel(20);

        Ok(Self {
            keypair,
            node_address: None,
            swarm,

            command_sender,
            command_receiver,
            network_event_sender,
            network_event_receiver,

            pending_exchange_outbound_requests: Default::default(),
            pending_exchange_inbound_response: Default::default(),
            // exchange_inbound_response_channels: Default::default(),
        })
    }

    pub async fn run(mut self) {
        loop {
            select! {
                command = self.command_receiver.recv() => {
                    match command {
                        Some(val) => {
                            self.command_handler(val).await
                        },
                        None => {return},
                    }
                }
                swarm_event = self.swarm.next() => {
                    match swarm_event {
                        Some(event) => {
                            self.swarm_event_handler(event).await;
                        }
                        None => {return}
                    }
                }
            }
        }
    }

    pub fn network_event_receiver(&self) -> broadcast::Receiver<NetworkEvent> {
        self.network_event_sender.subscribe()
    }

    pub fn network_command_sender(&self) -> mpsc::Sender<Command> {
        self.command_sender.clone()
    }

    async fn command_handler(&mut self, command: Command) {
        match command {
            Command::SendFileRequest {
                peer_id,
                request,
                sender,
            } => {
                let request_id = self
                    .swarm
                    .behaviour_mut()
                    .file_exchange
                    .send_request(&peer_id, request);
                self.pending_exchange_outbound_requests
                    .insert((peer_id, request_id), sender);
            }
        }
    }

    async fn swarm_event_handler(
        &mut self,
        event: SwarmEvent<BehaviourEvent, ConnectionHandlerUpgrErr<std::io::Error>>,
    ) {
        match event {
            SwarmEvent::Behaviour(BehaviourEvent::FileExchange(event)) => match event {
                RequestResponseEvent::Message { peer, message } => match message {
                    RequestResponseMessage::Request {
                        request_id,
                        request,
                        channel,
                    } => {
                        // Send response immediately
                        let _ = self
                            .swarm
                            .behaviour_mut()
                            .file_exchange
                            .send_response(channel, FileExchangeResponse::Ack);

                        emit_event(
                            &self.network_event_sender,
                            NetworkEvent::FileExchangeRequest {
                                sender_peer_id: peer,
                                request_id,
                                request,
                            },
                        )
                        .await;
                    }
                    RequestResponseMessage::Response {
                        request_id,
                        response,
                    } => {
                        if let Some(sender) = self
                            .pending_exchange_outbound_requests
                            .remove(&(peer, request_id))
                        {
                            sender.send(Ok(response));
                        } else {
                            error!(
                                "(file_exchange) response channel missing for request id {} for peer {}",
                                request_id,
                                peer
                            );
                        }
                    }
                },
                RequestResponseEvent::ResponseSent { peer, request_id } => {
                    if let Some(sender) = self
                        .pending_exchange_inbound_response
                        .remove(&(peer, request_id))
                    {
                        sender.send(Ok(()));
                    } else {
                        error!(
                            "(file_exchange) response channel missing for request id {}",
                            request_id
                        );
                    }
                }
                RequestResponseEvent::OutboundFailure {
                    peer,
                    request_id,
                    error,
                } => {
                    if let Some(sender) = self
                        .pending_exchange_outbound_requests
                        .remove(&(peer, request_id))
                    {
                        sender.send(Err(error.into()));
                    } else {
                        error!(
                            "(file_exchange) response channel missing for request id {}",
                            request_id
                        );
                    }
                }
                RequestResponseEvent::InboundFailure {
                    peer,
                    request_id,
                    error,
                } => {
                    if let Some(sender) = self
                        .pending_exchange_inbound_response
                        .remove(&(peer, request_id))
                    {
                        sender.send(Err(error.into()));
                    } else {
                        error!(
                            "(commit) response channel missing for request id {}",
                            request_id
                        );
                    }
                }
                _ => {}
            },

            SwarmEvent::IncomingConnection {
                local_addr,
                send_back_addr,
            } => {
                debug!(
                    "(swarm) incoming connection {:?} {:?} ",
                    local_addr, send_back_addr
                );
            }
            SwarmEvent::ConnectionEstablished {
                peer_id,
                endpoint,
                num_established,
                ..
            } => {
                debug!(
                    "(swarm) connection established {:?} {:?} {:?} ",
                    peer_id, endpoint, num_established
                );
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                endpoint,
                num_established,
                cause,
            } => {
                debug!(
                    "(swarm) connection closed {:?} {:?} {:?} {:?} ",
                    peer_id, endpoint, num_established, cause
                );
            }
            SwarmEvent::NewListenAddr {
                listener_id,
                address,
            } => {
                debug!(
                    "(swarm) new listener id {:?} and addr {:?} ",
                    listener_id, address
                );
                self.node_address = Some(address);
                // self.network_event_sender.send(NetworkEvent::NewListenAddr { listener_id, address}).await.expect("Network evvent message dropped");
            }
            _ => {}
        }
    }
}

async fn emit_event(sender: &broadcast::Sender<NetworkEvent>, event: NetworkEvent) {
    if sender.send(event).is_err() {
        error!("Network evnent failed: Network event receiver dropped");
    }
}

#[derive(Debug)]
pub enum Command {
    SendFileRequest {
        peer_id: PeerId,
        request: FileExchangeRequest,
        sender: oneshot::Sender<anyhow::Result<FileExchangeResponse>>,
    },
}

#[derive(Debug, Clone)]
pub enum NetworkEvent {
    FileExchangeRequest {
        sender_peer_id: PeerId,
        request_id: RequestId,
        request: FileExchangeRequest,
    },
}
