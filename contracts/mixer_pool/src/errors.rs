use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MixerError {
    /// Contract has already been initialized
    AlreadyInitialized = 1,
    /// Attached XLM does not match pool denomination
    WrongAmount = 2,
    /// Commitment already exists in the tree
    DuplicateCommitment = 3,
    /// Merkle tree is full (all 2^depth leaves occupied)
    TreeFull = 4,
    /// Root not found in root history window
    InvalidRoot = 5,
    /// Nullifier hash has already been spent (double-spend attempt)
    NullifierSpent = 6,
    /// Groth16 proof verification failed
    InvalidProof = 7,
    /// Relayer fee exceeds contract maximum
    FeeTooHigh = 8,
    /// Invalid denomination (must be positive)
    InvalidDenomination = 9,
    /// Invalid tree depth
    InvalidTreeDepth = 10,
    /// Invalid root history size
    InvalidRootHistorySize = 11,
    /// Caller is not authorized
    Unauthorized = 12,
}
