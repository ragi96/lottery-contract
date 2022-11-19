# lottery

Lottery Smart Contract written in ink!

## concept

Contract where you can buy lottery "bets"

The Sorting of the picks matter.

There is a drawing every x blocks of 3 numbers between 0 to 255

The bet is running till somebody bet has all 3 numbers right, then the game restarts

## usage

### run tests

```
cargo +nightly test
```

### with outputs

```
cargo test -- --nocapture
```
`

### create coverage lcov

```
cargo tarpaulin
```

### build contract

```
cargo +nightly contract build
```
