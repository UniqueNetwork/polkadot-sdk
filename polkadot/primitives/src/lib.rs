// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Polkadot types shared between the runtime and the Node-side code.

#![warn(missing_docs)]
#![cfg_attr(not(feature = "std"), no_std)]

// `v11` is currently the latest stable version of the runtime API.
pub mod v8;

// The 'staging' version is special - it contains primitives which are
// still in development. Once they are considered stable, they will be
// moved to a new versioned module.
pub mod vstaging;

// `runtime_api` contains the actual API implementation. It contains stable and
// unstable functions.
pub mod runtime_api;

extern crate alloc;

// Current primitives not requiring versioning are exported here.
// Primitives requiring versioning must not be exported and must be referred by an exact version.
pub use v8::{
	async_backing, byzantine_threshold, check_candidate_backing, collator_signature_payload,
	effective_minimum_backing_votes, executor_params, metric_definitions, node_features, slashing,
	supermajority_threshold, well_known_keys, AbridgedHostConfiguration, AbridgedHrmpChannel,
	AccountId, AccountIndex, AccountPublic, ApprovalVote, ApprovalVoteMultipleCandidates,
	ApprovalVotingParams, AssignmentId, AsyncBackingParams, AuthorityDiscoveryId,
	AvailabilityBitfield, BackedCandidate, Balance, BlakeTwo256, Block, BlockId, BlockNumber,
	CandidateCommitments, CandidateDescriptor, CandidateEvent, CandidateHash, CandidateIndex,
	CandidateReceipt, CheckedDisputeStatementSet, CheckedMultiDisputeStatementSet, ChunkIndex,
	CollatorId, CollatorSignature, CommittedCandidateReceipt, CompactStatement, ConsensusLog,
	CoreIndex, CoreState, DisputeState, DisputeStatement, DisputeStatementSet, DownwardMessage,
	EncodeAs, ExecutorParam, ExecutorParamError, ExecutorParams, ExecutorParamsHash,
	ExecutorParamsPrepHash, ExplicitDisputeStatement, GroupIndex, GroupRotationInfo, Hash, HashT,
	HeadData, Header, HorizontalMessages, HrmpChannelId, Id, InboundDownwardMessage,
	InboundHrmpMessage, IndexedVec, InherentData, InvalidDisputeStatementKind, Moment,
	MultiDisputeStatementSet, NodeFeatures, Nonce, OccupiedCore, OccupiedCoreAssumption,
	OutboundHrmpMessage, ParathreadClaim, ParathreadEntry, PersistedValidationData,
	PvfCheckStatement, PvfExecKind, PvfPrepKind, RuntimeMetricLabel, RuntimeMetricLabelValue,
	RuntimeMetricLabelValues, RuntimeMetricLabels, RuntimeMetricOp, RuntimeMetricUpdate,
	ScheduledCore, SchedulerParams, ScrapedOnChainVotes, SessionIndex, SessionInfo, Signature,
	Signed, SignedAvailabilityBitfield, SignedAvailabilityBitfields, SignedStatement,
	SigningContext, Slot, UncheckedSigned, UncheckedSignedAvailabilityBitfield,
	UncheckedSignedAvailabilityBitfields, UncheckedSignedStatement, UpgradeGoAhead,
	UpgradeRestriction, UpwardMessage, ValidDisputeStatementKind, ValidationCode,
	ValidationCodeHash, ValidatorId, ValidatorIndex, ValidatorSignature, ValidityAttestation,
	ValidityError, ASSIGNMENT_KEY_TYPE_ID, DEFAULT_SCHEDULING_LOOKAHEAD, LEGACY_MIN_BACKING_VOTES,
	LOWEST_PUBLIC_ID, MAX_CODE_SIZE, MAX_HEAD_DATA_SIZE, MAX_POV_SIZE, MIN_CODE_SIZE,
	ON_DEMAND_DEFAULT_QUEUE_MAX_SIZE, ON_DEMAND_MAX_QUEUE_MAX_SIZE, PARACHAINS_INHERENT_IDENTIFIER,
	PARACHAIN_KEY_TYPE_ID,
};

#[cfg(feature = "std")]
pub use v8::{AssignmentPair, CollatorPair, ValidatorPair};
