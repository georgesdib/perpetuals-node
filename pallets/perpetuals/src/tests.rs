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

//! Unit tests for perpetualasset module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, error::BadOrigin};
use mock::{
	Event, ExtBuilder, MockPriceSource, Origin, PerpetualsExchange, Runtime, System, Treasury, ALICE, BOB, CHARLIE,
	GEORGES, CurrencyId::{DOT, LDOT}
};

fn last_event() -> Event {
	System::events().last().unwrap().event.clone()
}

fn balance_of_treasury() -> u128 {
	PerpetualsExchange::total_treasury_balance(&Treasury::account_id())
		.try_into()
		.unwrap()
}

#[test]
fn setup_parameters_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();

		MockPriceSource::set_price(DOT, Some(10u128.into()));
		PerpetualsExchange::on_initialize(1);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 201i128));

		assert_noop!(
			PerpetualsExchange::set_global_params(
				Origin::signed(BOB),
				DOT,
				Change::NewValue(Permill::from_percent(30)),
				Change::NoChange,
				Change::NoChange
			),
			BadOrigin
		);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 201i128));

		assert_ok!(PerpetualsExchange::set_global_params(
			Origin::signed(ALICE),
			DOT,
			Change::NewValue(Permill::from_percent(30)),
			Change::NoChange,
			Change::NoChange
		));

		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 201i128),
			crate::Error::<Runtime>::NotEnoughIM
		);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 401i128));

		assert_ok!(PerpetualsExchange::set_global_params(
			Origin::signed(ALICE),
			DOT,
			Change::NoChange,
			Change::NoChange,
			Change::NewValue(Permill::from_percent(2))
		));

		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 410i128),
			crate::Error::<Runtime>::NotEnoughIM
		);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 420i128));

		assert_noop!(
			PerpetualsExchange::set_global_params(
				Origin::signed(ALICE),
				DOT,
				Change::NewValue(Permill::from_percent(1)),
				Change::NoChange,
				Change::NoChange
			),
			crate::Error::<Runtime>::BadIMParameters
		);

		assert_noop!(
			PerpetualsExchange::set_global_params(
				Origin::signed(ALICE),
				DOT,
				Change::NoChange,
				Change::NewValue(Permill::from_percent(30)),
				Change::NoChange
			),
			crate::Error::<Runtime>::BadIMParameters
		);

		assert_ok!(PerpetualsExchange::set_global_params(
			Origin::signed(ALICE),
			DOT,
			Change::NoChange,
			Change::NewValue(Permill::from_percent(29)),
			Change::NoChange
		));

		MockPriceSource::set_price(DOT, Some(11u128.into()));
		PerpetualsExchange::on_initialize(2);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), 0i128);
	});
}

#[test]
fn top_up_collateral_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		PerpetualsExchange::on_initialize(1);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 21i128));

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 0i128, 10i128));

		assert_eq!(PerpetualsExchange::total_collateral_balance(), 30u128);
		assert_eq!(balance_of_treasury(), 1u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 30u128);

		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 0i128, 2_000_000_000_000_000_000i128,),
			pallet_balances::Error::<Runtime>::InsufficientBalance,
		);

		assert_eq!(PerpetualsExchange::total_collateral_balance(), 30u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 30u128);
	});
}

#[test]
fn mint_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_noop!(
			PerpetualsExchange::mint(
				Origin::signed(ALICE),
				DOT,
				2_000_000_000_000_000_000i128,
				2_000_000_000_000_000_000i128
			),
			pallet_balances::Error::<Runtime>::InsufficientBalance
		);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 0u128);

		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 10i128, 1i128),
			crate::Error::<Runtime>::NotEnoughIM
		);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 21i128));

		assert_eq!(
			last_event(),
			Event::ecosystem_perpetuals_exchange(crate::Event::BalanceUpdated(ALICE, 100i128))
		);

		assert_eq!(PerpetualsExchange::total_collateral_balance(), 20u128);
		assert_eq!(
			PerpetualsExchange::total_treasury_balance(&ALICE),
			999_999_999_999_999_979u128
		);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 20u128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, -10i128, 0i128)); // Removes balance so no IM needed
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 19u128); // consumes 1 in fees
		assert_eq!(PerpetualsExchange::margin(&ALICE), 19u128); // also out of ALICE's margin
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 90i128);

		// Only 10 unit added, so 2.02 IM needed but margin is down 1, so top up by 4
		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 20i128, 3i128),
			crate::Error::<Runtime>::NotEnoughIM
		);
		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 20i128, 4i128));
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 22u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 22u128);

		// balance is now -200, so 40 IM needed, 22 already there, so need 18
		// plus fees of 0.31 rounded up to 1 so 19 needed
		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(ALICE), DOT, -310i128, 18i128),
			crate::Error::<Runtime>::NotEnoughIM
		);
		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, -310i128, 19i128));
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 40u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 40u128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 21i128));
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 60u128);
		assert_eq!(balance_of_treasury(), 5u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 20u128);
	});
}

