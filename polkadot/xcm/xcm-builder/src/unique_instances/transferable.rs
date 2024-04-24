use super::{LOG_TARGET, transfer_instance};
use core::marker::PhantomData;
use frame_support::traits::{
	tokens::asset_ops::{
		common_asset_kinds::Instance,
		common_strategies::FromTo,
		Transfer,
	},
	Get,
};
use xcm::latest::prelude::*;
use xcm_executor::traits::{ConvertLocation, MatchesInstance, TransactAsset};

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
