use std::ops::Range;
use syn::{Generics, Attribute, ItemEnum};
use super::{Version, VersionVariant};

pub struct VersionEnum {
	version_enum_generics: Generics,
	version_enum_attrs: Vec<Attribute>,
	variants: Vec<VersionVariant>,
}
impl VersionEnum {
	pub fn new(version_range: Range<Version>, version: Version, enum_template: &ItemEnum) -> Self {
		// let version_enum_attrs = enum_template.attrs.clone();

		// // TODO take only needed generics

		// let variants = enum_template
		// 	.variants
		// 	.iter()
		// 	.cloned()
		// 	.filter_map(|variant| {
		// 		VersionVariant::from(
		// 			&version_range,
		// 			version,
		// 			variant,
		// 			&item_enum.generics, // FIXME
		// 		).transpose()
		// 	})
		// 	.collect::<Result<Vec<_>>>()?;

		todo!()
	}
}
