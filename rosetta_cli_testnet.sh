#!/bin/bash

# kill any zombie process using ports 3030 + 3031
fuser -k 3030/tcp
fuser -k 3031/tcp

# clean up any previous modifications to config files
echo "reset rosetta-cli-conf/testnet..."
git checkout rosetta-cli-conf/testnet

# define a few vars
NODE_URL="https://api.hornet-rosetta.testnet.chrysalis2.com"
NETWORK="testnet6"
DATA_DIR=".rosetta-cli-testnet"
INDEXATION="rosetta"
HRP="atoi"
ROOT=$(pwd)
CONF_DIR=$ROOT/rosetta-cli-conf/testnet

# 1 to enable, comment out to disable
#BOOTSTRAP_GENESIS=1 ...deletes the DATA_DIR and starts synching from block index 1
#BOOTSTRAP_SNAPSHOT=1 ...deletes the DATA_DIR, downloads the latest available snapshot and starts synching from the snapshot state
#NO_BOOTSTRAP=1 ...continues to synch where it ended last time (DATA_DIR must exist and the ledger state must be present)
#INSTALL=1 ...installs rosetta-cli
#DATA=1 ...tests the Data API
#DATA_WITH_RECONCILIATION=1 ...tests the Data API with reconciliation enabled
#CONSTRUCTION=1 ...tests the Construction API

if [ $INSTALL ]; then
  # install rosetta-cli
  echo "installing rosetta-cli via curl..."
  curl -sSfL https://raw.githubusercontent.com/coinbase/rosetta-cli/master/scripts/install.sh | sh -s -- -b .
fi

if [ -z "$BOOTSTRAP_GENESIS" ] && [ -z "$BOOTSTRAP_SNAPSHOT" ] && [ -z "$NO_BOOTSTRAP" ]; then
  echo "no bootstrapping method was specified..."
  exit 1
fi

if [ -z "$DATA" ] && [ -z "$DATA_WITH_RECONCILIATION" ] && [ -z "$CONSTRUCTION" ]; then
  echo "not specified what should be tested..."
  exit 1
fi

# start servers (online and offline)
RUST_BACKTRACE=1 RUST_LOG=iota_rosetta=debug cargo run -p rosetta-iota-server -- --network $NETWORK --iota-endpoint $NODE_URL --bech32-hrp $HRP --indexation $INDEXATION --port 3030 --mode online &
PID_ONLINE=$!

RUST_BACKTRACE=1 RUST_LOG=iota_rosetta=debug cargo run -p rosetta-iota-server -- --network $NETWORK --iota-endpoint $NODE_URL --bech32-hrp $HRP --indexation $INDEXATION --port 3031 --mode offline &
PID_OFFLINE=$!

# wait for the server to completely start
sleep 5

if [ $BOOTSTRAP_GENESIS ]; then
  # remove the data directory
  rm -rf $DATA_DIR
  # all other values are already set in the default config
fi

