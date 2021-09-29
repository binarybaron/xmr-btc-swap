use crate::database::alice::Alice;
use crate::database::bob::Bob;
use crate::protocol::alice::AliceState;
use crate::protocol::bob::BobState;
use crate::protocol::{Database, State};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::Path;
use std::sync::Arc;

mod alice;
mod bob;
mod sled;
mod sqlite;

pub use self::sled::SledDatabase;
pub use sqlite::SqliteDatabase;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum Swap {
    Alice(Alice),
    Bob(Bob),
}

impl From<State> for Swap {
    fn from(state: State) -> Self {
        match state {
            State::Alice(state) => Swap::Alice(state.into()),
            State::Bob(state) => Swap::Bob(state.into()),
        }
    }
}

impl From<Alice> for Swap {
    fn from(from: Alice) -> Self {
        Swap::Alice(from)
    }
}

impl From<Bob> for Swap {
    fn from(from: Bob) -> Self {
        Swap::Bob(from)
    }
}

impl Display for Swap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Swap::Alice(alice) => Display::fmt(alice, f),
            Swap::Bob(bob) => Display::fmt(bob, f),
        }
    }
}

impl From<Swap> for State {
    fn from(value: Swap) -> Self {
        match value {
            Swap::Alice(alice) => State::Alice(alice.into()),
            Swap::Bob(bob) => State::Bob(bob.into()),
        }
    }
}

impl From<BobState> for Swap {
    fn from(state: BobState) -> Self {
        Self::Bob(Bob::from(state))
    }
}

impl From<AliceState> for Swap {
    fn from(state: AliceState) -> Self {
        Self::Alice(Alice::from(state))
    }
}

pub async fn open_db(
    sled_path: impl AsRef<Path>,
    sqlite_path: impl AsRef<Path>,
    sled_flag: bool,
) -> Result<Arc<dyn Database + Send + Sync>> {
    // if sled exists and sqlite doesnt exist try and migrate
    // if sled and sqlite exists and the sled flag is set, use sled
    // if sled and sqlite exists, use sqlite
    match (
        sled_path.as_ref().exists(),
        sqlite_path.as_ref().exists(),
        sled_flag,
    ) {
        (true, false, false) => {
            let sled_db = SledDatabase::open(sled_path.as_ref()).await?;

            tokio::fs::File::create(&sqlite_path).await?;
            let mut sqlite = SqliteDatabase::open(sqlite_path).await?;
            sqlite.run_migrations().await?;

            let swap_states = sled_db.all().await?;
            for (swap_id, state) in swap_states.iter() {
                sqlite.insert_latest_state(*swap_id, state.clone()).await?;
            }

            let monero_addresses = sled_db.get_all_monero_addresses();
            for (swap_id, monero_address) in monero_addresses.flatten() {
                sqlite.insert_monero_address(swap_id, monero_address).await?;
            }

            let peer_addresses =  sled_db.get_all_addresses();
            for (peer_id, addresses) in peer_addresses.flatten() {
                for address in addresses {
                    sqlite.insert_address(peer_id, address).await?;
                }
            }

            let peers = sled_db.get_all_peers();
            for (swap_id, peer_id) in peers.flatten() {
                sqlite.insert_peer_id(swap_id, peer_id).await?;
            }

            Ok(Arc::new(sqlite))
        }
        (_, _, false) => {
            tokio::fs::File::create(&sqlite_path).await?;
            let mut sqlite = SqliteDatabase::open(sqlite_path).await?;
            sqlite.run_migrations().await?;
            Ok(Arc::new(sqlite))
        }
        (false, _, true) => {
            bail!("Sled database does not exist at specified location")
        }
        (true, _, true) => {
            let sled = SledDatabase::open(sled_path.as_ref()).await?;
            Ok(Arc::new(sled))
        }
    }
}
