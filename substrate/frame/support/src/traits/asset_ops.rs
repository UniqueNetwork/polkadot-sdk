use crate::dispatch::DispatchResult;
use core::marker::PhantomData;
use sp_runtime::DispatchError;

/// Trait for providing types for identifying assets of different kinds.
pub trait AssetDefinition<AssetKind> {
	/// Type for identifying an asset.
	type Id;
}

pub trait SecondaryAsset<PrimaryAssetKind, AssetKind>: AssetDefinition<AssetKind> {
	type PrimaryAsset: AssetDefinition<PrimaryAssetKind>;
}

pub trait MetadataInspectStrategy {
	type Value;
}

pub trait InspectMetadata<AssetKind, Strategy: MetadataInspectStrategy>:
	AssetDefinition<AssetKind>
{
	fn inspect_metadata(
		id: &Self::Id,
		strategy: Strategy,
	) -> Result<Strategy::Value, DispatchError>;
}

pub trait MetadataUpdateStrategy {
	type Update<'u>;
}

pub trait UpdateMetadata<AssetKind, Strategy: MetadataUpdateStrategy>:
	AssetDefinition<AssetKind>
{
	fn update_metadata(
		id: &Self::Id,
		strategy: Strategy,
		update: Strategy::Update<'_>,
	) -> DispatchResult;
}

pub trait CreateStrategy {
	type Success;
}

pub trait Create<AssetKind, Strategy: CreateStrategy> {
	fn create(strategy: Strategy) -> Result<Strategy::Success, DispatchError>;
}

pub trait TransferStrategy {}

pub trait Transfer<AssetKind, Strategy: TransferStrategy>: AssetDefinition<AssetKind> {
	fn transfer(id: &Self::Id, strategy: Strategy) -> DispatchResult;
}

pub trait DestroyStrategy {}

pub trait Destroy<AssetKind, Strategy: DestroyStrategy>: AssetDefinition<AssetKind> {
	fn destroy(id: &Self::Id, strategy: Strategy) -> DispatchResult;
}

pub mod common_asset_kinds {
	pub struct Class;

	pub struct Instance;
}

pub mod common_strategies {
	use super::*;

	pub struct WithOrigin<RuntimeOrigin, Inner>(pub RuntimeOrigin, pub Inner);
	impl<RuntimeOrigin, Inner: MetadataInspectStrategy> MetadataInspectStrategy
		for WithOrigin<RuntimeOrigin, Inner>
	{
		type Value = Inner::Value;
	}
	impl<RuntimeOrigin, Inner: MetadataUpdateStrategy> MetadataUpdateStrategy
		for WithOrigin<RuntimeOrigin, Inner>
	{
		type Update<'u> = Inner::Update<'u>;
	}
	impl<RuntimeOrigin, Inner: CreateStrategy> CreateStrategy for WithOrigin<RuntimeOrigin, Inner> {
		type Success = Inner::Success;
	}
	impl<RuntimeOrigin, Inner: TransferStrategy> TransferStrategy for WithOrigin<RuntimeOrigin, Inner> {}
	impl<RuntimeOrigin, Inner: DestroyStrategy> DestroyStrategy for WithOrigin<RuntimeOrigin, Inner> {}

	pub struct Bytes<Flavor = ()>(pub Flavor);
	impl Bytes<()> {
		pub fn new() -> Self {
			Self(())
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

	pub struct WithAutoId<Id>(PhantomData<Id>);
	impl<Id> WithAutoId<Id> {
		pub fn new() -> Self {
			Self(PhantomData)
		}
	}
	impl<Id> CreateStrategy for WithAutoId<Id> {
		type Success = Id;
	}

	pub struct WithKnownId<'a, Id>(pub &'a Id);
	impl<'a, Id> CreateStrategy for WithKnownId<'a, Id> {
		type Success = ();
	}

	pub struct SecondaryTo<
		'a,
		PrimaryAssetKind,
		AssetKind,
		Secondary: SecondaryAsset<PrimaryAssetKind, AssetKind>,
	>(
		pub &'a <Secondary::PrimaryAsset as AssetDefinition<PrimaryAssetKind>>::Id,
		PhantomData<(PrimaryAssetKind, AssetKind)>,
	);
	impl<
			'a,
			PrimaryAssetKind,
			AssetKind,
			Secondary: SecondaryAsset<PrimaryAssetKind, AssetKind>,
		> SecondaryTo<'a, PrimaryAssetKind, AssetKind, Secondary>
	{
		pub fn from_primary_id(
			primary_id: &'a <Secondary::PrimaryAsset as AssetDefinition<PrimaryAssetKind>>::Id,
		) -> Self {
			Self(primary_id, PhantomData)
		}
	}
	impl<
			'a,
			PrimaryAssetKind,
			AssetKind,
			Secondary: SecondaryAsset<PrimaryAssetKind, AssetKind>,
		> CreateStrategy for SecondaryTo<'a, PrimaryAssetKind, AssetKind, Secondary>
	{
		type Success = Secondary::Id;
	}

	pub struct WithOwner<'a, Owner, Inner: CreateStrategy>(pub &'a Owner, pub Inner);
	impl<'a, Owner, Inner: CreateStrategy> CreateStrategy for WithOwner<'a, Owner, Inner> {
		type Success = Inner::Success;
	}

	pub struct WithAdmin<'a, Admin, Inner: CreateStrategy>(pub &'a Admin, pub Inner);
	impl<'a, Admin, Inner: CreateStrategy> CreateStrategy for WithAdmin<'a, Admin, Inner> {
		type Success = Inner::Success;
	}

	pub struct WithConfig<'a, Config, Inner: CreateStrategy>(pub &'a Config, pub Inner);
	impl<'a, Config, Inner: CreateStrategy> CreateStrategy for WithConfig<'a, Config, Inner> {
		type Success = Inner::Success;
	}

	pub struct WithWitness<'a, Witness, Inner>(pub &'a Witness, pub Inner);
	impl<'a, Witness, Inner: CreateStrategy> CreateStrategy for WithWitness<'a, Witness, Inner> {
		type Success = Inner::Success;
	}
	impl<'a, Witness, Inner: DestroyStrategy> DestroyStrategy for WithWitness<'a, Witness, Inner> {}

	pub struct FromTo<'a, Owner>(pub &'a Owner, pub &'a Owner);
	impl<'a, Owner> TransferStrategy for FromTo<'a, Owner> {}

	pub struct ForceTo<'a, Owner>(pub &'a Owner);
	impl<'a, Owner> TransferStrategy for ForceTo<'a, Owner> {}

	pub struct IfOwnedBy<'a, Owner>(pub &'a Owner);
	impl<'a, Owner> DestroyStrategy for IfOwnedBy<'a, Owner> {}

	pub struct ForceDestroy;
	impl DestroyStrategy for ForceDestroy {}
}
