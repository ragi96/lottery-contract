#![cfg_attr(not(feature = "std"), no_std)]

use ink_env::Environment;
use ink_lang as ink;

#[ink::chain_extension]
pub trait FetchRandom {
    type ErrorCode = RandomReadErr;

    #[ink(extension = 1101, returns_result = false)]
    fn fetch_random() -> [u8; 32];
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum RandomReadErr {
    FailGetRandomSource,
}

impl ink_env::chain_extension::FromStatusCode for RandomReadErr {
    fn from_status_code(status_code: u32) -> Result<(), Self> {
        match status_code {
            0 => Ok(()),
            1 => Err(Self::FailGetRandomSource),
            _ => panic!("encountered unknown status code"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CustomEnvironment {}

impl Environment for CustomEnvironment {
    const MAX_EVENT_TOPICS: usize = <ink_env::DefaultEnvironment as Environment>::MAX_EVENT_TOPICS;

    type AccountId = <ink_env::DefaultEnvironment as Environment>::AccountId;
    type Balance = <ink_env::DefaultEnvironment as Environment>::Balance;
    type Hash = <ink_env::DefaultEnvironment as Environment>::Hash;
    type BlockNumber = <ink_env::DefaultEnvironment as Environment>::BlockNumber;
    type Timestamp = <ink_env::DefaultEnvironment as Environment>::Timestamp;

    type ChainExtension = FetchRandom;
}

#[ink::contract(env = crate::CustomEnvironment)]
mod lottery {
    pub type Result<T> = core::result::Result<T, Error>;
    use ink_storage::{traits::SpreadAllocate, Mapping};

    /// Emitted whenever a new bet is being registered.
    #[ink(event)]
    pub struct RegisterBet {
        #[ink(topic)]
        bet: [u8; 32],
        #[ink(topic)]
        from: AccountId,
    }

    impl Default for Lottery {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    #[derive(SpreadAllocate)]
    pub struct Lottery {
        round: u8,
        ticket_and_address: Mapping<([u8; 32], u8), [AccountId; 8]>,
        def_address: [AccountId; 8],
        last_drawing: BlockNumber,
        jackpot: Balance,
        winner_bet: [u8; 32],
        last_pot_per_bet: Balance,
    }

    /// Errors that can occur upon calling this contract.
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(::scale_info::TypeInfo))]
    pub enum Error {
        BetAlreadyExists,
        TicketCosts,
    }

    const BET_PRICE: Balance = 1_000_000;
    const BLOCKS_PER_ROUND: u32 = 1000;

    impl Lottery {
        #[ink(constructor)]
        pub fn new() -> Self {
            ink_lang::utils::initialize_contract(Self::new_init)
        }

        fn new_init(&mut self) {
            let bet = [0; 32];
            self.round = 0;
            self.ticket_and_address
                .insert((bet, 0), &[AccountId::default(); 8]);
            self.jackpot = 0;
            self.last_drawing = BlockNumber::default();
            self.def_address = [AccountId::default(); 8];
            self.winner_bet = [0; 32];
            self.last_pot_per_bet = 0;
        }

        /// Register specific bet with caller as owner.
        #[ink(message, payable)]
        pub fn register_bet(&mut self, bet: [u8; 32]) -> Result<()> {
            let trans_bal = self.env().transferred_value();
            assert!(trans_bal == BET_PRICE, "insufficient funds!");
            self.jackpot += trans_bal;

            let caller = self.env().caller();

            if self.ticket_and_address.contains((bet, self.round)) {
                let mut betters = self.ticket_and_address.get((bet, self.round)).unwrap();
                assert!(betters[7] == AccountId::default(), "bet sold out!");
                for i in 0..betters.len() {
                    if betters[i] == AccountId::default() {
                        betters[i] = caller;
                        self.ticket_and_address.insert((bet, self.round), &betters);
                        self.env().emit_event(RegisterBet { bet, from: caller });
                        break;
                    }
                }
            } else {
                let mut betters: [AccountId; 8] = [AccountId::default(); 8];
                betters[0] = caller;
                self.ticket_and_address.insert((bet, self.round), &betters);
                self.env().emit_event(RegisterBet { bet, from: caller });
            }

            let now = self.env().block_number();
            if now - self.last_drawing >= BLOCKS_PER_ROUND && now != 0 {
                self.draw();
            }
            Ok(())
        }

        fn draw(&mut self) {
            let rand_output = self.env().extension().fetch_random().unwrap();

            let mut win_bet: [u8; 32] = [0; 32];
            win_bet[0] = rand_output[0];
            win_bet[1] = rand_output[1];
            win_bet[2] = rand_output[2];
            self.winner_bet = win_bet;
            self.last_drawing = self.env().block_number();

            let winners = self.get_winner_or_default();
            if winners != self.def_address {
                self.transfer_to_winners(winners);
            }
        }

        fn transfer_to_winners(&mut self, winners: [AccountId; 8]) {
            if self.jackpot > 0 {
                let number_of_winners = self.get_number_of_winner(winners);
                let jackpot_balance: Balance = self.jackpot / 8;
                if number_of_winners > 0 {
                    let jack_multiplication = 8 / number_of_winners;
                    self.last_pot_per_bet = jackpot_balance * u128::from(jack_multiplication);
                    for winner in 0..number_of_winners {
                        let winner_id = winners[usize::from(winner)];
                        if winner_id != AccountId::default() {
                            let _res = self.env().transfer(winner_id, self.last_pot_per_bet);
                        }
                    }
                    self.reset_game()
                }
            }
        }

        fn get_number_of_winner(&mut self, winners: [AccountId; 8]) -> u8 {
            let mut count = 0;
            for i in 0..7 {
                if winners[i] != AccountId::default() {
                    count += 1;
                }
            }
            count
        }

        fn reset_game(&mut self) {
            self.round += 1;
            self.jackpot = 0;
        }

        pub fn get_winner_or_default(&self) -> [AccountId; 8] {
            self.ticket_and_address
                .get((self.winner_bet, self.round))
                .unwrap_or(self.def_address)
        }

        /// Simply returns the winner bet
        #[ink(message)]
        pub fn get(&self) -> [u8; 32] {
            self.winner_bet
        }
        // Get all accounts per bet
        #[ink(message)]
        pub fn get_accounts_by_bet(&mut self, bet_hash: [u8; 32]) -> [AccountId; 8] {
            self.ticket_and_address
                .get((bet_hash, self.round))
                .unwrap_or(self.def_address)
        }

        #[ink(message)]
        pub fn get_last_winner_or_default(&self) -> [AccountId; 8] {
            self.ticket_and_address
                .get((self.winner_bet, self.round - 1))
                .unwrap_or(self.def_address)
        }

        /// Simply returns the block of the last drawing
        #[ink(message)]
        pub fn get_last_drawing(&mut self) -> BlockNumber {
            self.last_drawing
        }

        /// Simply returns the block of the last drawing
        #[ink(message)]
        pub fn get_next_drawing(&mut self) -> BlockNumber {
            self.last_drawing + BLOCKS_PER_ROUND
        }
        /// returns the price per winner of the last round
        #[ink(message)]
        pub fn get_last_pot_per_bet(&self) -> Balance {
            self.last_pot_per_bet
        }

        /// returns the block of the last drawing
        #[ink(message)]
        pub fn get_jackpot(&self) -> Balance {
            self.jackpot
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        use crate::CustomEnvironment;
        /// Imports `ink_lang` so we can use `#[ink::test]`.
        use ink_lang as ink;

        fn default_accounts() -> ink_env::test::DefaultAccounts<CustomEnvironment> {
            ink_env::test::default_accounts::<Environment>()
        }

        fn set_next_caller(caller: AccountId) {
            ink_env::test::set_caller::<Environment>(caller);
            ink_env::test::set_account_balance::<Environment>(caller, BET_PRICE * 255);
            ink_env::test::set_value_transferred::<Environment>(BET_PRICE);
        }

        fn set_next_caller_too_low_balance(caller: AccountId) {
            let too_low_price = BET_PRICE - 1;
            ink_env::test::set_caller::<Environment>(caller);
            ink_env::test::set_value_transferred::<Environment>(too_low_price);
        }

        fn set_next_caller_too_high_balance(caller: AccountId) {
            let too_high_price = BET_PRICE + 1;
            ink_env::test::set_caller::<Environment>(caller);
            ink_env::test::set_value_transferred::<Environment>(too_high_price);
        }

        fn register_number_of_same_bets(
            num_registers: u8,
            bet: [u8; 32],
            mut contract: Lottery,
        ) -> Lottery {
            for _i in 0..num_registers {
                assert_eq!(contract.register_bet(bet), Ok(()));
            }
            contract
        }

        fn get_win_bet() -> [u8; 32] {
            let mut bet_arr = [0; 32];
            bet_arr[0] = 21;
            bet_arr[1] = 236;
            bet_arr[2] = 123;

            return bet_arr;
        }

        fn setup_jackpot(numb_bets: u8) -> Lottery {
            use_random_chain_extension();
            let mut contract = Lottery::new();
            let default_accounts = default_accounts();
            let mut bet_arr = [0; 32];
            set_next_caller(default_accounts.bob);
            for i in 0..numb_bets {
                bet_arr[0] = i;
                bet_arr[1] = i;
                bet_arr[2] = i;
                assert_eq!(
                    ink_env::pay_with_call!(contract.register_bet(bet_arr), BET_PRICE),
                    Ok(())
                );
            }
            return contract;
        }

        fn register_number_of_win_bets(num_registers: u8, mut contract: Lottery) -> Lottery {
            let default_accounts = default_accounts();

            for i in 0..num_registers {
                let acc = i % 3;
                if acc == 0 {
                    set_next_caller(default_accounts.eve);
                } else if acc == 1 {
                    set_next_caller(default_accounts.alice);
                } else {
                    set_next_caller(default_accounts.bob);
                }
                assert_eq!(
                    ink_env::pay_with_call!(contract.register_bet(get_win_bet()), BET_PRICE),
                    Ok(())
                );
            }
            contract
        }

        fn use_random_chain_extension() {
            struct MockedExtension;
            impl ink_env::test::ChainExtension for MockedExtension {
                fn func_id(&self) -> u32 {
                    1101
                }
                fn call(&mut self, _input: &[u8], output: &mut Vec<u8>) -> u32 {
                    let ret = get_win_bet();
                    scale::Encode::encode_to(&ret, output);
                    0
                }
            }
            ink_env::test::register_chain_extension(MockedExtension);
        }

        fn advance_blocks(numb_blocks: u32) {
            for _i in 0..numb_blocks {
                ink_env::test::advance_block::<Environment>();
            }
        }

        #[ink::test]
        fn default_works() {
            use_random_chain_extension();
            let mut contract = Lottery::new();
            let init = contract.get();
            contract.draw();
            let second = contract.get();
            assert_ne!(init, second);
        }

        /// We test if the default constructor does its job.
        #[ink::test]
        fn register_works() {
            let default_accounts = default_accounts();
            let mut bet = [0; 32];
            bet[0] = 1;
            bet[1] = 2;
            bet[2] = 3;
            set_next_caller(default_accounts.alice);
            let mut contract = Lottery::new();

            assert_eq!(contract.register_bet(bet), Ok(()));
        }

        #[ink::test]
        #[should_panic(expected = "insufficient funds!")]
        fn transferred_balance_too_low() {
            let default_accounts = default_accounts();
            set_next_caller_too_low_balance(default_accounts.alice);
            let bet_arr = [0; 32];
            let mut contract = Lottery::new();
            assert_eq!(contract.register_bet(bet_arr), Err(Error::TicketCosts));
        }

        #[ink::test]
        #[should_panic(expected = "insufficient funds!")]
        fn transferred_balance_too_high() {
            let default_accounts = default_accounts();
            set_next_caller_too_high_balance(default_accounts.alice);
            let bet_arr = [0; 32];
            let mut contract = Lottery::new();
            assert_eq!(contract.register_bet(bet_arr), Err(Error::TicketCosts));
        }

        #[ink::test]
        fn get_accounts_by_bet_init_should_be_default() {
            let default_accounts = default_accounts();
            set_next_caller(default_accounts.alice);
            let mut contract = Lottery::new();
            assert_eq!(
                contract.get_accounts_by_bet([0; 32]),
                [AccountId::default(); 8]
            );
        }

        #[ink::test]
        fn last_winner_bet_changed_after_first_draw() {
            let mut contract = setup_jackpot(8);
            let default_accounts = default_accounts();
            let bet_arr = [0; 32];
            let old_win_bet = contract.get();
            advance_blocks(BLOCKS_PER_ROUND);
            set_next_caller(default_accounts.bob);
            assert_eq!(contract.register_bet(bet_arr), Ok(()));
            assert_ne!(get_win_bet(), old_win_bet)
        }

        #[ink::test]
        fn last_drawing_changed_after_first_draw() {
            let mut contract = setup_jackpot(8);
            let default_accounts = default_accounts();
            let bet_arr = [0; 32];
            let old_last_drawing = contract.get_last_drawing();
            advance_blocks(BLOCKS_PER_ROUND);
            set_next_caller(default_accounts.bob);
            assert_eq!(contract.register_bet(bet_arr), Ok(()));
            assert_ne!(old_last_drawing, contract.get_last_drawing());
        }

        #[ink::test]
        fn get_accounts_by_bet_should_be_alice() {
            let default_accounts = default_accounts();
            set_next_caller(default_accounts.alice);
            let mut contract = Lottery::new();

            let mut bet = [0; 32];
            bet[0] = 1;
            bet[1] = 2;
            bet[2] = 3;
            contract = register_number_of_same_bets(1, bet, contract);
            let mut winner_acc = [AccountId::default(); 8];
            winner_acc[0] = default_accounts.alice;
            assert_eq!(contract.get_accounts_by_bet(bet), winner_acc);
        }

        #[ink::test]
        fn get_accounts_by_bet_should_be_two_alice() {
            let default_accounts = default_accounts();
            let mut bet = [0; 32];
            bet[0] = 1;
            bet[1] = 2;
            bet[2] = 3;
            let mut winner_acc = [AccountId::default(); 8];
            winner_acc[0] = default_accounts.alice;
            winner_acc[1] = default_accounts.alice;

            set_next_caller(default_accounts.alice);
            let mut contract = Lottery::new();
            contract = register_number_of_same_bets(2, bet, contract);

            assert_eq!(contract.get_accounts_by_bet(bet), winner_acc);
        }

        #[ink::test]
        fn bet_filled_dont_panic() {
            let default_accounts = default_accounts();
            let mut bet_arr = [0; 32];
            bet_arr[0] = 100;
            bet_arr[1] = 100;
            bet_arr[2] = 100;

            set_next_caller(default_accounts.bob);
            let mut contract = Lottery::new();

            // 8 is fine
            for _i in 0..8 {
                assert_eq!(contract.register_bet(bet_arr), Ok(()));
            }
        }

        #[ink::test]
        #[should_panic(expected = "bet sold out!")]
        fn bet_sold_out() {
            let default_accounts = default_accounts();
            let mut bet_arr = [0; 32];
            bet_arr[0] = 99;
            bet_arr[1] = 99;
            bet_arr[2] = 99;

            set_next_caller(default_accounts.bob);
            let mut contract = Lottery::new();

            for _i in 0..9 {
                assert_eq!(contract.register_bet(bet_arr), Ok(()));
            }
        }

        #[ink::test]
        fn get_last_drawing_init_should_be_zero() {
            let default_accounts = default_accounts();
            set_next_caller(default_accounts.alice);
            let mut contract = Lottery::new();
            assert_eq!(contract.get_last_drawing(), 0);
        }

        #[ink::test]
        fn test_draw_sets_last_drawing_to_actual_block() {
            use_random_chain_extension();
            let default_accounts = default_accounts();
            set_next_caller(default_accounts.bob);
            let mut contract = Lottery::new();
            advance_blocks(10);
            contract.draw();
            assert_eq!(contract.get_last_drawing(), 10);
        }

        #[ink::test]
        fn test_last_winner_bet_init() {
            let default_accounts = default_accounts();
            set_next_caller(default_accounts.bob);
            let contract = Lottery::new();
            assert_eq!(contract.get(), [0; 32]);
        }

        #[ink::test]
        fn test_last_winner_bet_not_init_after_draw() {
            use_random_chain_extension();
            let default_accounts = default_accounts();
            set_next_caller(default_accounts.bob);
            let mut contract = Lottery::new();
            contract.draw();
            assert_ne!(contract.get(), [0; 32]);
        }

        #[ink::test]
        fn test_last_winner_bet_is_win_bet() {
            use_random_chain_extension();
            let default_accounts = default_accounts();
            set_next_caller(default_accounts.bob);
            let mut contract = Lottery::new();
            contract.draw();
            assert_eq!(contract.get(), get_win_bet());
        }

        #[ink::test]
        fn winner_is_alice() {
            let default_accounts = default_accounts();
            use_random_chain_extension();
            set_next_caller(default_accounts.alice);
            let mut contract = Lottery::new();

            assert_eq!(contract.register_bet(get_win_bet()), Ok(()));
            advance_blocks(10);
            contract.draw();

            let mut winners: [AccountId; 8] = [AccountId::default(); 8];
            winners[0] = default_accounts.alice;
            assert_eq!(winners, contract.get_last_winner_or_default())
        }

        #[ink::test]
        fn winner_is_bob_and_not_alice() {
            let default_accounts = default_accounts();
            use_random_chain_extension();
            set_next_caller(default_accounts.bob);
            let mut contract = Lottery::new();

            assert_eq!(contract.register_bet(get_win_bet()), Ok(()));

            set_next_caller(default_accounts.alice);
            let mut bet_arr2 = [0; 32];
            bet_arr2[0] = 1;
            bet_arr2[1] = 1;
            bet_arr2[2] = 1;

            assert_eq!(contract.register_bet(bet_arr2), Ok(()));

            contract.draw();
            let winner = contract.get_last_winner_or_default();

            let mut should_be_winner: [AccountId; 8] = [AccountId::default(); 8];
            should_be_winner[0] = default_accounts.bob;
            assert_eq!(should_be_winner, winner);

            let mut not_the_winner: [AccountId; 8] = [AccountId::default(); 8];
            not_the_winner[0] = default_accounts.alice;
            assert_ne!(not_the_winner, winner);
        }

        #[ink::test]
        fn test_1000_applicants() {
            use_random_chain_extension();
            let mut contract = setup_jackpot(255);
            assert_eq!(contract.register_bet(get_win_bet()), Ok(()));
        }

        #[ink::test]
        fn winner_is_bob_and_alice() {
            let default_accounts = default_accounts();
            use_random_chain_extension();
            set_next_caller(default_accounts.bob);
            let mut contract = Lottery::new();
            assert_eq!(contract.register_bet(get_win_bet()), Ok(()));

            set_next_caller(default_accounts.alice);
            assert_eq!(contract.register_bet(get_win_bet()), Ok(()));

            contract.draw();
            assert_eq!(get_win_bet(), contract.get());
            let winner = contract.get_last_winner_or_default();
            let mut should_win: [AccountId; 8] = [AccountId::default(); 8];
            should_win[0] = default_accounts.bob;
            should_win[1] = default_accounts.alice;
            assert_eq!(should_win, winner);
        }

        #[ink::test]
        fn jackpot_should_be_twice_bet_price() {
            let mut contract = Lottery::new();
            contract = register_number_of_win_bets(2, contract);
            assert_eq!(2_000_000, contract.get_jackpot());
        }

        #[ink::test]
        fn two_winner_jackpot_should_be_half() {
            let mut contract = setup_jackpot(5);
            contract = register_number_of_win_bets(2, contract);
            contract.draw();
            let winner = contract.get_winner_or_default();
            contract.transfer_to_winners(winner);

            assert_eq!(3_500_000, contract.get_last_pot_per_bet());
        }

        #[ink::test]
        fn three_winner_jackpot_should_be_third() {
            let mut contract = setup_jackpot(5);
            contract = register_number_of_win_bets(3, contract);

            contract.draw();
            let winner = contract.get_winner_or_default();
            contract.transfer_to_winners(winner);

            assert_eq!(2_000_000, contract.get_last_pot_per_bet());
        }

        #[ink::test]
        fn four_winner_jackpot_should_be_fourth() {
            let mut contract = setup_jackpot(5);
            contract = register_number_of_win_bets(4, contract);

            contract.draw();
            let winner = contract.get_winner_or_default();
            contract.transfer_to_winners(winner);

            assert_eq!(2_250_000, contract.get_last_pot_per_bet());
        }

        #[ink::test]
        fn eight_winner_jackpot_should_be_eighth() {
            let mut contract = setup_jackpot(5);
            contract = register_number_of_win_bets(8, contract);

            contract.draw();
            let winner = contract.get_winner_or_default();
            contract.transfer_to_winners(winner);

            assert_eq!(1_625_000, contract.get_last_pot_per_bet());
        }

        #[ink::test]
        fn reset_game_works() {
            let default_accounts = default_accounts();
            let mut contract = Lottery::new();
            set_next_caller(default_accounts.alice);
            let bet = get_win_bet();

            let mut bet_arr2 = [0; 32];
            bet_arr2[0] = 1;
            bet_arr2[1] = 1;
            bet_arr2[2] = 1;

            assert_eq!(contract.register_bet(bet), Ok(()));
            assert_eq!(contract.register_bet(bet_arr2), Ok(()));
            let account_bet = contract.get_accounts_by_bet(bet);
            contract.reset_game();

            assert_ne!(contract.get_accounts_by_bet(bet), account_bet);
        }
    }
}
