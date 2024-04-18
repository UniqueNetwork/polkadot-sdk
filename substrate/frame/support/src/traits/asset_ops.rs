use crate::dispatch::DispatchResult;
use sp_runtime::DispatchError;
use core::marker::PhantomData;

/// Trait for providing types for identifying assets of different kinds.
pub trait AssetDefinition<AssetKind> {
    /// Type for identifying an asset.
    type Id;
}

pub trait MetadataStrategy {
    type InnermostStrategy: MetadataStrategy;
}

pub trait MetadataDefinition<AssetKind, Strategy: MetadataStrategy> {
    type Key<'k>;
    type Value;
}

pub trait InspectMetadata<AssetKind, Strategy: MetadataStrategy>: MetadataDefinition<AssetKind, Strategy::InnermostStrategy> {
    fn asset_metadata(strategy: Strategy, key: Self::Key<'_>) -> Result<Self::Value, DispatchError>;
}

pub trait UpdateMetadata<AssetKind, Strategy: MetadataStrategy>: MetadataDefinition<AssetKind, Strategy::InnermostStrategy> {
    fn update_asset_metadata(
        strategy: Strategy,
        key: Self::Key<'_>,
        update: Option<&Self::Value>
    ) -> DispatchResult;
}

pub trait CreateStrategy {
    type Success;
}

pub trait Create<Strategy: CreateStrategy> {
    fn create(strategy: Strategy) -> Result<Strategy::Success, DispatchError>;
}

pub trait TransferStrategy {}

pub trait Transfer<AssetKind, Strategy: TransferStrategy>: AssetDefinition<AssetKind> {
    fn transfer(
        id: &Self::Id,
        strategy: Strategy,
    ) -> DispatchResult;
}

pub trait DestroyStrategy {}

pub trait Destroy<AssetKind, Strategy: DestroyStrategy>: AssetDefinition<AssetKind> {
    fn destroy(
        id: &Self::Id,
        strategy: Strategy,
    ) -> DispatchResult;
}

pub mod common_asset_kinds {
    pub struct Class;

    pub struct Instance;
}

pub mod common_strategies {
    use super::*;

    pub struct CheckOrigin<RuntimeOrigin, Inner>(pub RuntimeOrigin, pub Inner);
    impl<RuntimeOrigin, Inner: MetadataStrategy> MetadataStrategy for CheckOrigin<RuntimeOrigin, Inner> {
        type InnermostStrategy = Inner::InnermostStrategy;
    }
    impl<RuntimeOrigin, Inner: CreateStrategy> CreateStrategy for CheckOrigin<RuntimeOrigin, Inner> {
        type Success = Inner::Success;
    }
    impl<RuntimeOrigin, Inner: TransferStrategy> TransferStrategy for CheckOrigin<RuntimeOrigin, Inner> {}
    impl<RuntimeOrigin, Inner: DestroyStrategy> DestroyStrategy for CheckOrigin<RuntimeOrigin, Inner> {}

    pub struct Primary;
    impl MetadataStrategy for Primary {
        type InnermostStrategy = Self;
    }

    pub struct Ownership;
    impl MetadataStrategy for Ownership {
        type InnermostStrategy = Self;
    }

    pub struct CanCreate;
    impl MetadataStrategy for CanCreate {
        type InnermostStrategy = Self;
    }

    pub struct CanTransfer;
    impl MetadataStrategy for CanTransfer {
        type InnermostStrategy = Self;
    }

    pub struct CanDestroy;
    impl MetadataStrategy for CanDestroy {
        type InnermostStrategy = Self;
    }

    pub struct CanUpdateMetadata;
    impl MetadataStrategy for CanUpdateMetadata {
        type InnermostStrategy = Self;
    }

    pub enum Ability {
        Set,
        Reset,
    }
    impl<'a> From<Ability> for Option<&'a ()> {
        fn from(ability: Ability) -> Self {
            match ability {
                Ability::Set => Some(& ()),
                Ability::Reset => None,
            }
        }
    }

    pub struct NewOwnedAsset<'a, AssetKind, Id, Owner>(pub &'a Owner, PhantomData<(AssetKind, Id)>);
    impl<'a, AssetKind, Id, Owner> NewOwnedAsset<'a, AssetKind, Id, Owner> {
        pub fn from(owner: &'a Owner) -> Self {
            Self(owner, PhantomData)
        }
    }
    impl<'a, AssetKind, Id, Owner> CreateStrategy for NewOwnedAsset<'a, AssetKind, Id, Owner> {
        type Success = Id;
    }

    pub struct NewOwnedAssetWithId<'a, AssetKind, Id, Owner> {
        pub id: &'a Id,
        pub owner: &'a Owner,
        _phantom: PhantomData<AssetKind>,
    }
    impl<'a, AssetKind, Id, Owner> NewOwnedAssetWithId<'a, AssetKind, Id, Owner> {
        pub fn from(id: &'a Id, owner: &'a Owner) -> Self {
            Self {
                id,
                owner,
                _phantom: PhantomData,
            }
        }
    }
    impl<'a, AssetKind, Id, Owner> CreateStrategy for NewOwnedAssetWithId<'a, AssetKind, Id, Owner> {
        type Success = ();
    }

    pub struct NewOwnedChildAsset<'a, AssetKind, ParentAssetId, Id, Owner> {
        pub parent_asset_id: &'a ParentAssetId,
        pub owner: &'a Owner,
        _phantom: PhantomData<(AssetKind, Id)>,
    }
    impl<'a, AssetKind, ParentAssetId, Id, Owner> NewOwnedChildAsset<'a, AssetKind, ParentAssetId, Id, Owner> {
        pub fn from(parent_asset_id: &'a ParentAssetId, owner: &'a Owner) -> Self {
            Self {
                parent_asset_id,
                owner,
                _phantom: PhantomData,
            }
        }
    }
    impl<'a, AssetKind, ParentAssetId, Id, Owner> CreateStrategy for NewOwnedChildAsset<'a, AssetKind, ParentAssetId, Id, Owner> {
        type Success = Id;
    }

    pub struct NewOwnedChildAssetWithId<'a, ChildAssetKind, ParentAssetId, Id, Owner> {
        pub parent_asset_id: &'a ParentAssetId,
        pub id: &'a Id,
        pub owner: &'a Owner,
        _phantom: PhantomData<ChildAssetKind>,
    }
    impl<'a, ChildAssetKind, ParentAssetId, Id, Owner> NewOwnedChildAssetWithId<'a, ChildAssetKind, ParentAssetId, Id, Owner> {
        pub fn from(
            parent_asset_id: &'a ParentAssetId,
            id: &'a Id,
            owner: &'a Owner,
        ) -> Self {
            Self {
                parent_asset_id,
                id,
                owner,
                _phantom: PhantomData,
            }
        }
    }
    impl<'a, ChildAssetKind, ParentAssetId, Id, Owner> CreateStrategy for NewOwnedChildAssetWithId<'a, ChildAssetKind, ParentAssetId, Id, Owner> {
        type Success = ();
    }

    pub struct FromTo<'a, Owner>(pub &'a Owner, pub &'a Owner);
    impl<'a, Owner> TransferStrategy for FromTo<'a, Owner> {}

    pub struct ForceTo<'a, Owner>(pub &'a Owner);
    impl<'a, Owner> TransferStrategy for ForceTo<'a, Owner> {}

    pub struct IfOwnedBy<'a, Owner>(pub &'a Owner);
    impl<'a, Owner> DestroyStrategy for IfOwnedBy<'a, Owner> {}

    pub struct ForceDestroy;
    impl DestroyStrategy for ForceDestroy {}
}