#[test]
fn match_interest_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();

		PerpetualsExchange::on_initialize(1);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 21i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 21i128));

		PerpetualsExchange::on_initialize(2);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, -50i128, 1i128));
		PerpetualsExchange::on_initialize(3);
		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 50i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -50i128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(CHARLIE), DOT, 100i128, 21i128));
		PerpetualsExchange::on_initialize(4);
		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 33i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 66i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(GEORGES), DOT, -100i128, 21i128));
		PerpetualsExchange::on_initialize(4);
		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 50i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -75i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -75i128);
	});
}

#[test]
fn redeem_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		PerpetualsExchange::on_initialize(1);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 21i128));
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 20u128);
		assert_eq!(balance_of_treasury(), 1u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 20u128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);

		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 0i128, -1i128),
			crate::Error::<Runtime>::NotEnoughIM
		);

		assert_eq!(PerpetualsExchange::total_collateral_balance(), 20u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 20u128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 60i128));
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 79u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 79u128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 200i128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, -10i128));
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 68u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 68u128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 300i128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 13i128));
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 80u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 80u128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 400i128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 0i128, 10i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 11i128));

		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 10i128),
			crate::Error::<Runtime>::NotEnoughIM
		);
	});
}

#[test]
fn liquidate_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		PerpetualsExchange::on_initialize(1);
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 21i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 21i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(CHARLIE), DOT, 50i128, 20i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(GEORGES), DOT, -10i128, 20i128));
		PerpetualsExchange::on_initialize(2);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 73i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 36i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -10i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 50i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -10i128);

		MockPriceSource::set_price(DOT, Some(2u128.into()));
		PerpetualsExchange::update_margin(DOT);

		assert_eq!(PerpetualsExchange::total_collateral_balance(), 78u128);
		assert_eq!(balance_of_treasury(), 4u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 93u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 0u128);
		assert_eq!(PerpetualsExchange::margin(&CHARLIE), 55u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 9u128);

		PerpetualsExchange::liquidate();

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 73i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 36i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -10i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 50i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -10i128);

		PerpetualsExchange::match_interest(DOT);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 6i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 3i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -10i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 50i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -10i128);
	});
}

#[test]
fn liquidate_works_0_price() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		MockPriceSource::set_price(DOT, Some(20u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 402i128));
		PerpetualsExchange::match_interest(DOT);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);

		// Price goes to 0, ALICE should be fully liquidated
		MockPriceSource::set_price(DOT, Some(0u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::liquidate();
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 800u128);
		assert_eq!(balance_of_treasury(), 4u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 0u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 2400u128);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
	});
}

#[test]
fn liquidate_works_complex_2() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		MockPriceSource::set_price(DOT, Some(20u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(CHARLIE),
			DOT,
			100i128,
			4000i128
		));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(GEORGES),
			DOT,
			100i128,
			4000i128
		));
		PerpetualsExchange::match_interest(DOT);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 33i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 33i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), 33i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), 100i128);

		// liquidate all of Alice's open interest
		MockPriceSource::set_price(DOT, Some(9u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::liquidate();
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 8796u128);
		assert_eq!(balance_of_treasury(), 8u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 37u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 1500u128);
		assert_eq!(PerpetualsExchange::margin(&CHARLIE), 3635u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 3635u128);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 33i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 33i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), 33i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 33i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), 100i128);

		PerpetualsExchange::match_interest(DOT);
		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 14i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 42i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), 42i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 33);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), 100i128);
	});
}

