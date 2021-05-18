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

//! # PerpetualAsset Module
//!
//! ## Overview
//!
//! Given an asset for which an Oracle can provide a price, give a way
//! for longs and shorts to express their view

// TODO: add weight stuff, and benchmark it
// TODO: allow any sort of payoff
// TODO: make documentation better
// TODO: clean up code
// TODO: check collateral redeeming cases, for now if pool is at a loss
//       there is a race, and the first person to claim collateral takes
//       more than the others (the others may end up with 0!)
// TODO: Should I clean 0 balances to clear up storage?
// TODO: move liquidation and all this to offchain worker

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, OnUnbalanced, WithdrawReasons},
	transactional,
};
use frame_system::pallet_prelude::*;
use codec::FullCodec;

use sp_arithmetic::Perquintill;
use sp_runtime::{
	traits::{AccountIdConversion, Zero},
	FixedPointNumber, Permill, ModuleId, FixedU128,
};
use sp_std::{convert::TryInto, result, vec::Vec, fmt::Debug,};

/// Indicate if should change a value
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub enum Change<Value> {
	/// No change.
	NoChange,
	/// Changed to new value.
	NewValue(Value),
}

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

type PalletBalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

pub trait PriceProvider<T> {
	fn get_price(currency_id: T) -> Option<FixedU128>;
}

/// Asset params
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default)]
pub struct AssetParams {
	pub initial_im_ratio: Permill,
	pub liquidation_ratio: Permill,
	pub transaction_fee: Permill,
}

