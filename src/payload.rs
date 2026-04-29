//! Payload types for Gnosis.
//!
//! Backed by upstream's now-generic [`EthereumPayloadBuilder`] /
//! [`default_ethereum_payload`] (paradigmxyz/reth#23827). What used to be a
//! 643-line copy of upstream's payload-building loop is replaced by a thin
//! newtype around `EthBuiltPayload<GnosisNodePrimitives>`.
//!
//! The newtype is retained (rather than a type alias) so the engine-API
//! envelope `From`/`TryFrom` impls below stay local — the orphan rule rejects
//! impls on `EthBuiltPayload<GnosisNodePrimitives>` because both that type and
//! the envelope types are foreign.

use std::sync::Arc;

use alloy_eips::eip7685::Requests;
use alloy_primitives::U256;
use reth::rpc::types::engine::{
    BlobsBundleV1, BlobsBundleV2, ExecutionPayloadEnvelopeV5, ExecutionPayloadFieldV2,
    ExecutionPayloadV3,
};
use reth_basic_payload_builder::{
    BuildArguments, BuildOutcome, HeaderForPayload, MissingPayloadBehaviour, PayloadBuilder,
    PayloadConfig,
};
use reth_chainspec::{ChainSpecProvider, EthereumHardforks};
use reth_ethereum_engine_primitives::{
    BuiltPayloadConversionError, ExecutionPayloadEnvelopeV2, ExecutionPayloadEnvelopeV3,
    ExecutionPayloadEnvelopeV4, ExecutionPayloadEnvelopeV6, ExecutionPayloadV1,
};
use reth_ethereum_payload_builder::{EthereumBuilderConfig, EthereumPayloadBuilder};
use reth_ethereum_primitives::TransactionSigned;
use reth_evm::{ConfigureEvm, NextBlockEnvAttributes};
use reth_node_builder::{BuiltPayload, PayloadBuilderError};
use reth_payload_builder::{BlobSidecars, EthBuiltPayload, EthPayloadBuilderAttributes, PayloadId};
use reth_primitives_traits::SealedBlock;
use reth_storage_api::StateProviderFactory;
use reth_transaction_pool::{PoolTransaction, TransactionPool};

use crate::primitives::{block::GnosisBlock, GnosisNodePrimitives};

/// Gnosis payload builder. Newtype around the upstream generic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GnosisPayloadBuilder<Pool, Client, EvmConfig>(
    pub EthereumPayloadBuilder<Pool, Client, EvmConfig>,
);

impl<Pool, Client, EvmConfig> GnosisPayloadBuilder<Pool, Client, EvmConfig> {
    pub const fn new(
        client: Client,
        pool: Pool,
        evm_config: EvmConfig,
        builder_config: EthereumBuilderConfig,
    ) -> Self {
        Self(EthereumPayloadBuilder::new(
            client,
            pool,
            evm_config,
            builder_config,
        ))
    }
}

impl<Pool, Client, EvmConfig> PayloadBuilder for GnosisPayloadBuilder<Pool, Client, EvmConfig>
where
    EvmConfig:
        ConfigureEvm<Primitives = GnosisNodePrimitives, NextBlockEnvCtx = NextBlockEnvAttributes>,
    Client: StateProviderFactory + ChainSpecProvider<ChainSpec: EthereumHardforks> + Clone,
    Pool: TransactionPool<Transaction: PoolTransaction<Consensus = TransactionSigned>>,
{
    type Attributes = EthPayloadBuilderAttributes;
    type BuiltPayload = GnosisBuiltPayload;

    fn try_build(
        &self,
        args: BuildArguments<EthPayloadBuilderAttributes, GnosisBuiltPayload>,
    ) -> Result<BuildOutcome<GnosisBuiltPayload>, PayloadBuilderError> {
        let upstream_args = BuildArguments {
            cached_reads: args.cached_reads,
            config: args.config,
            cancel: args.cancel,
            best_payload: args.best_payload.map(|p| p.0),
        };
        let outcome = self.0.try_build(upstream_args)?;
        Ok(map_outcome(outcome))
    }

    fn on_missing_payload(
        &self,
        _args: BuildArguments<Self::Attributes, Self::BuiltPayload>,
    ) -> MissingPayloadBehaviour<Self::BuiltPayload> {
        MissingPayloadBehaviour::RaceEmptyPayload
    }

    fn build_empty_payload(
        &self,
        config: PayloadConfig<Self::Attributes, HeaderForPayload<Self::BuiltPayload>>,
    ) -> Result<Self::BuiltPayload, PayloadBuilderError> {
        let outcome = self.0.build_empty_payload(config)?;
        Ok(GnosisBuiltPayload(outcome))
    }
}

fn map_outcome(
    outcome: BuildOutcome<EthBuiltPayload<GnosisNodePrimitives>>,
) -> BuildOutcome<GnosisBuiltPayload> {
    match outcome {
        BuildOutcome::Better {
            payload,
            cached_reads,
        } => BuildOutcome::Better {
            payload: GnosisBuiltPayload(payload),
            cached_reads,
        },
        BuildOutcome::Aborted { fees, cached_reads } => {
            BuildOutcome::Aborted { fees, cached_reads }
        }
        BuildOutcome::Cancelled => BuildOutcome::Cancelled,
        BuildOutcome::Freeze(payload) => BuildOutcome::Freeze(GnosisBuiltPayload(payload)),
    }
}

