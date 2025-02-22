#!/bin/bash

CHRYSALIS_MAINNET_CONF_DIR="../rosetta-cli-conf/chrysalis-mainnet"
CHRYSALIS_MAINNET_BECH32_HRP="iota"
TESTNET7_CONF_DIR="../rosetta-cli-conf/testnet7"
TESTNET7_HRP="atoi"
DB="rosetta-cli-db"

# uncomment to enable
# INSTALL_ROSETTA_CLI=1 ...installs rosetta-cli to the current folder
# BOOTSTRAP_BALANCES=1 ...deletes the rosetta-cli database, downloads the latest available IOTA snapshots and bootstraps balances
# NO_BOOTSTRAP=1 ...keeps the rosetta-cli database and continues synching from the available state
# NETWORK = ...the network that should be tested on; can be either chrysalis-mainnet or testnet7

if [ -z "$NETWORK" ]; then
  echo "Please specify the network on which to test. Following networks are supported: chrysalis-mainnet or testnet7."
  exit 1
fi

if [[ "$NETWORK" == "chrysalis-mainnet" ]]; then
  CONF_DIR=$CHRYSALIS_MAINNET_CONF_DIR
  HRP=$CHRYSALIS_MAINNET_BECH32_HRP
elif [[ "$NETWORK" == "testnet7" ]]; then
  CONF_DIR=$TESTNET7_CONF_DIR
  HRP=$TESTNET7_HRP
else
  echo "The provided network is not supported. Please run the test for following networks: chrysalis-mainnet or testnet7."
  exit 1
fi

if [ -z "$BOOTSTRAP_BALANCES" ] && [ -z "$NO_BOOTSTRAP" ]; then
  echo "Please specify how rosetta-cli should be bootsrapped. To delete the rosetta-cli database and to bootstrap rosetta-cli with balances from IOTA snapshots, set BOOTSTRAP_BALANCES=1. To keep the rosetta-cli database and continue synching from the available state, set NO_BOOTSTRAP=1."
  exit 1
fi

if [ "$BOOTSTRAP_BALANCES" ] && [ "$NO_BOOTSTRAP" ]; then
  echo "Multiple bootstrapping methods provided. Please provide only one bootstrapping method."
  exit 1
fi

# bootstrap balances
if [ $BOOTSTRAP_BALANCES ]; then

  # remove the rosetta-cli database
  rm -rf $DB

  if [[ "$NETWORK" == "chrysalis-mainnet" ]]; then
    echo "Copy snapshot(s) from ../data/snapshots/chrysalis-mainnet"
    cp -r ../data/snapshots/chrysalis-mainnet/* .
  elif [[ "$NETWORK" == "testnet7" ]]; then
    echo "Copy snapshot(s) from ../data/snapshots/testnet7"
    cp -r ../data/snapshots/testnet7/* .
  fi

  # create the bootstrap_balances.json file
  echo "Process the provided snapshots to create the bootstrap_balances.json file..."
  RUST_BACKTRACE=1 cargo run -p rosetta-iota-snapshot --release -- --network $NETWORK --bech32-hrp $HRP
  ROSETTA_UTILS_EXIT=$?

  if [ $ROSETTA_UTILS_EXIT -ne 0 ]; then
    exit 1
  fi

  # move generated file to $CONF_DIR
  mv bootstrap_balances.json $CONF_DIR

  SEP_INDEX=$(cat sep_index)
  START_MS=`expr $SEP_INDEX + 1`

  cat <<< $(jq --argjson START_MS "$START_MS" '.data.start_index |= $START_MS' $CONF_DIR/config.json) > $CONF_DIR/config.json

  # clean up artifacts
  rm delta_snapshot.bin
  rm full_snapshot.bin
  rm sep_index
fi

# continue from database state
if [ "$NO_BOOTSTRAP" ]; then
  cat <<< $(jq 'del(.data.start_index)' $CONF_DIR/config.json) > $CONF_DIR/config.json
  cat <<< $(jq 'del(.data.bootstrap_balances)' $CONF_DIR/config.json) > $CONF_DIR/config.json
  if [ -f "$DB" ]; then
    echo "Can not find rosetta-cli database. Please boostrap rosetta-cli."
    exit 1
  fi
fi

# install rosetta-cli
if [ $INSTALL ]; then
  # install rosetta-cli
  echo "installing rosetta-cli via curl..."
  curl -sSfL https://raw.githubusercontent.com/coinbase/rosetta-cli/master/scripts/install.sh | sh -s -- -b .
fi

# test Data API
echo "--------------------------------------------------------------------------------"
echo "running rosetta-cli check:data"
./rosetta-cli check:data --configuration-file $CONF_DIR/config.json
DATA_EXIT=$?

if [ $DATA_EXIT -ne 0 ]; then
  echo "rosetta-cli check:data unsuccessful..."
  exit 1
fi