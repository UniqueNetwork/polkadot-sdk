use core::marker::PhantomData;
use frame_support::traits::{
	asset_ops::{
		common_asset_kinds::{Class, Instance},
		common_strategies::{FromTo, IfOwnedBy, SecondaryTo, WithKnownId, WithOwner},
		AssetDefinition, Create, Destroy, SecondaryAsset, Transfer,
	},
	Get,
};
use xcm::latest::prelude::*;
use xcm_executor::traits::{ConvertLocation, Error as MatchError, MatchesInstance, TransactAsset};

const LOG_TARGET: &str = "xcm::unique_instances";

pub struct TransferableInstanceAdapter<
	AccountId,
	AccountIdConverter,
	Matcher,
	InstanceTransfer,
	StashLocation,
>(PhantomData<(AccountId, AccountIdConverter, Matcher, InstanceTransfer, StashLocation)>);

impl<
		AccountId,
		AccountIdConverter: ConvertLocation<AccountId>,
		Matcher: MatchesInstance<InstanceTransfer::Id>,
		InstanceTransfer: for<'a> Transfer<Instance, FromTo<'a, AccountId>>,
		StashLocation: Get<Location>,
	> TransactAsset
	for TransferableInstanceAdapter<
		AccountId,
		AccountIdConverter,
		Matcher,
		InstanceTransfer,
		StashLocation,
	>
{
	fn deposit_asset(what: &Asset, who: &Location, context: Option<&XcmContext>) -> XcmResult {
		log::trace!(
			target: LOG_TARGET,
			"TransferableInstanceAdapter::deposit_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			context,
		);

		transfer_instance::<AccountId, AccountIdConverter, Matcher, InstanceTransfer>(
			what,
			&StashLocation::get(),
			who,
		)
	}

	fn withdraw_asset(
		what: &Asset,
		who: &Location,
		maybe_context: Option<&XcmContext>,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"TransferableInstanceAdapter::withdraw_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			maybe_context,
		);

		transfer_instance::<AccountId, AccountIdConverter, Matcher, InstanceTransfer>(
			what,
			who,
			&StashLocation::get(),
		)?;

		Ok(what.clone().into())
	}

	fn internal_transfer_asset(
		what: &Asset,
		from: &Location,
		to: &Location,
		context: &XcmContext,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"TransferableInstanceAdapter::internal_transfer_asset what: {:?}, from: {:?}, to: {:?}, context: {:?}",
			what,
			from,
			to,
			context,
		);

		transfer_instance::<AccountId, AccountIdConverter, Matcher, InstanceTransfer>(
			what, from, to,
		)?;

		Ok(what.clone().into())
	}
}

pub struct RecreateableInstanceAdapter<AccountId, AccountIdConverter, Matcher, InstanceOps>(
	PhantomData<(AccountId, AccountIdConverter, Matcher, InstanceOps)>,
);

impl<AccountId, AccountIdConverter, Matcher, InstanceOps> TransactAsset
	for RecreateableInstanceAdapter<AccountId, AccountIdConverter, Matcher, InstanceOps>
where
	AccountIdConverter: ConvertLocation<AccountId>,
	Matcher: MatchesInstance<InstanceOps::Id>,
	for<'a> InstanceOps: Create<Instance, WithOwner<'a, AccountId, WithKnownId<'a, InstanceOps::Id>>>
		+ Transfer<Instance, FromTo<'a, AccountId>>
		+ Destroy<Instance, IfOwnedBy<'a, AccountId>>,
{
	fn deposit_asset(what: &Asset, who: &Location, context: Option<&XcmContext>) -> XcmResult {
		log::trace!(
			target: LOG_TARGET,
			"RecreateableInstanceAdapter::deposit_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			context,
		);

		let instance_id = Matcher::matches_instance(what)?;
		let who = AccountIdConverter::convert_location(who)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		InstanceOps::create(WithOwner(&who, WithKnownId(&instance_id)))
			.map_err(|e| XcmError::FailedToTransactAsset(e.into()))
	}

	fn withdraw_asset(
		what: &Asset,
		who: &Location,
		maybe_context: Option<&XcmContext>,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"RecreateableInstanceAdapter::withdraw_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			maybe_context,
		);
		let instance_id = Matcher::matches_instance(what)?;
		let who = AccountIdConverter::convert_location(who)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		InstanceOps::destroy(&instance_id, IfOwnedBy(&who))
			.map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

		Ok(what.clone().into())
	}

	fn internal_transfer_asset(
		what: &Asset,
		from: &Location,
		to: &Location,
		context: &XcmContext,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"RecreateableInstanceAdapter::internal_transfer_asset what: {:?}, from: {:?}, to: {:?}, context: {:?}",
			what,
			from,
			to,
			context,
		);

		transfer_instance::<AccountId, AccountIdConverter, Matcher, InstanceOps>(what, from, to)?;

		Ok(what.clone().into())
	}
}