/// Built Gnosis payload. Newtype around the upstream generic so the engine-API
/// envelope conversion impls below are orphan-rule-safe.
#[derive(Debug, Clone)]
pub struct GnosisBuiltPayload(pub EthBuiltPayload<GnosisNodePrimitives>);

impl Default for GnosisBuiltPayload {
    fn default() -> Self {
        Self(EthBuiltPayload::new(
            PayloadId::default(),
            Arc::new(SealedBlock::default()),
            U256::ZERO,
            None,
        ))
    }
}

impl GnosisBuiltPayload {
    pub fn new(
        id: PayloadId,
        block: Arc<SealedBlock<GnosisBlock>>,
        fees: U256,
        requests: Option<Requests>,
    ) -> Self {
        Self(EthBuiltPayload::new(id, block, fees, requests))
    }

    pub fn id(&self) -> PayloadId {
        self.0.id()
    }

    pub fn block(&self) -> &SealedBlock<GnosisBlock> {
        self.0.block()
    }

    pub fn sidecars(&self) -> &BlobSidecars {
        self.0.sidecars()
    }

    pub fn with_sidecars(self, sidecars: impl Into<BlobSidecars>) -> Self {
        Self(self.0.with_sidecars(sidecars))
    }
}

impl BuiltPayload for GnosisBuiltPayload {
    type Primitives = GnosisNodePrimitives;

    fn block(&self) -> &SealedBlock<GnosisBlock> {
        self.0.block()
    }

    fn fees(&self) -> U256 {
        self.0.fees()
    }

    fn requests(&self) -> Option<Requests> {
        self.0.requests()
    }
}

// === Engine-API envelope conversions ===

impl From<GnosisBuiltPayload> for ExecutionPayloadV1 {
    fn from(value: GnosisBuiltPayload) -> Self {
        let hash = value.block().hash();
        Self::from_block_unchecked(hash, &value.block().clone().into_block())
    }
}

impl From<GnosisBuiltPayload> for ExecutionPayloadEnvelopeV2 {
    fn from(value: GnosisBuiltPayload) -> Self {
        let hash = value.block().hash();
        Self {
            block_value: value.0.fees(),
            execution_payload: ExecutionPayloadFieldV2::from_block_unchecked(
                hash,
                &value.block().clone().into_block(),
            ),
        }
    }
}

impl TryFrom<GnosisBuiltPayload> for ExecutionPayloadEnvelopeV3 {
    type Error = BuiltPayloadConversionError;

    fn try_from(value: GnosisBuiltPayload) -> Result<Self, Self::Error> {
        let hash = value.block().hash();
        let blobs_bundle = match value.sidecars() {
            BlobSidecars::Empty => BlobsBundleV1::empty(),
            BlobSidecars::Eip4844(sidecars) => BlobsBundleV1::from(sidecars.clone()),
            BlobSidecars::Eip7594(_) => {
                return Err(BuiltPayloadConversionError::UnexpectedEip7594Sidecars)
            }
        };
        Ok(Self {
            execution_payload: ExecutionPayloadV3::from_block_unchecked(
                hash,
                &value.block().clone().into_block(),
            ),
            block_value: value.0.fees(),
            should_override_builder: false,
            blobs_bundle,
        })
    }
}

impl TryFrom<GnosisBuiltPayload> for ExecutionPayloadEnvelopeV4 {
    type Error = BuiltPayloadConversionError;

    fn try_from(value: GnosisBuiltPayload) -> Result<Self, Self::Error> {
        let execution_requests = value.0.requests().clone().unwrap_or_default();
        Ok(Self {
            execution_requests,
            envelope_inner: value.try_into()?,
        })
    }
}

impl TryFrom<GnosisBuiltPayload> for ExecutionPayloadEnvelopeV5 {
    type Error = BuiltPayloadConversionError;

    fn try_from(value: GnosisBuiltPayload) -> Result<Self, Self::Error> {
        let execution_requests = value.0.requests().clone().unwrap_or_default();
        let hash = value.block().hash();
        let blobs_bundle = match value.sidecars() {
            BlobSidecars::Empty => BlobsBundleV2::empty(),
            BlobSidecars::Eip7594(sidecars) => BlobsBundleV2::from(sidecars.clone()),
            BlobSidecars::Eip4844(_) => {
                return Err(BuiltPayloadConversionError::UnexpectedEip4844Sidecars)
            }
        };
        Ok(Self {
            execution_payload: ExecutionPayloadV3::from_block_unchecked(
                hash,
                &value.block().clone().into_block(),
            ),
            block_value: value.0.fees(),
            should_override_builder: false,
            blobs_bundle,
            execution_requests,
        })
    }
}

impl TryFrom<GnosisBuiltPayload> for ExecutionPayloadEnvelopeV6 {
    type Error = BuiltPayloadConversionError;

    fn try_from(_value: GnosisBuiltPayload) -> Result<Self, Self::Error> {
        unimplemented!("ExecutionPayloadEnvelopeV6 not yet supported")
    }
}
