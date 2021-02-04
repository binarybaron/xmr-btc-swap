use crate::{
    network::request_response::{OneShotCodec, Request, Response, Swap, TIMEOUT},
    protocol::alice::SwapResponse,
};
use anyhow::Result;
use libp2p::{
    request_response::{
        handler::RequestProtocol, ProtocolSupport, RequestId, RequestResponse,
        RequestResponseConfig, RequestResponseEvent, RequestResponseMessage,
    },
    swarm::{NetworkBehaviourAction, NetworkBehaviourEventProcess, PollParameters},
    NetworkBehaviour, PeerId,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    task::{Context, Poll},
    time::Duration,
};
use tracing::{debug, error};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SwapRequest {
    #[serde(with = "::bitcoin::util::amount::serde::as_sat")]
    pub btc_amount: bitcoin::Amount,
}

#[derive(Copy, Clone, Debug)]
pub struct OutEvent {
    pub swap_response: SwapResponse,
}

/// A `NetworkBehaviour` that represents doing the negotiation of a swap.
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "OutEvent", poll_method = "poll")]
#[allow(missing_debug_implementations)]
pub struct Behaviour {
    rr: RequestResponse<OneShotCodec<Swap>>,
    #[behaviour(ignore)]
    events: VecDeque<OutEvent>,
}

impl Behaviour {
    pub fn send(&mut self, alice: PeerId, swap_request: SwapRequest) -> Result<RequestId> {
        let msg = Request::SwapRequest(Box::new(swap_request));
        let id = self.rr.send_request(&alice, msg);

        Ok(id)
    }

    fn poll(
        &mut self,
        _: &mut Context<'_>,
        _: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<RequestProtocol<OneShotCodec<Swap>>, OutEvent>> {
        if let Some(event) = self.events.pop_front() {
            return Poll::Ready(NetworkBehaviourAction::GenerateEvent(event));
        }

        Poll::Pending
    }
}

impl Default for Behaviour {
    fn default() -> Self {
        let timeout = Duration::from_secs(TIMEOUT);

        let mut config = RequestResponseConfig::default();
        config.set_request_timeout(timeout);

        Self {
            rr: RequestResponse::new(
                OneShotCodec::default(),
                vec![(Swap, ProtocolSupport::Outbound)],
                config,
            ),
            events: Default::default(),
        }
    }
}

impl NetworkBehaviourEventProcess<RequestResponseEvent<Request, Response>> for Behaviour {
    fn inject_event(&mut self, event: RequestResponseEvent<Request, Response>) {
        match event {
            RequestResponseEvent::Message {
                message: RequestResponseMessage::Request { .. },
                ..
            } => panic!("Bob should never get a request from Alice"),
            RequestResponseEvent::Message {
                message: RequestResponseMessage::Response { response, .. },
                ..
            } => {
                if let Response::SwapResponse(swap_response) = response {
                    debug!("Received swap response");
                    self.events.push_back(OutEvent {
                        swap_response: *swap_response,
                    });
                }
            }
            RequestResponseEvent::InboundFailure { error, .. } => {
                error!("Inbound failure: {:?}", error);
            }
            RequestResponseEvent::OutboundFailure { error, .. } => {
                error!("Outbound failure: {:?}", error);
            }
            RequestResponseEvent::ResponseSent { .. } => {
                error!("Bob does not send a swap response to Alice");
            }
        }
    }
}