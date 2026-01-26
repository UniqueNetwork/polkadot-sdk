use crate::DoubleEncoded;
use codec::{Decode, DecodeWithMemTracking, Encode};
use derive_where::derive_where;
use scale_info::TypeInfo;

use crate::{
	v5::{OriginKind, Weight},
	IdentifyVersion, Versioned,
};

struct TestVer;
impl Versioned<1> for TestVer {
	type Type = u8;
}
impl Versioned<2> for TestVer {
	type Type = i16;
}
impl Versioned<3> for TestVer {
	type Type = u32;
}
impl Versioned<4> for TestVer {
	type Type = u8;
}
impl Versioned<5> for TestVer {
	type Type = char;
}

#[xcm_procedural::versioned]
#[version(1..=5)]
#[derive_where(PartialEq, Eq, Debug, Clone)]
// #[versioned_enum(derive(PartialEq, Eq, Debug))]
// #[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, /*xcm_procedural::XcmWeightInfoTrait,
// xcm_procedural::Builder*/)] #[codec(encode_bound())]
// #[codec(decode_bound())]
// #[codec(decode_with_mem_tracking_bound())]
// #[scale_info(bounds(), skip_type_params(Call))]
#[versioned_enum(derive(Encode, Decode, DecodeWithMemTracking, TypeInfo))]
#[versioned_enum(derive_where(Clone, Eq, PartialEq, Debug))]
#[versioned_enum_variants(codec(index = version!()))]
pub(crate) enum Instruction<Call> {
	#[version(1..)]
	#[for_struct(derive(Debug, PartialEq, Eq, Clone))]
	WithdrawAsset(#[version(flatten)] TestVer),

	#[version(1..=2)]
	#[removed(since = 3)]
	Transact {
		origin_kind: OriginKind,
		fallback_max_weight: Option<Weight>,
		call: DoubleEncoded<Call>,
	},
}

#[test]
fn identify_version() {
	let w3 = v3::Instruction::<()>::WithdrawAsset(42u32);
	let w4: <VersionedInstruction<_> as Versioned<4>>::Type =
		v4::Instruction::<()>::WithdrawAsset(42u8);
	let w5 = v5::Instruction::<()>::WithdrawAsset('a');

	let a = VersionedInstruction::from(w3);
	let b = VersionedInstruction::from(w4);
	let c = VersionedInstruction::from(w5);

	assert_eq!(a.identify_version(), 3);
	assert_eq!(b.identify_version(), 4);
	assert_eq!(c.identify_version(), 5);
}

#[test]
fn try_froms() {
	let w3 = v3::WithdrawAsset(112u32);

	let w4: v4::WithdrawAsset = w3.clone().try_into().unwrap();
	let ww4 = v4::WithdrawAsset(112u8);

	let ww3: v3::WithdrawAsset = ww4.clone().try_into().unwrap();

	assert_eq!(w4, ww4);
	assert_eq!(w3, ww3);

	let instr3 = v3::Instruction::<()>::from(w3);
	let instr4 = v4::Instruction::<()>::from(w4);

	let instr4from3 = v4::Instruction::try_from(instr3.clone()).unwrap();
	let instr3from4 = v3::Instruction::try_from(instr4.clone()).unwrap();
	assert_eq!(instr4, instr4from3);
	assert_eq!(instr3, instr3from4);
}

#[test]
fn froms() {
	let w3 = v3::Instruction::WithdrawAsset(42u32);
	let w4 = v4::Instruction::WithdrawAsset(42u8);
	let w5 = v5::Instruction::WithdrawAsset('a');

	let a = VersionedInstruction::V3(w3.clone());
	let b = VersionedInstruction::V4(w4.clone());
	let c = VersionedInstruction::V5(w5.clone());

	let aa: VersionedInstruction<()> = w3.into();
	let bb: VersionedInstruction<()> = w4.into();
	let cc: VersionedInstruction<()> = w5.into();

	assert_eq!(a, aa);
	assert_eq!(b, bb);
	assert_eq!(c, cc);
}
