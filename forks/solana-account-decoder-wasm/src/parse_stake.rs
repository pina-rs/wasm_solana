use bincode::deserialize;
use serde::Deserialize;
use serde::Serialize;
use solana_clock::Epoch;
use solana_clock::UnixTimestamp;
use solana_stake_interface::state::Authorized;
use solana_stake_interface::state::Delegation;
use solana_stake_interface::state::Lockup;
use solana_stake_interface::state::Meta;
use solana_stake_interface::state::Stake;
use solana_stake_interface::state::StakeStateV2;

use crate::StringAmount;
use crate::parse_account_data::ParsableAccount;
use crate::parse_account_data::ParseAccountError;

/// Deserialize a stake account into its JSON form.
///
/// Returns [`ParseAccountError::AccountNotParsable`] if the data is not a
/// valid bincode serialized [`StakeStateV2`].
pub fn parse_stake(data: &[u8]) -> Result<StakeAccountType, ParseAccountError> {
	let stake_state: StakeStateV2 = deserialize(data)
		.map_err(|_| ParseAccountError::AccountNotParsable(ParsableAccount::Stake))?;
	let parsed_account = match stake_state {
		StakeStateV2::Uninitialized => StakeAccountType::Uninitialized,
		StakeStateV2::Initialized(meta) => {
			StakeAccountType::Initialized(UiStakeAccount {
				meta: meta.into(),
				stake: None,
			})
		}
		StakeStateV2::Stake(meta, stake, _) => {
			StakeAccountType::Delegated(UiStakeAccount {
				meta: meta.into(),
				stake: Some(stake.into()),
			})
		}
		StakeStateV2::RewardsPool => StakeAccountType::RewardsPool,
	};
	Ok(parsed_account)
}

/// The parsed contents of a stake account, tagged by state in JSON.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "type", content = "info")]
pub enum StakeAccountType {
	/// The account has not been initialized.
	Uninitialized,
	/// The account is initialized but not delegated.
	Initialized(UiStakeAccount),
	/// The account is delegated to a vote account.
	Delegated(UiStakeAccount),
	/// The account is a rewards pool account.
	RewardsPool,
}

/// Parsed stake account state and optional delegation.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiStakeAccount {
	/// The account's metadata.
	pub meta: UiMeta,
	/// The delegation details, absent unless the account is delegated.
	pub stake: Option<UiStake>,
}

/// Parsed stake account metadata.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiMeta {
	/// Rent exempt reserve held by the stake account, as a string.
	#[deprecated(
		since = "4.1.0",
		note = "Stake account rent must be calculated via the `Rent` sysvar. This value will \
		        cease to be correct once lamports-per-byte is adjusted."
	)]
	pub rent_exempt_reserve: StringAmount,
	/// The staker and withdrawer authorities.
	pub authorized: UiAuthorized,
	/// The lockup that restricts withdrawals.
	pub lockup: UiLockup,
}

impl From<Meta> for UiMeta {
	fn from(meta: Meta) -> Self {
		Self {
			#[expect(deprecated)]
			rent_exempt_reserve: meta.rent_exempt_reserve.to_string(),
			authorized: meta.authorized.into(),
			lockup: meta.lockup.into(),
		}
	}
}

/// Parsed stake lockup.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiLockup {
	/// Unix timestamp at which the lockup expires.
	pub unix_timestamp: UnixTimestamp,
	/// Epoch at which the lockup expires.
	pub epoch: Epoch,
	/// The pubkey that may override the lockup, as a string.
	pub custodian: String,
}

impl From<Lockup> for UiLockup {
	fn from(lockup: Lockup) -> Self {
		Self {
			unix_timestamp: lockup.unix_timestamp,
			epoch: lockup.epoch,
			custodian: lockup.custodian.to_string(),
		}
	}
}

/// Parsed stake authorities.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UiAuthorized {
	/// The staker authority, as a string.
	pub staker: String,
	/// The withdrawer authority, as a string.
	pub withdrawer: String,
}

impl From<Authorized> for UiAuthorized {
	fn from(authorized: Authorized) -> Self {
		Self {
			staker: authorized.staker.to_string(),
			withdrawer: authorized.withdrawer.to_string(),
		}
	}
}

