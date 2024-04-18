use crate::{types::asset_strategies::*, *};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{
		asset_ops::{
			common_asset_kinds::Class,
			common_strategies::{CheckOrigin, Ownership, Primary},
			AssetDefinition, Create, InspectMetadata, MetadataDefinition, UpdateMetadata,
		},
		EnsureOrigin,
	},
	BoundedSlice,
};
use frame_system::ensure_signed;
use sp_core::Get;
use sp_runtime::{BoundedVec, DispatchError};

impl<T: Config<I>, I: 'static> AssetDefinition<Class> for Pallet<T, I> {
	type Id = T::CollectionId;
}

impl<T: Config<I>, I: 'static> MetadataDefinition<Class, Ownership> for Pallet<T, I> {
	type Key<'k> = &'k T::CollectionId;
	type Value = T::AccountId;
}

impl<T: Config<I>, I: 'static> MetadataDefinition<Class, Primary> for Pallet<T, I> {
	type Key<'k> = &'k T::CollectionId;
	type Value = BoundedVec<u8, T::StringLimit>;
}

impl<T: Config<I>, I: 'static> MetadataDefinition<Class, RegularAttribute> for Pallet<T, I> {
	type Key<'k> = (&'k T::CollectionId, &'k [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<T: Config<I>, I: 'static> MetadataDefinition<Class, SystemAttribute> for Pallet<T, I> {
	type Key<'k> = (&'k T::CollectionId, &'k [u8]);
	type Value = BoundedVec<u8, T::ValueLimit>;
}

impl<T: Config<I>, I: 'static> MetadataDefinition<Class, HasRole> for Pallet<T, I> {
	type Key<'k> = (&'k T::CollectionId, &'k T::AccountId);
	type Value = bool;
}

impl<'a, T: Config<I>, I: 'static> Create<ConfiguredCollection<'a, T, I>> for Pallet<T, I> {
	fn create(strategy: ConfiguredCollection<'a, T, I>) -> Result<T::CollectionId, DispatchError> {
		let ConfiguredCollection { owner, admin, config } = strategy;

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
	Create<CheckOrigin<T::RuntimeOrigin, ConfiguredCollection<'a, T, I>>> for Pallet<T, I>
{
	fn create(
		strategy: CheckOrigin<T::RuntimeOrigin, ConfiguredCollection<'a, T, I>>,
	) -> Result<T::CollectionId, DispatchError> {
		let CheckOrigin(origin, ConfiguredCollection { owner, admin, config }) = strategy;

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

impl<T: Config<I>, I: 'static> InspectMetadata<Class, Ownership> for Pallet<T, I> {
	fn asset_metadata(
		_ownership: Ownership,
		collection: Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		Collection::<T, I>::get(collection)
			.map(|a| a.owner)
			.ok_or(Error::<T, I>::UnknownCollection.into())
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Class, Primary> for Pallet<T, I> {
	fn asset_metadata(
		_primary: Primary,
		collection: Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		CollectionMetadataOf::<T, I>::get(collection)
			.map(|collection_metadata| collection_metadata.data)
			.ok_or(Error::<T, I>::MetadataNotFound.into())
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Class, Primary> for Pallet<T, I> {
	fn update_asset_metadata(
		_primary: Primary,
		collection: Self::Key<'_>,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let maybe_check_origin = None;

		match update {
			Some(data) =>
				Self::do_set_collection_metadata(maybe_check_origin, *collection, data.clone()),
			None => Self::do_clear_collection_metadata(maybe_check_origin, *collection),
		}
	}
}

impl<T: Config<I>, I: 'static> UpdateMetadata<Class, CheckOrigin<T::RuntimeOrigin, Primary>>
	for Pallet<T, I>
{
	fn update_asset_metadata(
		strategy: CheckOrigin<T::RuntimeOrigin, Primary>,
		collection: Self::Key<'_>,
		update: Option<&Self::Value>,
	) -> DispatchResult {
		let CheckOrigin(origin, _primary) = strategy;

		let maybe_check_origin = T::ForceOrigin::try_origin(origin)
			.map(|_| None)
			.or_else(|origin| ensure_signed(origin).map(Some).map_err(DispatchError::from))?;

		match update {
			Some(data) =>
				Self::do_set_collection_metadata(maybe_check_origin, *collection, data.clone()),
			None => Self::do_clear_collection_metadata(maybe_check_origin, *collection),
		}
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Class, RegularAttribute> for Pallet<T, I> {
	fn asset_metadata(
		_regular_attribute: RegularAttribute,
		(collection, attribute): Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		let attribute =
			BoundedSlice::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((
			collection,
			Option::<T::ItemId>::None,
			AttributeNamespace::CollectionOwner,
			attribute,
		))
		.map(|a| a.0)
		.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Class, SystemAttribute> for Pallet<T, I> {
	fn asset_metadata(
		_system_attribute: SystemAttribute,
		(collection, attribute): Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		let item: Option<T::ItemId> = None;
		let namespace = AttributeNamespace::Pallet;
		let attribute =
			BoundedSlice::<_, _>::try_from(attribute).map_err(|_| Error::<T, I>::IncorrectData)?;

		Attribute::<T, I>::get((collection, item, namespace, attribute))
			.map(|a| a.0)
			.ok_or(Error::<T, I>::AttributeNotFound.into())
	}
}

impl<T: Config<I>, I: 'static> InspectMetadata<Class, HasRole> for Pallet<T, I> {
	fn asset_metadata(
		HasRole(role): HasRole,
		(collection, who): Self::Key<'_>,
	) -> Result<Self::Value, DispatchError> {
		Ok(Self::has_role(collection, who, role))
	}
}
