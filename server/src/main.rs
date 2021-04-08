// Copyright 2020 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use rosetta_iota_server::{run_server, Options};

use structopt::StructOpt;

#[tokio::main]
async fn main() {
    let options = Options::from_args();

    let shutdown = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install CTRL+C signal handler");
    };

    run_server(options, shutdown).await;
}
