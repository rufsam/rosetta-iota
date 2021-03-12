// Copyright 2020 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::types::*;
use crate::{Options, require_offline_mode};
use crate::error::ApiError;

use std::collections::HashMap;
use std::str::FromStr;

use bee_message::prelude::*;

use log::debug;
use crate::construction::{address_from_public_key};

use serde::{Deserialize, Serialize};
use bee_rest_api::types::{OutputDto, AddressDto};
use crate::operations::{utxo_input_operation, utxo_output_operation};
use bee_rest_api::endpoints::api::v1::output::OutputResponse;

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
    construction_parse_request: ConstructionParseRequest,
    options: Options,
) -> Result<ConstructionParseResponse, ApiError> {
    debug!("/construction/parse");

    let _ = require_offline_mode(&options)?;

    if construction_parse_request.signed {
        parse_signed_transaction(construction_parse_request, &options).await
    } else {
        parse_unsigned_transaction(construction_parse_request, &options).await
    }

}

async fn parse_unsigned_transaction(
    construction_parse_request: ConstructionParseRequest,
    options: &Options
) -> Result<ConstructionParseResponse, ApiError> {

    let unsigned_transaction_decoded = hex::decode(construction_parse_request.transaction)?;
    let unsigned_transaction: UnsignedTransaction = serde_json::from_slice(&unsigned_transaction_decoded).unwrap();

    let regular_essence = match unsigned_transaction.essence() {
        Essence::Regular(r) => r,
        _ => return Err(ApiError::BadConstructionRequest("essence type not supported".to_string()))
    };

    let operations = regular_essence_to_operations(&regular_essence, unsigned_transaction.inputs_metadata(), options).await?;

    Ok(ConstructionParseResponse {
        operations,
        account_identifier_signers: None,
    })
}

async fn parse_signed_transaction(
    construction_parse_request: ConstructionParseRequest,
    options: &Options
) -> Result<ConstructionParseResponse, ApiError> {

    let signed_transaction_decoded = hex::decode(construction_parse_request.transaction)?;
    let signed_transaction: SignedTransaction = serde_json::from_slice(&signed_transaction_decoded).unwrap();
    let transaction = signed_transaction.transaction();

    let regular_essence = match signed_transaction.transaction().essence() {
        Essence::Regular(r) => r,
        _ => return Err(ApiError::BadConstructionRequest("essence type not supported".to_string()))
    };

    let operations = regular_essence_to_operations(&regular_essence, signed_transaction.inputs_metadata(), options).await?;

    let account_identifier_signers = {
        let mut accounts_identifiers = Vec::new();
        for unlock_block in transaction.unlock_blocks().into_iter() {
            if let UnlockBlock::Signature(s) = unlock_block {
                let signature = match s {
                    SignatureUnlock::Ed25519(s) => s,
                    _ => return Err(ApiError::BadConstructionRequest("signature type not supported".to_string()))
                };
                let bech32_addr = address_from_public_key(&hex::encode(signature.public_key()))?.to_bech32(&options.bech32_hrp);
                accounts_identifiers.push(AccountIdentifier {
                    address: bech32_addr,
                    sub_account: None
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

async fn regular_essence_to_operations(regular_essence: &RegularEssence, inputs_metadata: &HashMap<String, OutputResponse>, options: &Options) -> Result<Vec<Operation>, ApiError>{

    let mut operations = vec![];
    let mut operation_counter = 0;

    for input in regular_essence.inputs() {

        let utxo_input = match input {
            Input::UTXO(i) => i,
            _ => return Err(ApiError::BadConstructionRequest("input type not supported".to_string()))
        };

        let input_metadata = match inputs_metadata.get(&utxo_input.to_string()) {
            Some(metadata) => metadata,
            None => return Err(ApiError::BadConstructionRequest("metadata for input missing".to_string()))
        };

        let transaction_id = input_metadata.transaction_id.clone();
        let output_index = input_metadata.output_index.clone();
        let is_spent = input_metadata.is_spent.clone();

        let (amount, ed25519_address) = match &input_metadata.output {
            OutputDto::Treasury(_) => panic!("Can't be used as input"),
            OutputDto::SignatureLockedSingle(x) => match x.address.clone() {
                AddressDto::Ed25519(ed25519) => (x.amount, ed25519.address)
            },
            OutputDto::SignatureLockedDustAllowance(_) => panic!("not implemented!"),
        };

        let bech32_address = Ed25519Address::from_str(&ed25519_address).unwrap().to_bech32(&options.bech32_hrp);

        operations.push(utxo_input_operation(transaction_id, bech32_address, amount, output_index, operation_counter, &true, is_spent, false));

        operation_counter = operation_counter + 1;
    }

    let mut output_index = 0;
    for output in regular_essence.outputs() {
        let (amount, ed25519_address) = match output {
            Output::SignatureLockedSingle(x) => match x.address() {
                Address::Ed25519(ed25519) => (x.amount(), ed25519.clone().to_string()),
                _ => panic!("not implemented!")
            },
            _ => panic!("not implemented!")
        };

        let bech32_address = Ed25519Address::from_str(&ed25519_address).unwrap().to_bech32(&options.bech32_hrp);

        operations.push(utxo_output_operation(bech32_address, amount, output_index, operation_counter));
        output_index = output_index + 1;
        operation_counter = operation_counter + 1;
    }

    Ok(operations)

}