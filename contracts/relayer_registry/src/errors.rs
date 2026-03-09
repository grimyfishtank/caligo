use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    AlreadyInitialized = 1,
    FeeTooHigh = 2,
    InvalidEndpoint = 3,
    RelayerNotFound = 4,
    Unauthorized = 5,
}
