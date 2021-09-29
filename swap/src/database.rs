use crate::database::alice::Alice;
use crate::database::bob::Bob;
use crate::protocol::alice::AliceState;
use crate::protocol::bob::BobState;
use crate::protocol::State;
use serde::{Deserialize, Serialize};

mod alice;
mod bob;
mod sled;
mod sqlite;

pub use self::sled::SledDatabase;
pub use sqlite::SqliteDatabase;
use std::fmt::Display;

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
