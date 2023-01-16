// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
use anyhow::Context as _;
use fvm_shared::sys;
use fvm_shared::sys::out::network::NetworkContext;

use super::Context;
use crate::kernel::{ClassifyResult, Kernel, Result};
use fuzzing_tracker::instrument;
#[cfg(feature="tracing")]
// Injected during build
#[no_mangle]
extern "Rust" {
    fn set_custom_probe(line: u64) -> ();
}


// Injected during build
#[no_mangle]
extern "Rust" {
    fn set_syscall_probe(probe: &'static str) -> ();
}

/// Returns the network circ supply split as two u64 ordered in little endian.
#[instrument()]
pub fn total_fil_circ_supply(context: Context<'_, impl Kernel>) -> Result<sys::TokenAmount> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.network.total_fil_circ_supply") };
    context
        .kernel
        .total_fil_circ_supply()?
        .try_into()
        .context("circulating supply exceeds u128 limit")
        .or_fatal()
}

#[instrument()]
pub fn context(context: Context<'_, impl Kernel>) -> crate::kernel::Result<NetworkContext> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.network.context") };
    context.kernel.network_context()
}

#[instrument()]
pub fn tipset_cid(
    context: Context<'_, impl Kernel>,
    epoch: i64,
    obuf_off: u32,
    obuf_len: u32,
) -> Result<u32> {
    #[cfg(feature = "instrument-syscalls")]
    unsafe { set_syscall_probe("syscall.network.tipset_cid") };
    // We always check arguments _first_, before we do anything else.
    context.memory.check_bounds(obuf_off, obuf_len)?;

    let cid = context.kernel.tipset_cid(epoch)?;
    context.memory.write_cid(&cid, obuf_off, obuf_len)
}
