use crate::monero::Scalar;
use crate::network::cbor_request_response::CborCodec;
use crate::{asb, cli};
use libp2p::core::ProtocolName;
use libp2p::request_response::{
    ProtocolSupport, RequestResponse, RequestResponseConfig, RequestResponseEvent,
    RequestResponseMessage,
};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const PROTOCOL: &str = "/comit/xmr/btc/cooperative_xmr_redeem_after_punish/1.0.0";
type OutEvent = RequestResponseEvent<Request, Response>;
type Message = RequestResponseMessage<Request, Response>;

pub type Behaviour = RequestResponse<CborCodec<CooperativeXmrRedeemProtocol, Request, Response>>;

#[derive(Debug, Clone, Copy, Default)]
pub struct CooperativeXmrRedeemProtocol;

impl ProtocolName for CooperativeXmrRedeemProtocol {
    fn protocol_name(&self) -> &[u8] {
        PROTOCOL.as_bytes()
    }
}

#[derive(Debug, thiserror::Error, Clone, Serialize, Deserialize)]
pub enum CooperativeXmrRedeemRejectReason {
    #[error("Alice does not have a record of the swap")]
    UnknownSwap,
    #[error("Alice rejected the request because it deemed it malicious")]
    MaliciousRequest,
    #[error("Alice is in a state where a cooperative redeem is not possible")]
    SwapInvalidState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub swap_id: Uuid,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Response {
    Fullfilled {
        swap_id: Uuid,
        s_a: Scalar,
    },
    Rejected {
        swap_id: Uuid,
        reason: CooperativeXmrRedeemRejectReason,
    },
}
pub fn alice() -> Behaviour {
    Behaviour::new(
        CborCodec::default(),
        vec![(CooperativeXmrRedeemProtocol, ProtocolSupport::Inbound)],
        RequestResponseConfig::default(),
    )
}

pub fn bob() -> Behaviour {
    Behaviour::new(
        CborCodec::default(),
        vec![(CooperativeXmrRedeemProtocol, ProtocolSupport::Outbound)],
        RequestResponseConfig::default(),
    )
}

impl From<(PeerId, Message)> for asb::OutEvent {
    fn from((peer, message): (PeerId, Message)) -> Self {
        match message {
            Message::Request {
                request, channel, ..
            } => Self::CooperativeXmrRedeemRequested {
                swap_id: request.swap_id,
                channel,
                peer,
            },
            Message::Response { .. } => Self::unexpected_response(peer),
        }
    }
}

crate::impl_from_rr_event!(OutEvent, asb::OutEvent, PROTOCOL);

impl From<(PeerId, Message)> for cli::OutEvent {
    fn from((peer, message): (PeerId, Message)) -> Self {
        match message {
            Message::Request { .. } => Self::unexpected_request(peer),
            Message::Response {
                response,
                request_id,
            } => match response {
                Response::Fullfilled { swap_id, s_a } => Self::CooperativeXmrRedeemFulfilled {
                    id: request_id,
                    swap_id,
                    s_a,
                },
                Response::Rejected {
                    swap_id,
                    reason: error,
                } => Self::CooperativeXmrRedeemRejected {
                    id: request_id,
                    swap_id,
                    reason: error,
                },
            },
        }
    }
}

crate::impl_from_rr_event!(OutEvent, cli::OutEvent, PROTOCOL);
