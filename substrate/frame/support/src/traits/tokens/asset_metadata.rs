use crate::dispatch::DispatchResult;
use sp_runtime::DispatchError;

pub trait MetadataStrategy {
    type InnermostStrategy: MetadataStrategy;
}

pub trait MetadataDefinition<AssetKind, Strategy: MetadataStrategy> {
    type Key<'a>;
    type Value;
}

pub trait InspectMetadata<AssetKind, Strategy: MetadataStrategy>: MetadataDefinition<AssetKind, Strategy::InnermostStrategy> {
    fn asset_metadata(key: Self::Key<'_>, strategy: Strategy) -> Result<Self::Value, DispatchError>;
}

pub trait UpdateMetadata<AssetKind, Strategy: MetadataStrategy>: MetadataDefinition<AssetKind, Strategy::InnermostStrategy> {
    fn update_asset_metadata(
        key: Self::Key<'_>,
        strategy: Strategy,
        update: Option<&Self::Value>
    ) -> DispatchResult;
}
