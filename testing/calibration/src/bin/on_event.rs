// Copyright 2021-2023 Protocol Labs
// SPDX-License-Identifier: Apache-2.0, MIT
#![feature(slice_group_by)]

use std::usize;

use fil_gas_calibration_actor::{Method, OnEventParams};
use fvm_gas_calibration::*;
use fvm_shared::event::Flags;
use rand::{thread_rng, Rng};

const CHARGE_VALIDATE: &str = "OnActorEventValidate";
const CHARGE_ACCEPT: &str = "OnActorEventAccept";
const METHOD: Method = Method::OnEvent;

fn main() {
    let entry_counts: Vec<_> = (0u32..=4).map(|n| u64::pow(2, n)).collect(); // up to 16 entries
    let sizes: Vec<_> = (0u32..=8).map(|n| u64::pow(2, n)).collect(); // up to 256 bytes

    let iterations = 500;

    let mut obs = Vec::new();

    let mut te = instantiate_tester();

    let mut rng = thread_rng();

    for entry_count in entry_counts.iter() {
        for size in sizes.iter() {
            let label = format!("{entry_count:?}");
            let params = OnEventParams {
                iterations,
                entries: *entry_count as usize,
                sizes: (*size as usize, *size as usize),
                flags: Flags::FLAG_INDEXED_ALL,
                seed: rng.gen(),
            };

            let ret = te.execute_or_die(METHOD as u64, &params);

            {
                let mut series = collect_obs(ret.clone(), CHARGE_ACCEPT, &label, *size as usize);
                //series = eliminate_outliers(series, 0.05, Eliminate::Top);
                obs.extend(series);
            };

            {
                let mut series = collect_obs(ret.clone(), CHARGE_VALIDATE, &label, *size as usize);
                //series = eliminate_outliers(series, 0.05, Eliminate::Top);
                obs.extend(series);
            };
        }
    }

    let regs = obs
        .group_by(|a, b| a.label == b.label)
        .map(|g| least_squares(g[0].label.to_owned(), g, 0))
        .collect::<Vec<_>>();

    export(CHARGE_VALIDATE, &obs, &regs).unwrap();
    export(CHARGE_ACCEPT, &obs, &regs).unwrap();
}
