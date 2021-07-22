fn main() {}

// mod para;
// mod relay;

use frame_support::traits::GenesisBuild;
use sp_runtime::AccountId32;
use xcm_simulator::{decl_test_network, decl_test_parachain, decl_test_relay_chain};

use karura_runtime as karura;
use kusama_runtime as kusama;
use statemine_runtime as statemine;

pub const ALICE: AccountId32 = AccountId32::new([0u8; 32]);

decl_test_parachain! {
	pub struct Karura {
		Runtime = karura::Runtime,
		new_ext = karura_ext(2000),
	}
}

decl_test_relay_chain! {
	pub struct Kusama {
		Runtime = kusama::Runtime,
		XcmConfig = kusama::XcmConfig,
		new_ext = kusama_ext(),
	}
}

decl_test_parachain! {
	pub struct Statemine {
		Runtime = statemine::Runtime,
		new_ext = statemine_ext(1000),
	}
}

decl_test_network! {
	pub struct MockNet {
		relay_chain = Kusama,
		parachains = vec![
			(1000, Statemine),
			(2000, Karura),
		],
	}
}

pub const INITIAL_BALANCE: u128 = 100_000_000_000;

pub fn karura_ext(para_id: u32) -> sp_io::TestExternalities {
	use karura::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let parachain_info_config = parachain_info::GenesisConfig {
		parachain_id: para_id.into(),
	};

	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(&parachain_info_config, &mut t)
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(ALICE, INITIAL_BALANCE)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn kusama_ext() -> sp_io::TestExternalities {
	use kusama::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(ALICE, INITIAL_BALANCE)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	use polkadot_primitives::v1::{MAX_CODE_SIZE, MAX_POV_SIZE};
	// default parachains host configuration from polkadot's `chain_spec.rs`
	kusama::ParachainsConfigurationConfig {
		config: polkadot_runtime_parachains::configuration::HostConfiguration {
			validation_upgrade_frequency: 1u32,
			validation_upgrade_delay: 1,
			code_retention_period: 1200,
			max_code_size: MAX_CODE_SIZE,
			max_pov_size: MAX_POV_SIZE,
			max_head_data_size: 32 * 1024,
			group_rotation_frequency: 20,
			chain_availability_period: 4,
			thread_availability_period: 4,
			max_upward_queue_count: 8,
			max_upward_queue_size: 1024 * 1024,
			max_downward_message_size: 1024,
			// this is approximatelly 4ms.
			//
			// Same as `4 * frame_support::weights::WEIGHT_PER_MILLIS`. We don't bother with
			// an import since that's a made up number and should be replaced with a constant
			// obtained by benchmarking anyway.
			ump_service_total_weight: 4 * 1_000_000_000,
			max_upward_message_size: 1024 * 1024,
			max_upward_message_num_per_candidate: 5,
			hrmp_open_request_ttl: 5,
			hrmp_sender_deposit: 0,
			hrmp_recipient_deposit: 0,
			hrmp_channel_max_capacity: 8,
			hrmp_channel_max_total_size: 8 * 1024,
			hrmp_max_parachain_inbound_channels: 4,
			hrmp_max_parathread_inbound_channels: 4,
			hrmp_channel_max_message_size: 1024 * 1024,
			hrmp_max_parachain_outbound_channels: 4,
			hrmp_max_parathread_outbound_channels: 4,
			hrmp_max_message_num_per_candidate: 5,
			dispute_period: 6,
			no_show_slots: 2,
			n_delay_tranches: 25,
			needed_approvals: 2,
			relay_vrf_modulo_samples: 2,
			zeroth_delay_tranche_width: 0,
			..Default::default()
		},
	}.assimilate_storage(&mut t).unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn statemine_ext(para_id: u32) -> sp_io::TestExternalities {
	use statemine::{Runtime, System};

	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let parachain_info_config = parachain_info::GenesisConfig {
		parachain_id: para_id.into(),
	};

	<parachain_info::GenesisConfig as GenesisBuild<Runtime, _>>::assimilate_storage(&parachain_info_config, &mut t)
		.unwrap();

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(ALICE, INITIAL_BALANCE)],
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub type KaruraXcm = pallet_xcm::Pallet<karura::Runtime>;
pub type KusamaXcm = pallet_xcm::Pallet<kusama::Runtime>;
pub type StatemineXcm = pallet_xcm::Pallet<statemine::Runtime>;

#[cfg(test)]
mod tests {
	use super::*;

	use codec::Encode;
	use frame_support::{assert_ok, dispatch::GetDispatchInfo};
	use xcm::v0::{
		Junction::{self, Parachain, Parent},
		MultiAsset::*,
		MultiLocation::*,
		NetworkId, OriginKind,
		Xcm::*,
	};
	use xcm_simulator::TestExt;

	fn print_events<T: frame_system::Config>(context: &str) {
		println!("------ {:?} events ------", context);
		frame_system::Pallet::<T>::events().iter().for_each(|r| {
			println!("{:?}", r.event);
		});
	}

	#[test]
	fn reserve_transfer() {
		MockNet::reset();

		Kusama::execute_with(|| {
			assert_ok!(KusamaXcm::reserve_transfer_assets(
				kusama::Origin::signed(ALICE),
				X1(Parachain(1000)),
				X1(Junction::AccountId32 {
					network: NetworkId::Any,
					id: ALICE.into(),
				}),
				vec![ConcreteFungible { id: Null, amount: 123 }],
				123,
			));

			print_events::<kusama::Runtime>("Kusama");
		});

		Statemine::execute_with(|| {
			print_events::<statemine::Runtime>("Statemine");

			// free execution, full amount received
			assert_eq!(
				pallet_balances::Pallet::<statemine::Runtime>::free_balance(&ALICE),
				INITIAL_BALANCE + 123
			);
		});
	}

	#[test]
	fn dmp() {
		MockNet::reset();

		let remark = statemine::Call::System(frame_system::Call::<statemine::Runtime>::remark_with_event(vec![1, 2, 3]));

		Kusama::execute_with(|| {
			assert_ok!(KusamaXcm::send_xcm(
				Null,
				X1(Parachain(1000)),
				Transact {
					origin_type: OriginKind::SovereignAccount,
					require_weight_at_most: remark.get_dispatch_info().weight,
					call: remark.encode().into(),
				},
			));

			print_events::<kusama::Runtime>("Kusama");
		});

		Statemine::execute_with(|| {
			print_events::<statemine::Runtime>("Statemine");

			panic!();
		});
	}

	#[test]
	fn ump() {
		MockNet::reset();

		let remark = kusama::Call::System(frame_system::Call::<kusama::Runtime>::remark_with_event(vec![1, 2, 3]));
		Statemine::execute_with(|| {
			assert_ok!(StatemineXcm::send_xcm(
				Null,
				X1(Parent),
				Transact {
					origin_type: OriginKind::SovereignAccount,
					require_weight_at_most: INITIAL_BALANCE as u64,
					call: remark.encode().into(),
				},
			));
		});

		Kusama::execute_with(|| {
			print_events::<kusama::Runtime>("RelayChain");
		});
	}

	#[test]
	fn xcmp() {
		MockNet::reset();

		let remark = karura::Call::System(frame_system::Call::<karura::Runtime>::remark_with_event(vec![1, 2, 3]));
		Statemine::execute_with(|| {
			assert_ok!(StatemineXcm::send_xcm(
				Null,
				X2(Parent, Parachain(2000)),
				Transact {
					origin_type: OriginKind::SovereignAccount,
					require_weight_at_most: INITIAL_BALANCE as u64,
					call: remark.encode().into(),
				},
			));
		});

		Karura::execute_with(|| {
			print_events::<karura::Runtime>("Karura");
		});
	}
}
