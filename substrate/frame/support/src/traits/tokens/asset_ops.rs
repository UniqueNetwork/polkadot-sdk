use crate::dispatch::DispatchResult;
use core::marker::PhantomData;
use sp_runtime::DispatchError;

/// Trait for providing types for identifying assets of different kinds.
pub trait AssetDefinition<AssetKind> {
	/// Type for identifying an asset.
	type Id;
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

pub trait IdAssignment {
	type ReportedId;
}

pub trait Create<AssetKind, Strategy: CreateStrategy> {
	fn create(strategy: Strategy) -> Result<Strategy::Success, DispatchError>;
}

pub trait TransferStrategy {}

pub trait Transfer<AssetKind, Strategy: TransferStrategy>: AssetDefinition<AssetKind> {
	fn transfer(id: &Self::Id, strategy: Strategy) -> DispatchResult;
}

pub trait DestroyStrategy {
	type Success;
}

pub trait Destroy<AssetKind, Strategy: DestroyStrategy>: AssetDefinition<AssetKind> {
	fn destroy(id: &Self::Id, strategy: Strategy) -> Result<Strategy::Success, DispatchError>;
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
	impl<RuntimeOrigin, Inner: DestroyStrategy> DestroyStrategy for WithOrigin<RuntimeOrigin, Inner> {
		type Success = Inner::Success;
	}

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

	pub struct AutoId<Id>(PhantomData<Id>);
	impl<Id> AutoId<Id> {
		pub fn new() -> Self {
			Self(PhantomData)
		}
	}
	impl<Id> IdAssignment for AutoId<Id> {
		type ReportedId = Id;
	}

	pub struct PredefinedId<'a, Id>(pub &'a Id);
	impl<'a, Id> IdAssignment for PredefinedId<'a, Id> {
		type ReportedId = ();
	}

	pub struct DeriveIdFrom<'a, ParentId, ChildId>(pub &'a ParentId, PhantomData<ChildId>);
	impl<'a, ParentId, ChildId> DeriveIdFrom<'a, ParentId, ChildId> {
		pub fn parent_id(primary_id: &'a ParentId) -> Self {
			Self(primary_id, PhantomData)
		}
	}
	impl<'a, ParentId, ChildId> IdAssignment for DeriveIdFrom<'a, ParentId, ChildId> {
		type ReportedId = ChildId;
	}

	pub struct Owned<'a, Assignment: IdAssignment, Owner, Config = (), Witness = ()> {
		pub id_assignment: Assignment,
		pub owner: &'a Owner,
		pub config: &'a Config,
		pub witness: &'a Witness,
	}
	impl<'a, Assignment: IdAssignment, Owner> Owned<'a, Assignment, Owner, (), ()> {
		pub fn new(id_assignment: Assignment, owner: &'a Owner) -> Self {
			Self { id_assignment, owner, config: &(), witness: &() }
		}
	}
	impl<'a, Assignment: IdAssignment, Owner, Config>
		Owned<'a, Assignment, Owner, Config, ()>
	{
		pub fn new_configured(
			id_assignment: Assignment,
			owner: &'a Owner,
			config: &'a Config,
		) -> Self {
			Self { id_assignment, owner, config, witness: &() }
		}
	}
	impl<'a, Assignment: IdAssignment, Owner, Config, Witness> CreateStrategy
		for Owned<'a, Assignment, Owner, Config, Witness>
	{
		type Success = Assignment::ReportedId;
	}

	pub struct Adminable<'a, Assignment: IdAssignment, Account, Config = (), Witness = ()> {
		pub id_assignment: Assignment,
		pub owner: &'a Account,
		pub admin: &'a Account,
		pub config: &'a Config,
		pub witness: &'a Witness,
	}
	impl<'a, Assignment: IdAssignment, Account> Adminable<'a, Assignment, Account, (), ()> {
		pub fn new(id_assignment: Assignment, owner: &'a Account, admin: &'a Account) -> Self {
			Self { id_assignment, owner, admin, config: &(), witness: &() }
		}
	}
	impl<'a, Assignment: IdAssignment, Account, Config>
		Adminable<'a, Assignment, Account, Config, ()>
	{
		pub fn new_configured(
			id_assignment: Assignment,
			owner: &'a Account,
			admin: &'a Account,
			config: &'a Config,
		) -> Self {
			Self { id_assignment, owner, admin, config, witness: &() }
		}
	}
	impl<'a, Assignment: IdAssignment, Account, Config, Witness> CreateStrategy
		for Adminable<'a, Assignment, Account, Config, Witness>
	{
		type Success = Assignment::ReportedId;
	}

	pub struct FromTo<'a, Owner>(pub &'a Owner, pub &'a Owner);
	impl<'a, Owner> TransferStrategy for FromTo<'a, Owner> {}

	pub struct ForceTo<'a, Owner>(pub &'a Owner);
	impl<'a, Owner> TransferStrategy for ForceTo<'a, Owner> {}

	pub struct IfOwnedBy<'a, Owner>(pub &'a Owner);
	impl<'a, Owner> DestroyStrategy for IfOwnedBy<'a, Owner> {
		type Success = ();
	}

	pub struct WithWitness<'a, Witness>(pub &'a Witness);
	impl<'a, Witness> DestroyStrategy for WithWitness<'a, Witness> {
		type Success = Witness;
	}

	pub struct IfOwnedByWithWitness<'a, Owner, Witness> {
		pub owner: &'a Owner,
		pub witness: &'a Witness,
	}
	impl<'a, Owner, Witness> DestroyStrategy for IfOwnedByWithWitness<'a, Owner, Witness> {
		type Success = Witness;
	}

	pub struct ForceDestroy;
	impl DestroyStrategy for ForceDestroy {
		type Success = ();
	}
}
