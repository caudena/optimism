use alloy_eips::BlockNumberOrTag;
use alloy_network::TransactionResponse;
use alloy_rpc_types::Block;
use alloy_rpc_types_eth::BlockTransactions;
use alloy_rpc_types_trace::parity::{LocalizedTransactionTrace, TraceResults, TraceResultsWithTransactionHash, TraceType};
use futures::join;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use jsonrpsee_types::ErrorObjectOwned;
use op_alloy_consensus::OpTxEnvelope;
use op_alloy_network::Optimism;
use reth_rpc_eth_api::helpers::{EthBlocks, Trace, FullEthApi};
use revm_inspectors::tracing::TracingInspectorConfig;
use alloy_primitives::{hex, map::HashSet, Address, FixedBytes};
use serde::{Deserialize, Serialize};

/// `EnrichedTransaction` object used in RPC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EnrichedTransaction {
    ///Alloy Optimism transaction
    #[serde(flatten)]
    pub inner: op_alloy_rpc_types::Transaction,

    ///compressed public key
    pub public_key: String,

    ///Alloy Optimism receipts
    pub receipts: op_alloy_rpc_types::OpTransactionReceipt,

    ///Alloy traces
    pub trace: TraceResults,
}

/// `EnrichedBlock` object used in RPC
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct EnrichedBlock {
    ///Alloy block
    #[serde(flatten)]
    pub inner: Block<EnrichedTransaction>,

    ///static block rewards
    pub rewards: Vec<LocalizedTransactionTrace>,
}

#[rpc(server, namespace = "eth")]
pub trait EthBlockReceiptsTraceApi {
    #[method(name = "getBlockReceiptsTrace")]
    async fn block_receipts_trace(&self, number: BlockNumberOrTag) -> RpcResult<Option<EnrichedBlock>>;
}

pub(crate) struct EthBlockReceiptsTraceExt<Eth> {
    eth_api: Eth,
}

impl<Eth> EthBlockReceiptsTraceExt<Eth> {
    pub(crate) fn new(eth_api: Eth) -> Self {
        Self { eth_api }
    }
}