// typedef to help polkadot.js disambiguate Change with different generic
// parameters
type ChangePermill = Change<Permill>;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The asset identifier.
		type AssetId: FullCodec + Eq + PartialEq + Copy + MaybeSerializeDeserialize + Debug;

		/// The origin which may update risk management parameters. Root can
		/// always do this.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// The synthetic's module id, keep all collaterals.
		#[pallet::constant]
		type ModuleId: Get<ModuleId>;

		/// The list of valid asset types
		#[pallet::constant]
		type AssetIds: Get<Vec<Self::AssetId>>;

		/// The currency type in which fees will be paid.
		type Currency: Currency<Self::AccountId>;

		/// The treasury for funds
		type Treasury: OnUnbalanced<NegativeImbalanceOf<Self>>;

		/// Price provider, TODO work on that to make it more generic
		type PriceSource: PriceProvider<Self::AssetId>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Not enough IM is sent
		NotEnoughIM,
		/// Fail to convert from i128 to u128 and vice versa
		AmountConvertFailed,
		/// Overflow
		Overflow,
		/// Emitted when trying to redeem without enough balance
		NotEnoughBalance,
		/// Emitted when P0 not set
		PriceNotSet,
		/// Bad Asset ID,
		BadAssetID,
		/// Bad parameters being set
		BadIMParameters,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Emitted when collateral is updated by \[i128\]
		CollateralUpdated(i128),
		/// Emitted when the balance of \[T::AccountId\] is updated to
		/// \[i128\]
		BalanceUpdated(T::AccountId, i128),
		/// Emitted when IM ratio of \[AssetId\] is updated by \[Permill\]
		InitialIMRatioUpdated(T::AssetId, Permill),
		/// Emitted when liquidation ratio of \[AssetId\] is updated by \[Permill\]
		LiquidationRatioUpdated(T::AssetId, Permill),
		/// Emitted when transaction fee of \[AssetId\] is updated by \[Permill\]
		TransactionFeeUpdated(T::AssetId, Permill),
	}

	#[pallet::storage]
	#[pallet::getter(fn collateral_params)]
	pub type CollateralParams<T: Config> = StorageMap<_, Twox64Concat, T::AssetId, AssetParams, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn balances)]
	pub(crate) type Balances<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AssetId, Twox64Concat, T::AccountId, i128, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn inventory)]
	pub(crate) type Inventory<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::AssetId, Twox64Concat, T::AccountId, i128, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn margin)]
	pub(crate) type Margin<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, u128, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn price0)]
	pub(crate) type Price0<T: Config> = StorageMap<_, Twox64Concat, T::AssetId, FixedU128, OptionQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		#[allow(clippy::type_complexity)]
		pub collaterals_params: Vec<(T::AssetId, Permill, Permill, Permill)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				collaterals_params: vec![],
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			self.collaterals_params
				.iter()
				.for_each(|(id, initial_im_ratio, liquidation_ratio, transaction_fee)| {
					CollateralParams::<T>::insert(
						id,
						AssetParams {
							initial_im_ratio: *initial_im_ratio,
							liquidation_ratio: *liquidation_ratio,
							transaction_fee: *transaction_fee,
						},
					);
				});
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_n: T::BlockNumber) -> Weight {
			// TODO: this is called multiple times and not just at block start
			for currency_id in T::AssetIds::get() {
				Self::update_margin(currency_id);
				Self::match_interest(currency_id);
			}
			Self::liquidate(); // TODO, liquidate should run before match_interest
				   // TODO change this to weightinfo, check cdp-engine
			10
		}

		// TODO: this on seems to be called only once
		fn on_finalize(_n: T::BlockNumber) {}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Update parameters related to risk management of the asset
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		///
		/// If you pass None as option, it does not override
		///
		/// - `initial_im_ratio`: Initial ratio needed for IM.
		/// - `liquidation_ratio`: Minimum ratio needed for liquidation.
		/// - `transaction_fee`: Transaction fee ratio taken by treasury.
		/// TODO: add weights for this
		#[pallet::weight((10_000, DispatchClass::Operational))]
		#[transactional]
		pub(super) fn set_global_params(
			origin: OriginFor<T>,
			currency_id: T::AssetId,
			initial_im_ratio: ChangePermill,
			liquidation_ratio: ChangePermill,
			transaction_fee: ChangePermill,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;

			ensure!(T::AssetIds::get().contains(&currency_id), Error::<T>::BadAssetID);

			let mut collateral_params = Self::collateral_params(currency_id);

			if let Change::NewValue(update) = initial_im_ratio {
				ensure!(
					update >= collateral_params.liquidation_ratio,
					Error::<T>::BadIMParameters
				);
				collateral_params.initial_im_ratio = update;
				Self::deposit_event(Event::InitialIMRatioUpdated(currency_id, update));
			}

			if let Change::NewValue(update) = liquidation_ratio {
				ensure!(update < collateral_params.initial_im_ratio, Error::<T>::BadIMParameters);
				collateral_params.liquidation_ratio = update;
				Self::deposit_event(Event::LiquidationRatioUpdated(currency_id, update));
			}

			if let Change::NewValue(update) = transaction_fee {
				collateral_params.transaction_fee = update;
				Self::deposit_event(Event::TransactionFeeUpdated(currency_id, update));
			}

			CollateralParams::<T>::insert(currency_id, collateral_params);

			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::mint_or_burn())]
		#[transactional]
		/// Mints the payoff
		/// - `origin`: the calling account
		/// - 'currency_id': The currency in use
		/// - `amount`: the amount of asset to be minted(can be positive or negative)
		/// - `collateral`: the amount of collateral in native currency
		pub(super) fn mint(
			origin: OriginFor<T>,
			currency_id: T::AssetId,
			amount: i128,
			collateral: i128,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let transaction_fee = Self::collateral_params(currency_id).transaction_fee;
	
			// Check if enough collateral
			let current_margin = Self::amount_try_from_balance(Margin::<T>::try_get(who.clone()).unwrap_or(0u128.into()))?;
			let price = Self::price0(currency_id).ok_or(Error::<T>::PriceNotSet)?;
			//TODO: very ugly
			let pos_amount = Self::balance_try_from_amount_abs(amount)?;
			let fee = price
				.checked_mul_int(pos_amount)
				.and_then(|res| Some(transaction_fee.mul_ceil(res)))
				.ok_or(Error::<T>::Overflow)?;
			let f = Self::amount_try_from_balance(fee)?;
	
			let new_collateral = collateral.checked_sub(f).ok_or(Error::<T>::Overflow)?;
	
			let (needed_im, balance) = Self::get_needed_im(&who, &currency_id, amount)?;
			let new_margin = current_margin.checked_add(new_collateral).ok_or(Error::<T>::Overflow)?;
	
			ensure!(new_margin >= needed_im, Error::<T>::NotEnoughIM);
	
			let module_account = Self::account_id();
			let positive_margin = Self::balance_try_from_amount_abs(new_margin)?;
			let pos_collateral = Self::balance_try_from_amount_abs(new_collateral)?;
			let positive_collateral = Self::balance_to_pallet_balance(pos_collateral)?;
	
			if new_collateral.is_positive() {
				// Transfer the collateral to the module's account
				T::Currency::transfer(
					&who,
					&module_account,
					positive_collateral,
					ExistenceRequirement::KeepAlive,
				)?;
			}
	
			if new_collateral.is_negative() {
				// Transfer the collateral from the module's account
				T::Currency::transfer(
					&module_account,
					&who,
					positive_collateral,
					ExistenceRequirement::KeepAlive,
				)?;
			}
	
			// transfer the fee
			let fee_balance = Self::balance_to_pallet_balance(fee)?;
			let imbalance =
				T::Currency::withdraw(&who, fee_balance, WithdrawReasons::FEE, ExistenceRequirement::KeepAlive)?;
	
			T::Treasury::on_unbalanced(imbalance);
	
			if !new_collateral.is_zero() {
				Margin::<T>::insert(who.clone(), positive_margin);
				Self::deposit_event(Event::CollateralUpdated(new_collateral));
			}
	
			// Update the balances
			Balances::<T>::insert(currency_id, who.clone(), balance);
			Self::deposit_event(Event::BalanceUpdated(who, balance));
	
			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn get_needed_im(
		account: &T::AccountId,
		currency_id: &T::AssetId,
		amount: i128,
	) -> result::Result<(i128, i128), Error<T>> {
		let mut total_im_needed: u128 = 0u128;
		let mut amt = 0;
		ensure!(T::AssetIds::get().contains(currency_id), Error::<T>::BadAssetID);
		for ccy_id in T::AssetIds::get() {
			let price = Self::price0(ccy_id).ok_or(Error::<T>::PriceNotSet)?;
			let initial_im_ratio = Self::collateral_params(ccy_id).initial_im_ratio;
			let mut balance = Balances::<T>::try_get(ccy_id, account.clone()).unwrap_or(0.into());
			if ccy_id == *currency_id {
				balance += amount;
				amt = balance;
			}
			let value = price.checked_mul_int(balance).ok_or(Error::<T>::Overflow)?;
			let value = Self::balance_try_from_amount_abs(value)?;
			total_im_needed += initial_im_ratio.mul_ceil(value);
		}
		let res = Self::amount_try_from_balance(total_im_needed)?;
		Ok((res, amt))
	}

	/// Call *M* the total margin for a participant *A*,
	/// Call $T_i$ the total interest in asset *i*, and $B_i$ the inventory
	/// (open interest is $T_i - B_i$) The needed collateral for maintaining
	/// the inventory is $\sum_i B_i * P_i * L_i$. If $`sum_i B_i * P_i * L_i >= M$,
	/// then liquididate the inventory as per below.
	/// If $\sum_i B_i * P_i * L_i < M$, but $\sum_i T_i * P_i * L_i > M$ then close out
	/// all open interest, so total interest becomes $\forall i, T_i = B_i$
	/// and inventory remains at $B_i$
	/// This is done to make sure that if an opposing open interest comes during
	/// that block, it does not suffer from immediate liquidation.
	///
	/// ### Liquidation of inventory
	/// If $\sum_i B_i * P_i * L_i >= M$, liquidate all the positions
	/// so total position and inventory goes to $\forall i, T_i = B_i = 0$
	fn liquidate() {
		for (account, margin) in Margin::<T>::iter() {
			let mut liquidation_sum = 0;
			let mut unwind_sum = 0;
			for currency_id in T::AssetIds::get() {
				//TODO handle no price better
				if let Some(price) = Self::price0(currency_id) {
					let liq_div = Self::collateral_params(currency_id).liquidation_ratio;

					// TODO handle overflow better (for example emergency shutdown) for the 2 statements below
					let inventory =
						Self::balance_try_from_amount_abs(Self::inventory(currency_id, account.clone())).unwrap();
					let balance =
						Self::balance_try_from_amount_abs(Balances::<T>::get(currency_id, account.clone())).unwrap();

					//TODO: replace the saturating mul by a checked one
					liquidation_sum += liq_div.mul_ceil(price.saturating_mul_int(inventory));
					unwind_sum += liq_div.mul_ceil(price.saturating_mul_int(balance));
				}
			}

			// am I in liquidation?
			if liquidation_sum >= margin {
				// Yes I am
				for currency_id in T::AssetIds::get() {
					Balances::<T>::insert(currency_id, account.clone(), 0);
					Inventory::<T>::insert(currency_id, account.clone(), 0);
				}
			} else if unwind_sum > margin {
				// remove open interest
				for currency_id in T::AssetIds::get() {
					let inventory = Self::inventory(currency_id, account.clone());
					Balances::<T>::insert(currency_id, account.clone(), inventory);
				}
			}
		}
	}

	/// If $\forall i, X_i = 0$ then no interest to match. Otherwise, call $R =
	/// \frac{\sum_i Y_i}{\sum_i X_i}$ $B_i$ has bought $min(X_i, X_i * R)$
	/// $S_i$ has sold $min(Y_i, Y_i / R)$
	fn match_interest(currency_id: T::AssetId) {
		// TODO: only run if needed
		// Reset inventory
		Inventory::<T>::remove_prefix(currency_id);
		let mut shorts: u128 = 0u128;
		let mut longs: u128 = 0u128;
		for balance in Balances::<T>::iter_prefix_values(currency_id) {
			let b = Self::balance_try_from_amount_abs(balance).unwrap(); // TODO Panics if error
			if balance < 0 {
				shorts += b;
			} else {
				longs += b;
			}
		}

		// If one of them is 0, nothing to match
		if shorts != 0 && longs != 0 {
			let ratio;
			let shorts_filled;
			if shorts < longs {
				ratio = Perquintill::from_rational_approximation(shorts, longs);
				shorts_filled = true;
			} else {
				ratio = Perquintill::from_rational_approximation(longs, shorts);
				shorts_filled = false;
			}
			for (account, balance) in Balances::<T>::iter_prefix(currency_id) {
				let mut amount: i128;
				if (balance < 0 && shorts_filled) || (balance >= 0 && !shorts_filled) {
					amount = balance;
				} else {
					let b = Self::balance_try_from_amount_abs(balance).unwrap(); // TODO Panics if error
					amount = Self::amount_try_from_balance(ratio.mul_floor(b)).unwrap(); // Should never fail given we know no overflow
					if balance < 0 {
						amount *= -1;
					}
				}
				Inventory::<T>::insert(currency_id, account, amount);
			}
		}
	}

	fn update_margin(currency_id: T::AssetId) {
		// TODO: handle no price better
		if let Some(new_price) = T::PriceSource::get_price(currency_id) {
			let p0 = Self::price0(currency_id).unwrap_or(new_price);
			let multiplier;
			let delta;
			if new_price > p0 {
				multiplier = 1;
				delta = new_price - p0;
			} else {
				multiplier = -1;
				delta = p0 - new_price;
			}
			Price0::<T>::insert(currency_id, new_price);
			if !delta.is_zero() {
				Margin::<T>::translate(|account, margin: u128| -> Option<u128> {
					let inventory = Inventory::<T>::get(currency_id, account);
					let update_inventory = delta.saturating_mul_int(inventory) * multiplier; //TODO is this a problem if it saturates?
																		 // TODO panic if this fails
					let mut amount = Self::amount_try_from_balance(margin).unwrap() + update_inventory;
					if amount < 0 {
						amount = 0; // No more margin left, account will be liquidated,
						 // TODO: update margin for everyone
					}
					Some(Self::balance_try_from_amount_abs(amount).unwrap()) //TODO
				});
			}
		}
	}

	fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	/// Gets the total balance of collateral in NativeCurrency
	pub fn total_collateral_balance() -> PalletBalanceOf<T> {
		T::Currency::total_balance(&Self::account_id())
	}

	/// Gets the treasury balance
	pub fn total_treasury_balance(account: &T::AccountId) -> PalletBalanceOf<T> {
		T::Currency::free_balance(account)
	}

	/// Convert `u128` to `i128`.
	fn amount_try_from_balance(b: u128) -> result::Result<i128, Error<T>> {
		TryInto::<i128>::try_into(b).map_err(|_| Error::<T>::AmountConvertFailed)
	}

	/// Convert the absolute value of `i128` to `u128`.
	fn balance_try_from_amount_abs(a: i128) -> result::Result<u128, Error<T>> {
		TryInto::<u128>::try_into(a.saturating_abs()).map_err(|_| Error::<T>::AmountConvertFailed)
	}

	/// Converts u128 to PalletBalanceOf
	fn balance_to_pallet_balance(b: u128) -> result::Result<PalletBalanceOf<T>, Error<T>> {
		TryInto::<PalletBalanceOf<T>>::try_into(b).map_err(|_| Error::<T>::AmountConvertFailed)
	}
}

#[cfg(feature = "std")]
impl<T: Config> GenesisConfig<T> {
	/// Direct implementation of `GenesisBuild::build_storage`.
	///
	/// Kept in order not to break dependency.
	pub fn build_storage(&self) -> Result<sp_runtime::Storage, String> {
		<Self as GenesisBuild<T>>::build_storage(self)
	}

	/// Direct implementation of `GenesisBuild::assimilate_storage`.
	///
	/// Kept in order not to break dependency.
	pub fn assimilate_storage(&self, storage: &mut sp_runtime::Storage) -> Result<(), String> {
		<Self as GenesisBuild<T>>::assimilate_storage(self, storage)
	}
}