#[test]
fn liquidate_works_complex() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		MockPriceSource::set_price(DOT, Some(20u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 450i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(CHARLIE), DOT, 50i128, 400i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(GEORGES), DOT, -10i128, 400i128));
		PerpetualsExchange::match_interest(DOT);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 73i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 36i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -10i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 50i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -10i128);

		// No liquidation
		MockPriceSource::set_price(DOT, Some(19u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::liquidate();
		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 73i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 36i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -10i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 50i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -10i128);

		// liquidate Alice's open interest only
		MockPriceSource::set_price(DOT, Some(16u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::liquidate();
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 1646u128);
		assert_eq!(balance_of_treasury(), 6u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 156u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 800u128);
		assert_eq!(PerpetualsExchange::margin(&CHARLIE), 255u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 439u128);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 73i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 36i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -10i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 73i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 50i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -10i128);

		PerpetualsExchange::match_interest(DOT);
		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 65i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 44i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -10i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 73i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 50i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -10i128);
	});
}

#[test]
fn update_balances_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		PerpetualsExchange::on_initialize(1);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 21i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 21i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(CHARLIE), DOT, 50i128, 20i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(GEORGES), DOT, -10i128, 20i128));
		PerpetualsExchange::on_initialize(2);

		MockPriceSource::set_price(DOT, Some(2u128.into()));
		PerpetualsExchange::update_margin(DOT);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 73i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 36i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -10i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 50i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -10i128);

		assert_eq!(PerpetualsExchange::total_collateral_balance(), 78u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 93u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 0u128);
		assert_eq!(PerpetualsExchange::margin(&CHARLIE), 55u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 9u128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, 0i128, 120i128));
		assert_eq!(PerpetualsExchange::margin(&BOB), 120u128);
	});
}

#[test]
fn claim_collateral() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		MockPriceSource::set_price(DOT, Some(20u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(CHARLIE), DOT, 100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(GEORGES),
			DOT,
			-100i128,
			402i128
		));
		PerpetualsExchange::match_interest(DOT);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -100i128);

		// Price goes to 0, ALICE and CHARLIE should be fully liquidated
		MockPriceSource::set_price(DOT, Some(0u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::liquidate();
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 1600u128);
		assert_eq!(balance_of_treasury(), 8u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 0u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 2400u128);
		assert_eq!(PerpetualsExchange::margin(&CHARLIE), 0u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 2400u128);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -100i128);

		// Claim back collateral
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, 0i128, -1600i128));
		assert_noop!(
			PerpetualsExchange::mint(Origin::signed(GEORGES), DOT, 0i128, -1i128),
			pallet_balances::Error::<Runtime>::InsufficientBalance,
		);
	});
}

#[test]
fn claim_collateral_2() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		MockPriceSource::set_price(DOT, Some(20u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, -100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(CHARLIE), DOT, 100i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(GEORGES),
			DOT,
			-100i128,
			402i128
		));
		PerpetualsExchange::match_interest(DOT);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -100i128);

		// Price goes to 0, ALICE and CHARLIE should be fully liquidated
		MockPriceSource::set_price(DOT, Some(0u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::liquidate();
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 1600u128);
		assert_eq!(balance_of_treasury(), 8u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 0u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 2400u128);
		assert_eq!(PerpetualsExchange::margin(&CHARLIE), 0u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 2400u128);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), -100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -100i128);

		// Claim back collateral
		assert_ok!(PerpetualsExchange::mint(Origin::signed(BOB), DOT, 100i128, -1600i128));
		PerpetualsExchange::match_interest(DOT);

		MockPriceSource::set_price(DOT, Some(10u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::liquidate();

		assert_eq!(PerpetualsExchange::total_collateral_balance(), 0u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 0u128);
		assert_eq!(PerpetualsExchange::margin(&BOB), 800u128);
		assert_eq!(PerpetualsExchange::margin(&CHARLIE), 0u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 2400u128);

		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &BOB), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &CHARLIE), 0i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &BOB), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &CHARLIE), 0i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -100i128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 201i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(GEORGES), DOT, 0i128, -200i128));
	});
}

