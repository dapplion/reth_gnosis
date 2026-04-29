use alloy_consensus::TxEip4844;
pub use gnosis_primitives::header::GnosisHeader;

pub type TransactionSigned = alloy_consensus::EthereumTxEnvelope<TxEip4844>;

/// The Block type of this node
pub type GnosisBlock = alloy_consensus::Block<TransactionSigned, GnosisHeader>;

/// The body type of this node
pub type BlockBody = alloy_consensus::BlockBody<TransactionSigned, GnosisHeader>;

/// Convert a vanilla Ethereum block into a [`GnosisBlock`] by re-typing the header.
pub fn to_gnosis_block(
    block: alloy_consensus::Block<TransactionSigned, alloy_consensus::Header>,
) -> GnosisBlock {
    GnosisBlock {
        header: GnosisHeader::from(block.header),
        body: BlockBody {
            transactions: block.body.transactions,
            ommers: block
                .body
                .ommers
                .into_iter()
                .map(GnosisHeader::from)
                .collect(),
            withdrawals: block.body.withdrawals,
        },
    }
}
