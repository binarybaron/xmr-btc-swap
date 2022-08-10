use crate::bitcoin::{parse_rpc_error_code, RpcErrorCode, Txid, Wallet};
use crate::protocol::bob::BobState;
use crate::protocol::Database;
use anyhow::{bail, Result};
use std::convert::TryInto;
use std::sync::Arc;
use uuid::Uuid;

pub async fn cancel(
    swap_id: Uuid,
    bitcoin_wallet: Arc<Wallet>,
    db: Arc<dyn Database>,
) -> Result<(Txid, BobState)> {
    let state = db.get_state(swap_id).await?.try_into()?;

    let state6 = match state {
        BobState::BtcLocked { state3, .. } => state3.cancel(),
        BobState::XmrLockProofReceived { state, .. } => state.cancel(),
        BobState::XmrLocked(state4) => state4.cancel(),
        BobState::EncSigSent(state4) => state4.cancel(),
        BobState::CancelTimelockExpired(state6) => state6,
        BobState::BtcRefunded(state6) => state6,
        BobState::BtcCancelled(state6) => state6,

        BobState::Started { .. }
        | BobState::SwapSetupCompleted(_)
        | BobState::BtcRedeemed(_)
        | BobState::XmrRedeemed { .. }
        | BobState::BtcPunished { .. }
        | BobState::SafelyAborted => bail!(
            "Cannot cancel swap {} because it is in state {} which is not refundable.",
            swap_id,
            state
        ),
    };

    tracing::info!(%swap_id, "Manually cancelling swap");

    match state6.submit_tx_cancel(bitcoin_wallet.as_ref()).await {
        Ok(txid) => {
            let state = BobState::BtcCancelled(state6);
            db.insert_latest_state(swap_id, state.clone().into())
                .await?;

            return Ok((txid, state));
        },
        Err(err) => {
            if let Ok(code) = parse_rpc_error_code(&err) {
                tracing::debug!(%code, "Cancel transaction broadcast was rejected by electrum server");

                // RpcErrorCode::RpcVerifyAlreadyInChain is returned when the tx has been already been published and confirmed
                // RpcErrorCode::RpcVerifyError (-25) is returned when the tx has been already been published and confirmed the refund/punish transaction has already been published
                if code == i64::from(RpcErrorCode::RpcVerifyAlreadyInChain) || code == i64::from(RpcErrorCode::RpcVerifyError) {
                    tracing::info!("Cancel transaction has already been confirmed on chain. The swap has therefore already been cancelled by Alice");
                    let txid = state6.construct_tx_cancel().unwrap().txid();
                    let state = BobState::BtcCancelled(state6);
                    db.insert_latest_state(swap_id, state.clone().into())
                        .await?;

                    return Ok((txid, state));
                }

                // RpcErrorCode::RpcVerifyError (-25) is returned when the timelock is not yet expired
                if code == i64::from(RpcErrorCode::RpcVerifyRejected) {
                    bail!("Cancel timelock is not yet expired, please try again later");
                }
            }
            bail!(err);
        }
    };
}
