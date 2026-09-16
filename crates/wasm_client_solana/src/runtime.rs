use std::fmt;

use serde::Deserialize;
use serde::Serialize;

/// The reason a reward was paid to an account in a slot's reward list.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum RewardType {
	/// Transaction fee, paid to the block producer and voters.
	Fee,
	/// Rent collected from accounts that fell below the rent-exempt minimum.
	Rent,
	/// Staking reward, paid for delegated stake.
	Staking,
	/// Voting reward, paid to vote accounts for participation.
	Voting,
}

impl fmt::Display for RewardType {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"{}",
			match self {
				RewardType::Fee => "fee",
				RewardType::Rent => "rent",
				RewardType::Staking => "staking",
				RewardType::Voting => "voting",
			}
		)
	}
}
