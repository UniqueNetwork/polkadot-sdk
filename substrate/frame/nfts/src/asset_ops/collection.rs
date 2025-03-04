use core::marker::PhantomData;

use crate::{types::asset_strategies::*, Collection as CollectionStorage, *};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		tokens::asset_ops::{
			common_strategies::*, AssetDefinition, Create, Destroy, InspectMetadata, UpdateMetadata,
		},
		EnsureOrigin,
	},
	BoundedSlice,
};
use frame_system::ensure_signed;
use sp_core::Get;
use sp_runtime::DispatchError;

pub struct Collection<PalletInstance>(PhantomData<PalletInstance>);

impl<T: Config<I>, I: 'static> AssetDefinition for Collection<Pallet<T, I>> {
	type Id = T::CollectionId;
}

impl<T: Config<I>, I: 'static> InspectMetadata<Ownership<T::AccountId>>
	for Collection<Pallet<T, I>>
{
	fn inspect_metadata(
		collection: &Self::Id,
		_ownership: Ownership<T::AccountId>,
	) -> Result<T::AccountId, DispatchError> {
		CollectionStorage::<T, I>::get(collection)
			.map(|a| a.owner)
			.ok_or(Error::<T, I>::UnknownCollection.into())
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Bytes> for Collection<Pallet<T, I>> {
	fn inspect_metadata(collection: &Self::Id, _bytes: Bytes) -> Result<Vec<u8>, DispatchError> {
		CollectionMetadataOf::<T, I>::get(collection)
			.map(|collection_metadata| collection_metadata.data.into())
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Bytes> for Collection<Pallet<T, I>> {
	fn update_metadata(
		collection: &Self::Id,
		_bytes: Bytes,
		update: Option<&[u8]>,
	) -> DispatchResult {
		<Pallet<T, I>>::do_update_collection_metadata(
			None,
			*collection,
			update
				.map(|data| <Pallet<T, I>>::construct_metadata(data.to_vec()))
				.transpose()?,
		)
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<WithOrigin<T::RuntimeOrigin, Bytes>>
	for Collection<Pallet<T, I>>
{
	fn update_metadata(
		collection: &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, Bytes>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let WithOrigin(origin, _bytes) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		<Pallet<T, I>>::do_update_collection_metadata(
			maybe_check_origin,
			*collection,
			update
				.map(|data| <Pallet<T, I>>::construct_metadata(data.to_vec()))
				.transpose()?,
		)
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Bytes<RegularAttribute<'a>>>
	for Collection<Pallet<T, I>>
{
	fn inspect_metadata(
		collection: &Self::Id,
		bytes: Bytes<RegularAttribute>,
	) -> Result<Vec<u8>, DispatchError> {
		let item = None::<T::ItemId>;
		let Bytes(RegularAttribute(attribute)) = bytes;

		Attribute::<T, I>::get((
			collection,
			item,
			AttributeNamespace::CollectionOwner,
			<Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?,
		))
		.map(|a| a.0.into())
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<WithOrigin<T::RuntimeOrigin, Bytes<RegularAttribute<'a>>>>
	for Collection<Pallet<T, I>>
{
	fn update_metadata(
		collection: &Self::Id,
		bytes: WithOrigin<T::RuntimeOrigin, Bytes<RegularAttribute>>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let maybe_item = None::<T::ItemId>;
		let namespace = AttributeNamespace::CollectionOwner;

		let WithOrigin(origin, Bytes(RegularAttribute(attribute))) = bytes;
		let attribute = <Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?;
		let update = update
			.map(|data| <Pallet<T, I>>::construct_attribute_value(data.to_vec()))
			.transpose()?;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		<Pallet<T, I>>::do_update_attribute(
			maybe_check_origin,
			*collection,
			maybe_item,
			namespace,
			attribute,
			update,
		)
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Bytes<SystemAttribute<'a>>>
	for Collection<Pallet<T, I>>
{
	fn inspect_metadata(
		collection: &Self::Id,
		bytes: Bytes<SystemAttribute>,
	) -> Result<Vec<u8>, DispatchError> {
		let item: Option<T::ItemId> = None;
		let namespace = AttributeNamespace::Pallet;

		let Bytes(SystemAttribute(attribute)) = bytes;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, item, namespace, attribute))
			.map(|a| a.0.into())
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static> UpdateMetadata<Bytes<SystemAttribute<'a>>>
	for Collection<Pallet<T, I>>
{
	fn update_metadata(
		collection: &Self::Id,
		bytes: Bytes<SystemAttribute>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let maybe_item = None;
		let namespace = AttributeNamespace::Pallet;

		let Bytes(SystemAttribute(attribute)) = bytes;
		let attribute = <Pallet<T, I>>::construct_attribute_key(attribute.to_vec())?;
		let update = update
			.map(|data| <Pallet<T, I>>::construct_attribute_value(data.to_vec()))
			.transpose()?;

		<Pallet<T, I>>::do_update_attribute(
			None,
			*collection,
			maybe_item,
			namespace,
			attribute,
			update,
		)
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<HasRole<'a, T::AccountId>>
	for Collection<Pallet<T, I>>
{
	fn inspect_metadata(
		collection: &Self::Id,
		has_role: HasRole<T::AccountId>,
	) -> Result<bool, DispatchError> {
		let HasRole { who, role } = has_role;

		Ok(<Pallet<T, I>>::has_role(collection, who, role))
	}
}

impl<T: Config<I>, I: 'static>
	Create<Adminable<T::AccountId, AutoId<T::CollectionId>, CollectionConfigFor<T, I>>>
	for Collection<Pallet<T, I>>
{
	fn create(
		strategy: Adminable<T::AccountId, AutoId<T::CollectionId>, CollectionConfigFor<T, I>>,
	) -> Result<T::CollectionId, DispatchError> {
		let Adminable { owner, admin, config, .. } = strategy;

		let collection = NextCollectionId::<T, I>::get()
			.or(T::CollectionId::initial_value())
			.ok_or(Error::<T, I>::UnknownCollection)?;

		<Pallet<T, I>>::do_create_collection(
			collection,
			owner.clone(),
			admin.clone(),
			config,
			T::CollectionDeposit::get(),
			Event::Created { collection, creator: owner, owner: admin },
		)?;

		<Pallet<T, I>>::set_next_collection_id(collection);

		Ok(collection)
	}
}

impl<T: Config<I>, I: 'static>
	Create<
		WithOrigin<
			T::RuntimeOrigin,
			Adminable<T::AccountId, AutoId<T::CollectionId>, CollectionConfigFor<T, I>>,
		>,
	> for Collection<Pallet<T, I>>
{
	fn create(
		strategy: WithOrigin<
			T::RuntimeOrigin,
			Adminable<T::AccountId, AutoId<T::CollectionId>, CollectionConfigFor<T, I>>,
		>,
	) -> Result<T::CollectionId, DispatchError> {
		let WithOrigin(origin, creation_strategy) = strategy;
		let Adminable { owner, admin, config, .. } = creation_strategy;

		let collection = NextCollectionId::<T, I>::get()
			.or(T::CollectionId::initial_value())
			.ok_or(Error::<T, I>::UnknownCollection)?;

		let maybe_check_signer =
			T::ForceOrigin::try_origin(origin).map(|_| None).or_else(|origin| {
				T::CreateOrigin::ensure_origin(origin, &collection)
					.map(Some)
					.map_err(DispatchError::from)
			})?;

		let creation_deposit;
		if let Some(signer) = maybe_check_signer {
			ensure!(signer == owner, Error::<T, I>::NoPermission);

			// DepositRequired can be disabled by calling the with `ForceOrigin` only
			ensure!(
				!config.has_disabled_setting(CollectionSetting::DepositRequired),
				Error::<T, I>::WrongSetting
			);

			creation_deposit = T::CollectionDeposit::get();
		} else {
			creation_deposit = Zero::zero();
		}

		let collection = NextCollectionId::<T, I>::get()
			.or(T::CollectionId::initial_value())
			.ok_or(Error::<T, I>::UnknownCollection)?;

		<Pallet<T, I>>::do_create_collection(
			collection,
			owner.clone(),
			admin.clone(),
			config,
			creation_deposit,
			Event::Created { collection, creator: owner, owner: admin },
		)?;

		<Pallet<T, I>>::set_next_collection_id(collection);

		Ok(collection)
	}
}

impl<T: Config<I>, I: 'static>
	Create<Owned<T::AccountId, AutoId<T::CollectionId>, CollectionConfigFor<T, I>>>
	for Collection<Pallet<T, I>>
{
	fn create(
		strategy: Owned<T::AccountId, AutoId<T::CollectionId>, CollectionConfigFor<T, I>>,
	) -> Result<T::CollectionId, DispatchError> {
		let Owned { owner, id_assignment, config, .. } = strategy;
		let admin = owner.clone();

		Self::create(Adminable::new_configured(owner, admin, id_assignment, config))
	}
}

impl<T: Config<I>, I: 'static> Destroy<WithWitness<DestroyWitness>> for Collection<Pallet<T, I>> {
	fn destroy(
		collection: &Self::Id,
		strategy: WithWitness<DestroyWitness>,
	) -> Result<DestroyWitness, DispatchError> {
		let WithWitness(witness) = strategy;

		<Pallet<T, I>>::do_destroy_collection(*collection, witness, None)
	}
}

impl<T: Config<I>, I: 'static> Destroy<WithOrigin<T::RuntimeOrigin, WithWitness<DestroyWitness>>>
	for Collection<Pallet<T, I>>
{
	fn destroy(
		collection: &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, WithWitness<DestroyWitness>>,
	) -> Result<DestroyWitness, DispatchError> {
		let WithOrigin(origin, WithWitness(witness)) = strategy;

		let maybe_check_owner = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		<Pallet<T, I>>::do_destroy_collection(*collection, witness, maybe_check_owner)
	}
}
