// Copyright (C) 2021 Georges Dib.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Mocks for ecosystem_perpetuals_exchange module.

#![cfg(test)]
#![allow(deprecated)]

use super::*;
use frame_support::{construct_runtime, ord_parameter_types, parameter_types};
use frame_system::{EnsureRoot, EnsureSignedBy};
use pallet_treasury::DefaultInstance;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};
use sp_std::cell::RefCell;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, PartialOrd, Ord)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CurrencyId {
	KUSD,
	DOT,
	LDOT,
}

pub type BlockNumber = u64;
pub type AccountId = u128;
pub type Balance = u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;
pub const GEORGES: AccountId = 4;

mod ecosystem_perpetuals_exchange {
	pub use super::super::*;
}

ord_parameter_types! {
	pub const Alice: AccountId = ALICE;
}

parameter_types!(
	pub const BlockHashCount: BlockNumber = 250;
	pub const PerpetualAssetModuleId: ModuleId = ModuleId(*b"aca/pasm");
	pub const PerpetualsTreasuryPalletId: ModuleId = ModuleId(*b"aca/ptsy");
	pub const NativeCurrencyId: CurrencyId = CurrencyId::KUSD;
	pub AssetIds: Vec<CurrencyId> = vec![CurrencyId::DOT, CurrencyId::LDOT];
);

impl frame_system::Config for Runtime {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

impl pallet_treasury::Config for Runtime {
	type ModuleId = PerpetualsTreasuryPalletId;
	type Currency = Balances;
	type ApproveOrigin = EnsureRoot<AccountId>;
	type RejectOrigin = EnsureRoot<AccountId>;
	type Event = Event;
	type OnSlash = ();
	type ProposalBond = ();
	type ProposalBondMinimum = ();
	type SpendPeriod = ();
	type Burn = ();
	type BurnDestination = ();
	type SpendFunds = ();
	type WeightInfo = ();
}

impl pallet_balances::Config for Runtime {
	type MaxLocks = ();
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ();
	type AccountStore = System;
	type WeightInfo = ();
}

thread_local! {
	static PRICE_DOT: RefCell<Option<FixedU128>> = RefCell::new(Some(FixedU128::one()));
	static PRICE_LDOT: RefCell<Option<FixedU128>> = RefCell::new(Some(FixedU128::one()));
}

pub struct MockPriceSource;

impl MockPriceSource {
	pub fn set_price(currency_id: CurrencyId, price: Option<FixedU128>) {
		if currency_id == CurrencyId::DOT {
			PRICE_DOT.with(|v| *v.borrow_mut() = price);
		} else {
			PRICE_LDOT.with(|v| *v.borrow_mut() = price);
		}
	}
}

impl PriceProvider<CurrencyId> for MockPriceSource {
	fn get_price(currency_id: CurrencyId) -> Option<FixedU128> {
		if currency_id == CurrencyId::DOT {
			return PRICE_DOT.with(|v| *v.borrow_mut());
		}

		PRICE_LDOT.with(|v| *v.borrow_mut())
	}
}

impl ecosystem_perpetuals_exchange::Config for Runtime {
	type Event = Event;
	type UpdateOrigin = EnsureSignedBy<Alice, AccountId>;
	type AssetId = CurrencyId;
	type ModuleId = PerpetualAssetModuleId;
	type Currency = Balances;
	type AssetIds = AssetIds;
	type PriceSource = MockPriceSource;
	type Treasury = Treasury;
	type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Module, Call, Event<T>},
		PerpetualsExchange: ecosystem_perpetuals_exchange::{Module, Call, Event<T>, Config<T>, Storage},
		Treasury: pallet_treasury::{Module, Call, Storage, Config, Event<T>},
		Balances: pallet_balances::{Module, Call, Storage, Config<T>, Event<T>},
	}
);

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, Balance)>,
	collaterals_params: Vec<(CurrencyId, Permill, Permill, Permill)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, 1_000_000_000_000_000_000u128),
				(BOB, 1_000_000_000_000_000_000u128),
				(CHARLIE, 1_000_000_000_000_000_000u128),
				(GEORGES, 1_000_000_000_000_000_000u128),
			],
			collaterals_params: vec![
				(
					CurrencyId::DOT,
					Permill::from_percent(20), // Initial IM Ratio
					Permill::from_percent(10), // liquidation ratio
					Permill::from_parts(1000), // transaction fee
				),
				(
					CurrencyId::LDOT,
					Permill::from_percent(30),  // Initial IM Ratio
					Permill::from_percent(10),  // liquidation ratio
					Permill::from_parts(20000), // transaction fee
				),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_treasury::GenesisConfig::default()
			.assimilate_storage::<Runtime, DefaultInstance>(&mut t)
			.unwrap();

		ecosystem_perpetuals_exchange::GenesisConfig::<Runtime> {
			collaterals_params: self.collaterals_params,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