/// Parsed stake of a delegated account.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiStake {
	/// The delegation to a vote account.
	pub delegation: UiDelegation,
	/// Number of stake credits observed when the delegation was last updated.
	pub credits_observed: u64,
}

impl From<Stake> for UiStake {
	fn from(stake: Stake) -> Self {
		Self {
			delegation: stake.delegation.into(),
			credits_observed: stake.credits_observed,
		}
	}
}

/// Parsed delegation details.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UiDelegation {
	/// The vote account the stake is delegated to, as a string.
	pub voter: String,
	/// The amount of stake delegated, in lamports as a string.
	pub stake: StringAmount,
	/// The epoch in which the stake became active, as a string.
	pub activation_epoch: StringAmount,
	/// The epoch in which the stake was deactivated, as a string.
	pub deactivation_epoch: StringAmount,
}

impl From<Delegation> for UiDelegation {
	fn from(delegation: Delegation) -> Self {
		#[allow(deprecated)]
		Self {
			voter: delegation.voter_pubkey.to_string(),
			stake: delegation.stake.to_string(),
			activation_epoch: delegation.activation_epoch.to_string(),
			deactivation_epoch: delegation.deactivation_epoch.to_string(),
		}
	}
}

#[cfg(test)]
mod test {
	use bincode::serialize;
	use solana_stake_interface::stake_flags::StakeFlags;

	use super::*;

	#[test]
	#[allow(deprecated)]
	fn test_parse_stake() {
		let stake_state = StakeStateV2::Uninitialized;
		let stake_data = serialize(&stake_state).unwrap();
		assert_eq!(
			parse_stake(&stake_data).unwrap(),
			StakeAccountType::Uninitialized
		);

		let pubkey = solana_pubkey::new_rand();
		let custodian = solana_pubkey::new_rand();
		let authorized = Authorized::auto(&pubkey);
		let lockup = Lockup {
			unix_timestamp: 0,
			epoch: 1,
			custodian,
		};
		let meta = Meta {
			rent_exempt_reserve: 42,
			authorized,
			lockup,
		};

		let stake_state = StakeStateV2::Initialized(meta);
		let stake_data = serialize(&stake_state).unwrap();
		assert_eq!(
			parse_stake(&stake_data).unwrap(),
			StakeAccountType::Initialized(UiStakeAccount {
				meta: UiMeta {
					rent_exempt_reserve: 42.to_string(),
					authorized: UiAuthorized {
						staker: pubkey.to_string(),
						withdrawer: pubkey.to_string(),
					},
					lockup: UiLockup {
						unix_timestamp: 0,
						epoch: 1,
						custodian: custodian.to_string(),
					}
				},
				stake: None,
			})
		);

		let voter_pubkey = solana_pubkey::new_rand();
		let stake = Stake {
			delegation: Delegation {
				voter_pubkey,
				stake: 20,
				activation_epoch: 2,
				deactivation_epoch: u64::MAX,
				_reserved: [0; 8],
			},
			credits_observed: 10,
		};

		let stake_state = StakeStateV2::Stake(meta, stake, StakeFlags::empty());
		let stake_data = serialize(&stake_state).unwrap();
		assert_eq!(
			parse_stake(&stake_data).unwrap(),
			StakeAccountType::Delegated(UiStakeAccount {
				meta: UiMeta {
					rent_exempt_reserve: 42.to_string(),
					authorized: UiAuthorized {
						staker: pubkey.to_string(),
						withdrawer: pubkey.to_string(),
					},
					lockup: UiLockup {
						unix_timestamp: 0,
						epoch: 1,
						custodian: custodian.to_string(),
					}
				},
				stake: Some(UiStake {
					delegation: UiDelegation {
						voter: voter_pubkey.to_string(),
						stake: 20.to_string(),
						activation_epoch: 2.to_string(),
						deactivation_epoch: u64::MAX.to_string(),
					},
					credits_observed: 10,
				})
			})
		);

		let stake_state = StakeStateV2::RewardsPool;
		let stake_data = serialize(&stake_state).unwrap();
		assert_eq!(
			parse_stake(&stake_data).unwrap(),
			StakeAccountType::RewardsPool
		);

		let bad_data = vec![1, 2, 3, 4];
		assert!(parse_stake(&bad_data).is_err());
	}
}
