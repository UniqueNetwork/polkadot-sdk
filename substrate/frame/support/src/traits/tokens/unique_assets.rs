use crate::dispatch::DispatchResult;
use sp_runtime::DispatchError;

/// Trait for providing types for identifying unique assets of different kinds.
pub trait Identification<AssetKind> {
    /// Type for identifying a unique asset.
    type Id;
}

pub trait Ownership<AssetKind, Owner>: Identification<AssetKind> {
    fn owner(id: &Self::Id) -> Result<Owner, DispatchError>;
}

pub trait CreateStrategy {
    type Success;
}

pub trait Create<Strategy: CreateStrategy> {
    fn create(strategy: Strategy) -> Result<Strategy::Success, DispatchError>;
}

pub trait TransferStrategy {}

pub trait Transfer<AssetKind, Strategy: TransferStrategy>: Identification<AssetKind> {
    fn transfer(
        id: &Self::Id,
        strategy: Strategy,
    ) -> DispatchResult;
}

pub trait DestroyStrategy {}

pub trait Destroy<AssetKind, Strategy: DestroyStrategy>: Identification<AssetKind> {
    fn destroy(
        id: &Self::Id,
        strategy: Strategy,
    ) -> DispatchResult;
}

pub mod common_asset_kinds {
    pub struct Class;

    pub struct Instance;
}
