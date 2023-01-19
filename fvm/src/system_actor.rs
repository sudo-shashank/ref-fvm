// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::Context;
use cid::Cid;
use fvm_ipld_blockstore::Blockstore;
use fvm_ipld_encoding::tuple::{Deserialize_tuple, Serialize_tuple};
use fvm_ipld_encoding::CborStore;
use fvm_shared::ActorID;

use crate::kernel::{ClassifyResult, Result};
use crate::state_tree::{ActorState, StateTree};

pub const SYSTEM_ACTOR_ID: ActorID = 0;
use fuzzing_tracker::instrument;
#[cfg(feature="tracing")]
// Injected during build
#[no_mangle]
extern "Rust" {
    fn set_custom_probe(line: u64) -> ();
}


#[derive(Default, Deserialize_tuple, Serialize_tuple)]
pub struct State {
    // builtin actor registry: Vec<(String, Cid)>
    pub builtin_actors: Cid,
}

impl State {
    #[cfg_attr(feature="tracing", instrument())]
    pub fn load<B>(state_tree: &StateTree<B>) -> Result<(Self, ActorState)>
    where
        B: Blockstore,
    {
        let system_act = state_tree
            .get_actor(SYSTEM_ACTOR_ID)?
            .context("system actor address could not be resolved")
            .or_fatal()?;

        let state = state_tree
            .store()
            .get_cbor(&system_act.state)
            .or_fatal()?
            .context("system actor state not found")
            .or_fatal()?;

        Ok((state, system_act))
    }
}
