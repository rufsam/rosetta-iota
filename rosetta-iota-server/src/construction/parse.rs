// Copyright 2020 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::{
    construction::{deserialize_signed_transaction, deserialize_unsigned_transaction},
    error::ApiError,
    is_wrong_network,
    operations::{utxo_input_operation, utxo_output_operation},
    types::*,
    Config,
};

use bee_message::prelude::*;
use bee_rest_api::types::{
    dtos::{AddressDto, OutputDto},
    responses::OutputResponse,
};

use crypto::hashes::{blake2b::Blake2b256, Digest};

use log::debug;
use serde::{Deserialize, Serialize};

use crate::operations::dust_allowance_output_operation;
use std::{collections::HashMap, convert::TryInto, str::FromStr};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConstructionParseRequest {
    pub network_identifier: NetworkIdentifier,
    pub signed: bool,
    pub transaction: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConstructionParseResponse {
    pub operations: Vec<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_identifier_signers: Option<Vec<AccountIdentifier>>,
}

pub(crate) async fn construction_parse_request(
    request: ConstructionParseRequest,
    options: Config,
) -> Result<ConstructionParseResponse, ApiError> {
    debug!("/construction/parse");

    if is_wrong_network(&options, &request.network_identifier) {
        return Err(ApiError::NonRetriable("wrong network".to_string()));
    }

    if request.signed {
        parse_signed_transaction(request, &options).await
    } else {
        parse_unsigned_transaction(request, &options).await
    }
}

async fn parse_unsigned_transaction(
    construction_parse_request: ConstructionParseRequest,
    options: &Config,
) -> Result<ConstructionParseResponse, ApiError> {
    let unsigned_transaction = deserialize_unsigned_transaction(&construction_parse_request.transaction);

    let operations = essence_to_operations(
        unsigned_transaction.essence(),
        unsigned_transaction.inputs_metadata(),
        options,
    )
    .await?;

    Ok(ConstructionParseResponse {
        operations,
        account_identifier_signers: None,
    })
}

async fn parse_signed_transaction(
    construction_parse_request: ConstructionParseRequest,
    options: &Config,
) -> Result<ConstructionParseResponse, ApiError> {
    let signed_transaction = deserialize_signed_transaction(&construction_parse_request.transaction);

    let transaction = signed_transaction.transaction();

    let operations =
        essence_to_operations(transaction.essence(), signed_transaction.inputs_metadata(), options).await?;

    let account_identifier_signers = {
        let mut accounts_identifiers = Vec::new();
        for unlock_block in transaction.unlock_blocks().into_iter() {
            if let UnlockBlock::Signature(s) = unlock_block {
                let signature = match s {
                    SignatureUnlock::Ed25519(s) => s,
                    _ => {
                        return Err(ApiError::NonRetriable("signature type not supported".to_string()));
                    }
                };
                let bech32_addr =
                    address_from_public_key(&hex::encode(signature.public_key()))?.to_bech32(&options.bech32_hrp);
                accounts_identifiers.push(AccountIdentifier {
                    address: bech32_addr,
                    sub_account: None,
                });
            }
        }
        accounts_identifiers
    };

    Ok(ConstructionParseResponse {
        operations,
        account_identifier_signers: Some(account_identifier_signers),
    })
}

async fn essence_to_operations(
    essence: &Essence,
    inputs_metadata: &HashMap<String, OutputResponse>,
    options: &Config,
) -> Result<Vec<Operation>, ApiError> {
    let regular_essence = match essence {
        Essence::Regular(r) => r,
        _ => {
            return Err(ApiError::NonRetriable("essence type not supported".to_string()));
        }
    };

    let mut operations = Vec::new();

    for input in regular_essence.inputs() {
        let utxo_input = match input {
            Input::Utxo(i) => i,
            _ => return Err(ApiError::NonRetriable("input type not supported".to_string())),
        };

        let input_metadata = match inputs_metadata.get(&utxo_input.to_string()) {
            Some(metadata) => metadata,
            None => {
                return Err(ApiError::NonRetriable("metadata for input missing".to_string()));
            }
        };

        let transaction_id = input_metadata.transaction_id.clone();
        let output_index = input_metadata.output_index.clone();

        let (amount, ed25519_address) = match &input_metadata.output {
            OutputDto::Treasury(_) => return Err(ApiError::NonRetriable("Can't be used as input".to_string())),
            OutputDto::SignatureLockedSingle(x) => match x.address.clone() {
                AddressDto::Ed25519(ed25519) => (x.amount, ed25519.address),
            },
            OutputDto::SignatureLockedDustAllowance(x) => match x.address.clone() {
                AddressDto::Ed25519(ed25519) => (x.amount, ed25519.address),
            },
        };

        let bech32_address =
            Address::Ed25519(Ed25519Address::from_str(&ed25519_address).unwrap()).to_bech32(&options.bech32_hrp);

        operations.push(utxo_input_operation(
            transaction_id,
            bech32_address,
            amount,
            output_index,
            operations.len(),
            true,
            false,
        ));
    }

    for output in regular_essence.outputs() {
        let output_operation = match output {
            Output::SignatureLockedSingle(o) => match o.address() {
                Address::Ed25519(addr) => {
                    let bech32_address = Address::Ed25519(addr.clone().into()).to_bech32(&options.bech32_hrp);
                    utxo_output_operation(bech32_address, o.amount(), operations.len(), false, None)
                }
                _ => unimplemented!(),
            },
            Output::SignatureLockedDustAllowance(o) => match o.address() {
                Address::Ed25519(addr) => {
                    let bech32_address = Address::Ed25519(addr.clone().into()).to_bech32(&options.bech32_hrp);
                    dust_allowance_output_operation(bech32_address, o.amount(), operations.len(), false, None)
                }
                _ => unimplemented!(),
            },
            _ => unimplemented!(),
        };
        operations.push(output_operation);
    }

    Ok(operations)
}

fn address_from_public_key(hex_string: &str) -> Result<Address, ApiError> {
    let public_key_bytes = hex::decode(hex_string)
        .map_err(|e| ApiError::NonRetriable(format!("can not derive address from public key: {}", e)))?;
    let hash = Blake2b256::digest(&public_key_bytes);
    let ed25519_address = Ed25519Address::new(hash.try_into().unwrap());
    let address = Address::Ed25519(ed25519_address);

    Ok(address)
}
