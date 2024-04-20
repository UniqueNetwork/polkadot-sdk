use crate::{types::asset_strategies::*, *};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		asset_ops::{
			common_asset_kinds::Class, common_strategies::*, AssetDefinition, Create, Destroy,
			InspectMetadata, UpdateMetadata,
		},
		EnsureOrigin,
	},
	BoundedSlice,
};
use frame_system::ensure_signed;
use sp_core::Get;
use sp_runtime::DispatchError;

impl<T: Config<I>, I: 'static> AssetDefinition<Class> for Pallet<T, I> {
	type Id = T::CollectionId;
}

impl<T: Config<I>, I: 'static> InspectMetadata<Class, Ownership<T::AccountId>> for Pallet<T, I> {
	fn inspect_metadata(
		collection: &Self::Id,
		_ownership: Ownership<T::AccountId>,
	) -> Result<T::AccountId, DispatchError> {
		Collection::<T, I>::get(collection)
			.map(|a| a.owner)
			.ok_or(Error::<T, I>::UnknownCollection.into())
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Class, Bytes> for Pallet<T, I> {
	fn inspect_metadata(collection: &Self::Id, _bytes: Bytes) -> Result<Vec<u8>, DispatchError> {
		CollectionMetadataOf::<T, I>::get(collection)
			.map(|collection_metadata| collection_metadata.data.into())
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Class, Bytes> for Pallet<T, I> {
	fn update_metadata(
		collection: &Self::Id,
		_bytes: Bytes,
		update: Option<&[u8]>,
	) -> DispatchResult {
		Self::do_update_collection_metadata(
			None,
			*collection,
			update.map(|data| Self::construct_metadata(data.to_vec())).transpose()?,
		)
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Class, WithOrigin<T::RuntimeOrigin, Bytes>>
	for Pallet<T, I>
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

		Self::do_update_collection_metadata(
			maybe_check_origin,
			*collection,
			update.map(|data| Self::construct_metadata(data.to_vec())).transpose()?,
		)
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Class, Bytes<RegularAttribute<'a>>>
	for Pallet<T, I>
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
			Self::construct_attribute_key(attribute.to_vec())?,
		))
		.map(|a| a.0.into())
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<'a, T: Config<I>, I: 'static>
	UpdateMetadata<Class, WithOrigin<T::RuntimeOrigin, Bytes<RegularAttribute<'a>>>> for Pallet<T, I>
{
	fn update_metadata(
		collection: &Self::Id,
		bytes: WithOrigin<T::RuntimeOrigin, Bytes<RegularAttribute>>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let maybe_item = None::<T::ItemId>;
		let namespace = AttributeNamespace::CollectionOwner;

		let WithOrigin(origin, Bytes(RegularAttribute(attribute))) = bytes;
		let attribute = Self::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| Self::construct_attribute_value(data.to_vec())).transpose()?;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		Self::do_update_attribute(
			maybe_check_origin,
			*collection,
			maybe_item,
			namespace,
			attribute,
			update,
		)
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Class, Bytes<SystemAttribute<'a>>>
	for Pallet<T, I>
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

impl<'a, T: Config<I>, I: 'static> UpdateMetadata<Class, Bytes<SystemAttribute<'a>>>
	for Pallet<T, I>
{
	fn update_metadata(
		collection: &Self::Id,
		bytes: Bytes<SystemAttribute>,
		update: Option<&[u8]>,
	) -> DispatchResult {
		let maybe_item = None;
		let namespace = AttributeNamespace::Pallet;

		let Bytes(SystemAttribute(attribute)) = bytes;
		let attribute = Self::construct_attribute_key(attribute.to_vec())?;
		let update =
			update.map(|data| Self::construct_attribute_value(data.to_vec())).transpose()?;

		Self::do_update_attribute(None, *collection, maybe_item, namespace, attribute, update)
	}
}

impl<'a, T: Config<I>, I: 'static> InspectMetadata<Class, HasRole<'a, T::AccountId>>
	for Pallet<T, I>
{
	fn inspect_metadata(
		collection: &Self::Id,
		has_role: HasRole<T::AccountId>,
	) -> Result<bool, DispatchError> {
		let HasRole { who, role } = has_role;

		Ok(Self::has_role(collection, who, role))
	}
}

impl<'a, T: Config<I>, I: 'static>
	Create<Class, ClassCreation<'a, T::AccountId, CollectionConfigFor<T, I>, T::CollectionId>>
	for Pallet<T, I>
{
	fn create(
		strategy: ClassCreation<'a, T::AccountId, CollectionConfigFor<T, I>, T::CollectionId>,
	) -> Result<T::CollectionId, DispatchError> {
		let WithOwner(owner, WithAdmin(admin, WithConfig(config, _with_auto_id))) = strategy;

		let collection = NextCollectionId::<T, I>::get()
			.or(T::CollectionId::initial_value())
			.ok_or(Error::<T, I>::UnknownCollection)?;

		Self::do_create_collection(
			collection,
			owner.clone(),
			admin.clone(),
			*config,
			T::CollectionDeposit::get(),
			Event::Created { collection, creator: owner.clone(), owner: admin.clone() },
		)?;

		Self::set_next_collection_id(collection);

		Ok(collection)
	}
}

impl<'a, T: Config<I>, I: 'static>
	Create<
		Class,
		WithOrigin<
			T::RuntimeOrigin,
			ClassCreation<'a, T::AccountId, CollectionConfigFor<T, I>, T::CollectionId>,
		>,
	> for Pallet<T, I>
{
	fn create(
		strategy: WithOrigin<
			T::RuntimeOrigin,
			ClassCreation<'a, T::AccountId, CollectionConfigFor<T, I>, T::CollectionId>,
		>,
	) -> Result<T::CollectionId, DispatchError> {
		let WithOrigin(origin, creation @ WithOwner(owner, WithAdmin(_, WithConfig(config, _)))) =
			strategy;

		let collection = NextCollectionId::<T, I>::get()
			.or(T::CollectionId::initial_value())
			.ok_or(Error::<T, I>::UnknownCollection)?;

		let maybe_check_signer =
			T::ForceOrigin::try_origin(origin).map(|_| None).or_else(|origin| {
				T::CreateOrigin::ensure_origin(origin, &collection)
					.map(Some)
					.map_err(DispatchError::from)
			})?;

		if let Some(signer) = maybe_check_signer {
			ensure!(signer == *owner, Error::<T, I>::NoPermission);

			// DepositRequired can be disabled by calling the with `ForceOrigin` only
			ensure!(
				!config.has_disabled_setting(CollectionSetting::DepositRequired),
				Error::<T, I>::WrongSetting
			);
		}

		<Self as Create<_, _>>::create(creation)
	}
}

impl<'a, T: Config<I>, I: 'static> Destroy<Class, WithWitness<'a, DestroyWitness, ForceDestroy>>
	for Pallet<T, I>
{
	fn destroy(
		collection: &Self::Id,
		strategy: WithWitness<'a, DestroyWitness, ForceDestroy>,
	) -> DispatchResult {
		let WithWitness(witness, _force_destroy) = strategy;

		Self::do_destroy_collection(*collection, *witness, None).map(|_| ())
	}
}

impl<'a, T: Config<I>, I: 'static>
	Destroy<Class, WithOrigin<T::RuntimeOrigin, WithWitness<'a, DestroyWitness, ForceDestroy>>>
	for Pallet<T, I>
{
	fn destroy(
		collection: &Self::Id,
		strategy: WithOrigin<T::RuntimeOrigin, WithWitness<'a, DestroyWitness, ForceDestroy>>,
	) -> DispatchResult {
		let WithOrigin(origin, WithWitness(witness, _force_destroy)) = strategy;

		let maybe_check_owner = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		Self::do_destroy_collection(*collection, *witness, maybe_check_owner).map(|_| ())
	}
}