fn transfer_instance<
	AccountId,
	AccountIdConverter: ConvertLocation<AccountId>,
	Matcher: MatchesInstance<InstanceTransfer::Id>,
	InstanceTransfer: for<'a> Transfer<Instance, FromTo<'a, AccountId>>,
>(
	what: &Asset,
	from: &Location,
	to: &Location,
) -> XcmResult {
	let instance_id = Matcher::matches_instance(what)?;
	let from =
		AccountIdConverter::convert_location(from).ok_or(MatchError::AccountIdConversionFailed)?;
	let to =
		AccountIdConverter::convert_location(to).ok_or(MatchError::AccountIdConversionFailed)?;

	InstanceTransfer::transfer(&instance_id, FromTo(&from, &to))
		.map_err(|e| XcmError::FailedToTransactAsset(e.into()))
}

pub enum DerivativeStatus<ClassId, InstanceId> {
	DepositableIn(ClassId),
	Exists(InstanceId),
}

pub struct BackedDerivativeInstanceAdapter<
	AccountId,
	AccountIdConverter,
	Matcher,
	InstanceOps,
	StashLocation,
>(PhantomData<(AccountId, AccountIdConverter, Matcher, InstanceOps, StashLocation)>);

impl<AccountId, AccountIdConverter, Matcher, InstanceOps, StashLocation> TransactAsset
	for BackedDerivativeInstanceAdapter<
		AccountId,
		AccountIdConverter,
		Matcher,
		InstanceOps,
		StashLocation,
	> where
	AccountIdConverter: ConvertLocation<AccountId>,
	Matcher: MatchesInstance<
		DerivativeStatus<
			<InstanceOps::PrimaryAsset as AssetDefinition<Class>>::Id,
			InstanceOps::Id,
		>,
	>,
	for<'a> InstanceOps: SecondaryAsset<Class, Instance>
		+ Create<Instance, WithOwner<'a, AccountId, SecondaryTo<'a, Class, Instance, InstanceOps>>>
		+ Transfer<Instance, FromTo<'a, AccountId>>,
	StashLocation: Get<Location>,
{
	fn deposit_asset(what: &Asset, who: &Location, context: Option<&XcmContext>) -> XcmResult {
		log::trace!(
			target: LOG_TARGET,
			"BackedDerivativeInstanceAdapter::deposit_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			context,
		);

		let derivative_status = Matcher::matches_instance(what)?;
		let to = AccountIdConverter::convert_location(who)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		let result = match derivative_status {
			DerivativeStatus::DepositableIn(class_id) =>
				InstanceOps::create(WithOwner(&to, SecondaryTo::from_primary_id(&class_id)))
					.map(|_id| ()),
			DerivativeStatus::Exists(instance_id) => {
				let from = AccountIdConverter::convert_location(&StashLocation::get())
					.ok_or(MatchError::AccountIdConversionFailed)?;

				InstanceOps::transfer(&instance_id, FromTo(&from, &to))
			},
		};

		result.map_err(|e| XcmError::FailedToTransactAsset(e.into()))
	}

	fn withdraw_asset(
		what: &Asset,
		who: &Location,
		maybe_context: Option<&XcmContext>,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"BackedDerivativeInstanceAdapter::withdraw_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			maybe_context,
		);

		let derivative_status = Matcher::matches_instance(what)?;
		let from = AccountIdConverter::convert_location(who)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		if let DerivativeStatus::Exists(instance_id) = derivative_status {
			let to = AccountIdConverter::convert_location(&StashLocation::get())
				.ok_or(MatchError::AccountIdConversionFailed)?;

			InstanceOps::transfer(&instance_id, FromTo(&from, &to))
				.map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

			Ok(what.clone().into())
		} else {
			Err(XcmError::NotWithdrawable)
		}
	}

	fn internal_transfer_asset(
		what: &Asset,
		from: &Location,
		to: &Location,
		context: &XcmContext,
	) -> Result<xcm_executor::AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"BackedDerivativeInstanceAdapter::internal_transfer_asset what: {:?}, from: {:?}, to: {:?}, context: {:?}",
			what,
			from,
			to,
			context,
		);

		let derivative_status = Matcher::matches_instance(what)?;
		let from = AccountIdConverter::convert_location(from)
			.ok_or(MatchError::AccountIdConversionFailed)?;
		let to = AccountIdConverter::convert_location(to)
			.ok_or(MatchError::AccountIdConversionFailed)?;

		if let DerivativeStatus::Exists(instance_id) = derivative_status {
			InstanceOps::transfer(&instance_id, FromTo(&from, &to))
				.map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

			Ok(what.clone().into())
		} else {
			Err(XcmError::NotWithdrawable)
		}
	}
}