#[test]
fn multiple_assets_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		MockPriceSource::set_price(DOT, Some(20u128.into()));
		MockPriceSource::set_price(LDOT, Some(20u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(ALICE),
			DOT,
			1000i128,
			10020i128
		));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(ALICE),
			LDOT,
			-1000i128,
			400i128
		));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(GEORGES),
			DOT,
			-1000i128,
			4020i128
		));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(GEORGES),
			LDOT,
			1000i128,
			6400i128
		));

		assert_eq!(PerpetualsExchange::total_collateral_balance(), 20000u128);
		assert_eq!(balance_of_treasury(), 840u128);
		assert_eq!(PerpetualsExchange::margin(&ALICE), 10000u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 10000u128);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, -100i128, 0i128)); // Removes balance so no IM needed
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 19998u128); // consumes 2 in fees
		assert_eq!(PerpetualsExchange::margin(&ALICE), 9998u128); // also out of ALICE's margin
		assert_ok!(PerpetualsExchange::mint(Origin::signed(GEORGES), LDOT, -100i128, 0i128)); // Removes balance so no IM needed
		assert_eq!(PerpetualsExchange::total_collateral_balance(), 19958u128); // consumes 40 in fees
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 9960u128); // also out of GEORGES's margin
		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 900i128);
		assert_eq!(PerpetualsExchange::balances(LDOT, &ALICE), -1000i128);
		assert_eq!(PerpetualsExchange::balances(LDOT, &GEORGES), 900i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -1000i128);

		PerpetualsExchange::match_interest(DOT);
		PerpetualsExchange::match_interest(LDOT);

		assert_eq!(PerpetualsExchange::inventory(DOT, &GEORGES), -900i128);
		assert_eq!(PerpetualsExchange::inventory(DOT, &ALICE), 900i128);
		assert_eq!(PerpetualsExchange::inventory(LDOT, &GEORGES), 900i128);
		assert_eq!(PerpetualsExchange::inventory(LDOT, &ALICE), -900i128);

		MockPriceSource::set_price(DOT, Some(12u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_eq!(PerpetualsExchange::margin(&ALICE), 2798u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 17160u128);
		PerpetualsExchange::liquidate();

		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::balances(LDOT, &ALICE), 0i128);
		assert_eq!(PerpetualsExchange::balances(LDOT, &GEORGES), 900i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -1000i128);
	});
}

#[test]
fn multiple_assets_works_2() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		MockPriceSource::set_price(DOT, Some(20u128.into()));
		MockPriceSource::set_price(LDOT, Some(20u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 900i128, 10020i128));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(ALICE),
			LDOT,
			-1000i128,
			400i128
		));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(GEORGES),
			DOT,
			-1000i128,
			4020i128
		));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(GEORGES),
			LDOT,
			900i128,
			6400i128
		));

		PerpetualsExchange::match_interest(DOT);
		PerpetualsExchange::match_interest(LDOT);

		MockPriceSource::set_price(DOT, Some(12u128.into()));
		MockPriceSource::set_price(LDOT, Some(19u128.into())); // offsets DOT move
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_eq!(PerpetualsExchange::margin(&ALICE), 3702u128);
		assert_eq!(PerpetualsExchange::margin(&GEORGES), 16340u128);
		PerpetualsExchange::liquidate();

		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 900i128);
		assert_eq!(PerpetualsExchange::balances(LDOT, &ALICE), -1000i128);
		assert_eq!(PerpetualsExchange::balances(LDOT, &GEORGES), 900i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -1000i128);
	});
}

#[test]
fn multiple_assets_works_3() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		System::reset_events();
		MockPriceSource::set_price(DOT, Some(20u128.into()));
		MockPriceSource::set_price(LDOT, Some(20u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);

		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), DOT, 100i128, 432i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(ALICE), LDOT, -100i128, 640i128));
		assert_ok!(PerpetualsExchange::mint(Origin::signed(GEORGES), DOT, -90i128, 402i128));
		assert_ok!(PerpetualsExchange::mint(
			Origin::signed(GEORGES),
			LDOT,
			100i128,
			640i128
		));

		PerpetualsExchange::match_interest(DOT);
		PerpetualsExchange::match_interest(LDOT);

		MockPriceSource::set_price(DOT, Some(12u128.into()));
		PerpetualsExchange::update_margin(DOT);
		PerpetualsExchange::update_margin(LDOT);
		PerpetualsExchange::liquidate();

		assert_eq!(PerpetualsExchange::balances(DOT, &ALICE), 90i128);
		assert_eq!(PerpetualsExchange::balances(LDOT, &ALICE), -100i128);
		assert_eq!(PerpetualsExchange::balances(LDOT, &GEORGES), 100i128);
		assert_eq!(PerpetualsExchange::balances(DOT, &GEORGES), -90i128);
	});
}