#[async_trait::async_trait]
impl<Eth> EthBlockReceiptsTraceApiServer for EthBlockReceiptsTraceExt<Eth>
where
    Eth: FullEthApi<NetworkTypes = Optimism> + Trace + Clone + 'static,
{
    async fn block_receipts_trace(&self, number: BlockNumberOrTag) -> RpcResult<Option<EnrichedBlock>> {
        let trace_task = tokio::spawn({
            let eth_api = self.eth_api.clone();
            async move {
                let mut trace_types: HashSet<TraceType> = HashSet::default();
                trace_types.insert(TraceType::Trace);

                let block_id = number.into();

                eth_api
                    .trace_block_with(
                        block_id,
                        None,
                        TracingInspectorConfig::from_parity_config(&trace_types),
                        move |tx_info, mut ctx| {
                            let full_trace = ctx
                                .take_inspector()
                                .into_parity_builder()
                                .into_trace_results(&ctx.result, &trace_types);

                            let trace = TraceResultsWithTransactionHash {
                                transaction_hash: tx_info.hash.expect("tx hash is set"),
                                full_trace,
                            };
                            Ok(trace)
                        },
                    )
                    .await
            }
        });

        let receipts_task = tokio::spawn({
            let eth_api = self.eth_api.clone();
            async move { EthBlocks::block_receipts(&eth_api, number.into()).await }
        });

        let (trx_traces_handle, trx_receipts_handle) = join!(trace_task, receipts_task);

        let trx_traces_handle_res = trx_traces_handle
            .map_err(|handle_err| {
                ErrorObjectOwned::owned(
                    1,
                    format!("Error in traces join handle for block number {}: {}", number, handle_err),
                    None::<()>,
                )
            })?
            .map_err(|trace_res_err| {
                ErrorObjectOwned::owned(
                    1,
                    format!("Error getting block traces result {} for block{}", trace_res_err, number),
                    None::<()>,
                )
            })
            .and_then(|traces_option| {
                traces_option.ok_or_else(|| {
                    ErrorObjectOwned::owned(1, format!("Error getting block traces option for block {}", number), None::<()>)
                })
            });

        let trx_receipts_handle_res = trx_receipts_handle
            .map_err(|handle_err| {
                ErrorObjectOwned::owned(
                    1,
                    format!("Error in transaction receipts for block number {}: {}", number, handle_err),
                    None::<()>,
                )
            })?
            .map_err(|receipt_res_err| {
                ErrorObjectOwned::owned(
                    2,
                    format!("Error getting transaction receipts result {} for block {}", receipt_res_err, number),
                    None::<()>,
                )
            })
            .and_then(|receipts_option| {
                receipts_option.ok_or_else(|| {
                    ErrorObjectOwned::owned(2, format!("Error getting transaction receipts option for block{}", number), None::<()>)
                })
            });

        let trx_traces = trx_traces_handle_res?;
        let trx_receipts = trx_receipts_handle_res?;

        let block = EthBlocks::rpc_block(&self.eth_api, number.into(), true).await.map_err(|e| ErrorObjectOwned::owned(1, e.to_string(), None::<()>))?.unwrap();

        if trx_receipts.len() != block.transactions.len() {
            return Err(ErrorObjectOwned::owned(1, "trx_receipts.size() != block.transactions.size()", None::<()>));
        }

        if trx_traces.len() != block.transactions.len() {
            return Err(ErrorObjectOwned::owned(1, "trx_traces.size() != block.transactions.size()", None::<()>));
        }

        let mut enriched_trxs: Vec<EnrichedTransaction> = Vec::new();

        if let BlockTransactions::Full(transactions) = block.transactions {
            for ((trx, receipt), trace) in transactions.into_iter().zip(trx_receipts).zip(trx_traces) {
                let alloy_trx: op_alloy_rpc_types::Transaction = trx;
                let alloy_receipt: op_alloy_rpc_types::OpTransactionReceipt = receipt;

                let tx_hash = alloy_trx.inner.tx_hash();
                let receipt_hash = alloy_receipt.inner.transaction_hash;

                if tx_hash != receipt_hash || tx_hash != trace.transaction_hash {
                    return Err(ErrorObjectOwned::owned(
                        2,
                        format!("Mismatch between transaction hash and corresponding receipt hash {}", tx_hash),
                        None::<()>,
                    ));
                }

                let mut alloy_public_key = String::new();
                let is_bridge = matches!(alloy_trx.inner.inner.inner(), OpTxEnvelope::Deposit(_));

                if !is_bridge {
                    let mut tx_message_hash: Option<FixedBytes<32>> = None;
                    let mut tx_sig: Option<alloy_primitives::Signature> = None;

                    match alloy_trx.inner.inner.inner().clone() {
                        OpTxEnvelope::Legacy(typed_tx) => {
                            tx_message_hash = Some(typed_tx.signature_hash());
                            tx_sig = Some(*typed_tx.signature());
                        }
                        OpTxEnvelope::Eip1559(typed_tx) => {
                            tx_message_hash = Some(typed_tx.signature_hash());
                            tx_sig = Some(*typed_tx.signature());
                        }
                        OpTxEnvelope::Eip2930(typed_tx) => {
                            tx_message_hash = Some(typed_tx.signature_hash());
                            tx_sig = Some(*typed_tx.signature());
                        }
                        OpTxEnvelope::Eip7702(typed_tx) => {
                            tx_message_hash = Some(typed_tx.signature_hash());
                            tx_sig = Some(*typed_tx.signature());
                        }
                        OpTxEnvelope::Deposit(_) => {}
                    }

                    if tx_sig.is_none() || tx_message_hash.is_none() {
                        return Err(ErrorObjectOwned::owned(
                            1,
                            format!("Signature not extracted from transaction {}", tx_hash),
                            None::<()>,
                        ));
                    }

                    let alloy_sig = tx_sig.unwrap().recover_from_prehash(&tx_message_hash.unwrap());
                    match alloy_sig {
                        Ok(signature) => {
                            let ec = signature.to_encoded_point(true);
                            let check_address = Address::from_public_key(&signature);
                            alloy_public_key = format!("0x{}", hex::encode(ec.as_bytes()));

                            if check_address != alloy_trx.inner.from() {
                                return Err(ErrorObjectOwned::owned(
                                    1,
                                    format!("Address doesn't match public key to address for {}", tx_hash),
                                    None::<()>,
                                ));
                            }
                        }
                        Err(p_key_err) => {
                            return Err(ErrorObjectOwned::owned(
                                1,
                                format!("Public key not extracted from message and signature, error: {}, transaction hash: {}", p_key_err, tx_hash),
                                None::<()>,
                            ));
                        }
                    }
                }

                enriched_trxs.push(EnrichedTransaction {
                    inner: alloy_trx,
                    public_key: if alloy_public_key.is_empty() { "0x0".to_string() } else { alloy_public_key },
                    receipts: alloy_receipt,
                    trace: trace.full_trace,
                });
            }
        }

        let e_block: Block<EnrichedTransaction> = Block {
            header: block.header,
            uncles: block.uncles,
            transactions: BlockTransactions::Full(enriched_trxs),
            withdrawals: block.withdrawals,
        };

        let block_rewards = vec![];
        let rich_block = EnrichedBlock { inner: e_block, rewards: block_rewards };

        Ok(Some(rich_block))
    }
}
