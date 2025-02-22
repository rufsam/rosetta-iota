// Copyright 2020 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{
    config::Config,
    currency::iota_currency,
    error::ApiError,
    is_offline_mode_enabled, is_wrong_network,
    types::{AccountIdentifier, Amount, BlockIdentifier, NetworkIdentifier, PartialBlockIdentifier},
};
use crate::client::{build_client, get_balance_of_address, get_confirmed_milestone_index};

use bee_message::milestone::MilestoneIndex;

use log::debug;
use serde::{Deserialize, Serialize};

use std::time::Duration;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccountBalanceRequest {
    pub network_identifier: NetworkIdentifier,
    pub account_identifier: AccountIdentifier,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_identifier: Option<PartialBlockIdentifier>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccountBalanceResponse {
    pub block_identifier: BlockIdentifier,
    pub balances: Vec<Amount>,
}

pub async fn account_balance(
    request: AccountBalanceRequest,
    options: Config,
) -> Result<AccountBalanceResponse, ApiError> {
    debug!("/account/balance");

    if is_wrong_network(&options, &request.network_identifier) {
        return Err(ApiError::NonRetriable("wrong network".to_string()));
    }

    if is_offline_mode_enabled(&options) {
        return Err(ApiError::NonRetriable(
            "endpoint does not support offline mode".to_string(),
        ));
    }

    // historical balance lookup is not supported
    if request.block_identifier.is_some() {
        return Err(ApiError::NonRetriable(
            "historical balance lookup not supported".to_string(),
        ));
    }

    let (balance, milestone_index) = balance_at_milestone(&request.account_identifier.address, &options).await?;

    Ok(AccountBalanceResponse {
        block_identifier: BlockIdentifier {
            index: *milestone_index,
            hash: (*milestone_index).to_string(),
        },
        balances: vec![balance],
    })
}

async fn balance_at_milestone(address: &str, options: &Config) -> Result<(Amount, MilestoneIndex), ApiError> {
    let client = build_client(options).await?;

    // to make sure the balance of an address does not change in the meantime, check the index of the confirmed
    // milestone before and after fetching the balance
    // TODO: this is only a short-term solution and should be replaced in future
    let (balance_response, index) = {
        loop {
            let index_before = get_confirmed_milestone_index(&client).await?;
            let balance_response = get_balance_of_address(address, &client).await?;
            tokio::time::sleep(Duration::from_millis(250)).await;
            let index_after = get_confirmed_milestone_index(&client).await?;
            if index_before == index_after {
                break (balance_response, index_before)
            }
        }
    };

    let amount = Amount {
        value: balance_response.balance.to_string(),
        currency: iota_currency(),
        metadata: None,
    };

    Ok((amount, MilestoneIndex(index)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::RosettaMode, mocked_node::start_mocked_node};
    use serial_test::serial;
    use tokio::sync::oneshot;

    #[tokio::test]
    #[serial]
    async fn test_balance() {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        tokio::task::spawn(start_mocked_node(shutdown_rx));

        let request = AccountBalanceRequest {
            network_identifier: NetworkIdentifier {
                blockchain: "iota".to_string(),
                network: "testnet7".to_string(),
                sub_network_identifier: None,
            },
            account_identifier: AccountIdentifier {
                address: String::from("atoi1qppx6868hzy497e3yamzxj3dp4ameljlh4x6ac7sdrrtg25fnk2tjlpxcek"),
                sub_account: None,
            },
            block_identifier: None,
        };

        let server_options = Config {
            node_url: "http://127.0.0.1:3029".to_string(),
            network: "testnet7".to_string(),
            tx_tag: "rosetta".to_string(),
            bech32_hrp: "atoi".to_string(),
            mode: RosettaMode::Online,
            bind_addr: "0.0.0.0:3030".to_string(),
        };

        let response = account_balance(request, server_options).await.unwrap();

        assert_eq!(68910, response.block_identifier.index);
        assert_eq!(
            "68910",
            response.block_identifier.hash
        );
        assert_eq!(1, response.balances.len());
        assert_eq!("IOTA", response.balances[0].currency.symbol);
        assert_eq!(0, response.balances[0].currency.decimals);
        assert_eq!("11000000", response.balances[0].value);

        let _ = shutdown_tx.send(());
    }
}
