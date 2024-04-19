use crate::dispatch::DispatchResult;
use sp_runtime::DispatchError;
use core::marker::PhantomData;

/// Trait for providing types for identifying assets of different kinds.
pub trait AssetDefinition<AssetKind> {
    /// Type for identifying an asset.
    type Id;
}

pub trait MetadataInspectStrategy {
    type Value;
}

pub trait InspectMetadata<AssetKind, Strategy: MetadataInspectStrategy>: AssetDefinition<AssetKind> {
    fn inspect_metadata(id: &Self::Id, strategy: Strategy) -> Result<Strategy::Value, DispatchError>;
}

pub trait MetadataUpdateStrategy {
    type Update<'u>;
}

pub trait UpdateMetadata<AssetKind, Strategy: MetadataUpdateStrategy>: AssetDefinition<AssetKind> {
    fn update_metadata(
        id: &Self::Id,
        strategy: Strategy,
        update: Strategy::Update<'_>,
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
    impl<RuntimeOrigin, Inner: MetadataInspectStrategy> MetadataInspectStrategy for CheckOrigin<RuntimeOrigin, Inner> {
        type Value = Inner::Value;
    }
    impl<RuntimeOrigin, Inner: MetadataUpdateStrategy> MetadataUpdateStrategy for CheckOrigin<RuntimeOrigin, Inner> {
        type Update<'u> = Inner::Update<'u>;
    }
    impl<RuntimeOrigin, Inner: CreateStrategy> CreateStrategy for CheckOrigin<RuntimeOrigin, Inner> {
        type Success = Inner::Success;
    }
    impl<RuntimeOrigin, Inner: TransferStrategy> TransferStrategy for CheckOrigin<RuntimeOrigin, Inner> {}
    impl<RuntimeOrigin, Inner: DestroyStrategy> DestroyStrategy for CheckOrigin<RuntimeOrigin, Inner> {}

    pub struct Bytes<Flavor = ()>(pub Flavor);
    impl Bytes<()> {
        pub fn new() -> Self {
            Self(())
        }
    }
    impl<Flavor> Bytes<Flavor> {
        pub fn from(flavor: Flavor) -> Self {
            Self(flavor)
        }
    }
    impl<Flavor> MetadataInspectStrategy for Bytes<Flavor> {
        type Value = Vec<u8>;
    }
    impl<Flavor> MetadataUpdateStrategy for Bytes<Flavor> {
        type Update<'u> = Option<&'u [u8]>;
    }

    pub struct Ownership<Owner>(PhantomData<Owner>);
    impl<Owner> Ownership<Owner> {
        pub fn new() -> Self {
            Self(PhantomData)
        }
    }
    impl<Owner> MetadataInspectStrategy for Ownership<Owner> {
        type Value = Owner;
    }

    pub struct CanCreate;
    impl MetadataInspectStrategy for CanCreate {
        type Value = bool;
    }
    impl MetadataUpdateStrategy for CanCreate {
        type Update<'u> = bool;
    }

    pub struct CanTransfer;
    impl MetadataInspectStrategy for CanTransfer {
        type Value = bool;
    }
    impl MetadataUpdateStrategy for CanTransfer {
        type Update<'u> = bool;
    }

    pub struct CanDestroy;
    impl MetadataInspectStrategy for CanDestroy {
        type Value = bool;
    }
    impl MetadataUpdateStrategy for CanDestroy {
        type Update<'u> = bool;
    }

    pub struct CanUpdateMetadata;
    impl MetadataInspectStrategy for CanUpdateMetadata {
        type Value = bool;
    }
    impl MetadataUpdateStrategy for CanUpdateMetadata {
        type Update<'u> = bool;
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
