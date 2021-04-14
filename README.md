<p align="center">
  <a href="https://www.rosetta-api.org">
    <img width="90%" alt="Rosetta" src="https://www.rosetta-api.org/img/rosetta_header.png">
  </a>
</p>
<h3 align="center">
   Rosetta IOTA
</h3>

## Overview

`rosetta-iota` provides a reference implementation of the Rosetta API for IOTA in Rust. If you haven't heard of the Rosetta API, you can find more information [here](https://www.rosetta-api.org/).

## Features

 - Implementation of both Data API and Construction API
 - Stateless, offline, curve-based transaction construction
 - [Storage Pruning](https://www.rosetta-api.org/docs/storage_pruning.html#docsNav) using a block height-based configuration setting
  
## Quick start

### Testnet deployment

1) Ensure `docker` and `docker-compose` are installed.
2) Download the latest release of `rosetta-iota` and extract the files in a folder of your choice.
3) Add your HORNET peers (to which your HORNET node should connect) to following configuration file: `hornet/peering.json`. If you don't have any peers, please contact us and we will help you.
4) **Run following commands to start a HORNET node together with a Rosetta API instance:**

    **Testnet:Online**
    ```
    NETWORK=testnet7 BECH32_HRP=atoi TX_TAG=Rosetta MODE=online docker-compose up
    ```

    **Testnet:Offline**
    ```
    NETWORK=testnet7 BECH32_HRP=atoi TX_TAG=Rosetta MODE=offline docker-compose up
    ```
Once the HORNET node has synced with the network, the Rosetta API will be available at: http://localhost:3030
The REST API of the HORNET node will be available at: http://localhost:14265/api/v1/info

### Testing with rosetta-cli

The provided scripts `check_data_testnet.sh` and `check_construction_testnet.sh` build on top of rosetta-cli. They help with bootstrapping rosetta-cli and make sure that rosetta-cli is runs with the correct configuration values.

1) To be able to run the testing scripts, install following dependencies:
    ```
    sudo apt-get install sed jq psmisc
    ```

2) Ensure the IOTA node is running and an instance of the Rosetta API is available.
Also ensure that the REST API of the IOTA is available at http://localhost:14265/api/v1/info.

    To validate the correctness of `rosetta-iota` run the commands below:

    Testing the Data API **(Testnet)**:
    ```
    ROSETTA_CLI_INSTALL=1 BOOTSTRAP_BALANCES=1 NODE_URL=http://localhost:14265 ./check_data_testnet.sh
    ```

    Testing the Construction API **(Testnet)**:
    ```
    ROSETTA_CLI_INSTALL=1 BOOTSTRAP_BALANCES=1 NODE_URL=http://localhost:14265 ./check_construction_testnet.sh
    ```
 
## Documentation

Please refer to [https://rosetta-api.docs.iota.org](https://rosetta-api.docs.iota.org) for further documentation.