if [ $BOOTSTRAP_SNAPSHOT ]; then
  # remove the data directory
  rm -rf $DATA_DIR

  # download latest snapshot and create the bootstrap_balances.json
  RUST_BACKTRACE=1 cargo run -p rosetta-iota-utils -- --mode snapshot 2> /dev/null

  # move generated file to $CONF_DIR
  mv bootstrap_balances.json $CONF_DIR

  SEP_INDEX=$(cat sep_index)
  START_MS=`expr $SEP_INDEX + 1`

  cat <<< $(jq --argjson START_MS "$START_MS" '.data.start_index |= $START_MS' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json

  # clean up artifacts
  rm delta_snapshot.bin
  rm full_snapshot.bin
  rm sep_index
fi

# start synching from $DATA_DIR
if [ "$NO_BOOTSTRAP" ]; then
  cat <<< $(jq 'del(.data.start_index)' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json
  cat <<< $(jq 'del(.data.bootstrap_balances)' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json
  if [ -d "$DATA_DIR" ]; then
    echo "can not find data directory, please boostrap rosetta-cli..."
  fi
fi

if [ $CONSTRUCTION ]; then
  echo "--------------------------------------------------------------------------------"
  echo "asking for faucet funds to load up prefunded_accounts..."

  PREFUNDED_ACCOUNT=$(RUST_BACKTRACE=1 cargo run -p rosetta-iota-utils -- --mode faucet 2> /dev/null)

  if [ -z "$PREFUNDED_ACCOUNT" ]; then
    echo "error on getting funds from faucet... exiting"
    exit 1
  fi

  echo "prefunded_account: ${PREFUNDED_ACCOUNT}"

  SK=$(echo $PREFUNDED_ACCOUNT | jq '.sk')
  ADDR=$(echo $PREFUNDED_ACCOUNT | jq '.bech32_addr')

  cat <<< $(jq --argjson ADDR "$ADDR" '.construction.prefunded_accounts[0].account_identifier.address |= $ADDR' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json
  cat <<< $(jq --argjson SK "$SK" '.construction.prefunded_accounts[0].privkey |= $SK' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json

  # render $ADDR again, now without quotes
  ADDR=$(echo $PREFUNDED_ACCOUNT | jq '.bech32_addr' -r)

  OUTPUT_IDS=$(curl -s -X GET "$NODE_URL/api/v1/addresses/$ADDR/outputs" -H  "accept: application/json" | jq '.data.outputIds')
  OUTPUT_ID_A=$(echo $OUTPUT_IDS | jq '.[0]')
  OUTPUT_ID_B=$(echo $OUTPUT_IDS | jq '.[1]')

  echo "output_id_A: ${OUTPUT_ID_A}"
  echo "output_id_B: ${OUTPUT_ID_B}"

  sed -i 's/idA/'$OUTPUT_ID_A'/g' $CONF_DIR/iota.ros
  sed -i 's/idB/'$OUTPUT_ID_B'/g' $CONF_DIR/iota.ros
fi

if [ $DATA_WITH_RECONCILIATION ]; then
  cat <<< $(jq '.data.reconciliation_disabled |= false' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json
  cat <<< $(jq '.data.end_conditions.reconciliation_coverage.coverage |= 0.95' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json
  cat <<< $(jq '.data.end_conditions.reconciliation_coverage.from_tip |= true' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json
  cat <<< $(jq 'del(.data.end_conditions.tip)' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json
fi

cat <<< $(jq --arg NETWORK "$NETWORK" '.network.network |= $NETWORK' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json
cat <<< $(jq --arg DATA_DIR "$DATA_DIR" '.data_directory |= $DATA_DIR' $CONF_DIR/rosetta-iota.json) > $CONF_DIR/rosetta-iota.json

if [ $CONSTRUCTION ]; then
  # test Construction API
  echo "--------------------------------------------------------------------------------"
  echo "running rosetta-cli check:construction"
  ./rosetta-cli check:construction --configuration-file $CONF_DIR/rosetta-iota.json
  CONSTRUCTION_EXIT=$?
fi

if [ $CONSTRUCTION ] && [ $CONSTRUCTION_EXIT -ne 0 ]; then
  echo "rosetta-cli check:construction unsuccessful..."
  exit $CONSTRUCTION_EXIT
fi

if [ $DATA ] || [ $DATA_WITH_RECONCILIATION ]; then
  # test Data API
  echo "--------------------------------------------------------------------------------"
  echo "running rosetta-cli check:data"
  ./rosetta-cli check:data --configuration-file $CONF_DIR/rosetta-iota.json
  DATA_EXIT=$?
fi

if ([ $DATA ] || [ $DATA_WITH_RECONCILIATION ]) && [ $DATA_EXIT -ne 0 ]; then
  echo "rosetta-cli check:data unsuccessful..."
  exit 1
fi

kill $PID_ONLINE
kill $PID_OFFLINE
