#![cfg_attr(not(feature = "std"), no_std)]

use ink_lang as ink;

#[ink::contract]
mod lottery {
    pub type Result<T> = core::result::Result<T, Error>;

    use ink_storage::Mapping;

    /// Emitted whenever a new bet is being registered.
    #[ink(event)]
    pub struct RegisterBet {
        #[ink(topic)]
        bet: [u8; 32],
        #[ink(topic)]
        from: AccountId,
    }

    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    pub struct Lottery {
        ticket_and_address: Mapping<[u8; 32], [AccountId; 8]>,
        def_address: [AccountId; 8],
        jackpot: Balance,
    }

    /// Errors that can occur upon calling this contract.
    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(::scale_info::TypeInfo))]
    pub enum Error {
        BetAlreadyExists,
        TicketCosts,
    }

    const BET_PRICE: Balance = 1_000_000_000_000;

    impl Lottery {
        /// Constructor that initializes the `bool` value to the given `init_value`.
        #[ink(constructor)]
        pub fn new() -> Self {
            Self {
                ticket_and_address: Mapping::default(),
                jackpot: 0,
                def_address: [AccountId::default(); 8],
            }
        }
        /// Register specific bet with caller as owner.
        #[ink(message, payable)]
        pub fn register_bet(&mut self, bet: [u8; 32]) -> Result<()> {
            let trans_bal = self.env().transferred_value();
            assert!(trans_bal == BET_PRICE, "insufficient funds!");
            self.jackpot += trans_bal;

            let caller = self.env().caller();

            if self.ticket_and_address.contains(bet) {
                let mut betters = self.ticket_and_address.get(&bet).unwrap();
                assert!(betters[7] == AccountId::default(), "bet sold out!");
                for i in 0..betters.len() {
                    if betters[i] == AccountId::default() {
                        betters[i] = caller;
                        self.ticket_and_address.insert(bet, &betters);
                        self.env().emit_event(RegisterBet { bet, from: caller });
                        break;
                    }
                }
            } else {
                let mut betters: [AccountId; 8] = [AccountId::default(); 8];
                betters[0] = caller;
                self.ticket_and_address.insert(bet, &betters);
                self.env().emit_event(RegisterBet { bet, from: caller });
            }
            Ok(())
        }
        // Get all accounts per bet
        #[ink(message)]
        pub fn get_accounts_by_bet(&self, bet_hash: [u8; 32]) -> [AccountId; 8] {
            return self
                .ticket_and_address
                .get(&bet_hash)
                .unwrap_or(self.def_address);
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// Imports `ink_lang` so we can use `#[ink::test]`.
        use ink_lang as ink;

        fn default_accounts() -> ink_env::test::DefaultAccounts<ink_env::DefaultEnvironment> {
            ink_env::test::default_accounts::<Environment>()
        }

        fn set_next_caller(caller: AccountId) {
            ink_env::test::set_caller::<Environment>(caller);
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
            let contract = Lottery::new();
            assert_eq!(
                contract.get_accounts_by_bet([0; 32]),
                [AccountId::default(); 8]
            );
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
    }
}
