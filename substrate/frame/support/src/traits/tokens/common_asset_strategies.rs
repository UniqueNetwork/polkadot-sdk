use super::asset_metadata::*;
use super::unique_assets::*;
use core::marker::PhantomData;

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

pub struct NewOwnedChildAssetWithId<'a, AssetKind, ParentAssetId, Id, Owner> {
    pub parent_asset_id: &'a ParentAssetId,
    pub id: &'a Id,
    pub owner: &'a Owner,
    _phantom: PhantomData<AssetKind>,
}
impl<'a, AssetKind, ParentAssetId, Id, Owner> NewOwnedChildAssetWithId<'a, AssetKind, ParentAssetId, Id, Owner> {
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
impl<'a, AssetKind, ParentAssetId, Id, Owner> CreateStrategy for NewOwnedChildAssetWithId<'a, AssetKind, ParentAssetId, Id, Owner> {
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
