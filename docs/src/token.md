---
title: Token Program
---

A Token program on the Solana blockchain.

This program defines a common implementation for Fungible and Non Fungible tokens.

## Background

Solana's programming model and the definitions of the Solana terms used in this
document are available at:

- https://docs.solana.com/apps
- https://docs.solana.com/terminology

## Source

The Token Program's source is available on
[github](https://github.com/solana-labs/solana-program-library)

## Interface

The on-chain Token Program is written in Rust and available on crates.io as
[spl-token](https://docs.rs/spl-token). The program's [instruction interface
documentation](https://docs.rs/spl-token/2.0.4/spl_token/instruction/enum.TokenInstruction.html)
can also be found there.

Auto-generated C bindings are also available for the on-chain Token Program and
available
[here](https://github.com/solana-labs/solana-program-library/blob/master/token/program/inc/token.h)

[JavaScript
bindings](https://github.com/solana-labs/solana-program-library/blob/master/token/js/client/token.js)
are available that support loading the Token Program on to a chain and issue
instructions.

## Command-line Utility

The `spl-token` command-line utility can be used to experiment with SPL
tokens.  Once you have [Rust installed](https://rustup.rs/), run:
```sh
$ cargo install spl-token-cli
```

Run `spl-token --help` for a full description of available commands.

### Configuration

The `spl-token` configuration is shared with the `solana` command-line tool.

#### Current Configuration

```
solana config get
```

```
Config File: ${HOME}/.config/solana/cli/config.yml
RPC URL: https://api.mainnet-beta.solana.com
WebSocket URL: wss://api.mainnet-beta.solana.com/ (computed)
Keypair Path: ${HOME}/.config/solana/id.json
```

#### Cluster RPC URL

See [Solana clusters](https://docs.solana.com/clusters) for cluster-specific RPC URLs
```
solana config set --url https://devnet.solana.com
```

#### Default Keypair

See [Keypair conventions]
(https://docs.solana.com/cli/conventions#keypair-conventions) for information on
how to setup a keypair if you don't already have one.

Keypair File
```
solana config set --keypair ${HOME}/new-keypair.json
```

Hardware Wallet URL (See [URL spec](https://docs.solana.com/wallet-guide/hardware-wallets#specify-a-keypair-url))
```
solana config set --keypair usb://ledger/
```

### Example: Creating your own fungible token

```sh
$ spl-token create-token
Creating token AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM
Signature: 47hsLFxWRCg8azaZZPSnQR8DNTRsGyPNfUK7jqyzgt7wf9eag3nSnewqoZrVZHKm8zt3B6gzxhr91gdQ5qYrsRG4
```

The unique identifier of the token is `AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM`.

Tokens when initially created by `spl-token` have no supply:
```sh
spl-token supply AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM
0
```

Let's mint some.  First create an account to hold a balance of the new
`AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM` token:
```sh
$ spl-token create-account AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM
Creating account 7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi
Signature: 42Sa5eK9dMEQyvD9GMHuKxXf55WLZ7tfjabUKDhNoZRAxj9MsnN7omriWMEHXLea3aYpjZ862qocRLVikvkHkyfy
```

`7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi` is now an empty account:
```sh
$ spl-token balance 7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi
0
```

Mint 100 tokens into the account:
```sh
$ spl-token mint AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM 100 \
                 7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi
Minting 100 tokens
  Token: AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM
  Recipient: 7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi
Signature: 41mARH42fPkbYn1mvQ6hYLjmJtjW98NXwd6pHqEYg9p8RnuoUsMxVd16RkStDHEzcS2sfpSEpFscrJQn3HkHzLaa
```

The token `supply` and account `balance` now reflect the result of minting:
```sh
$ spl-token supply AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM
100
$ spl-token balance 7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi
100
```

### Example: View all Tokens that you own

```sh
$ spl-token accounts
Account                                      Token                                        Balance
-------------------------------------------------------------------------------------------------
2ryb53FGVLVYFXmAemN7avawevuNFXwTVetMpH9ag3XZ 7e2X5oeAAJyUTi4PfSGXFLGhyPw2H8oELm1mx87ZCgwF 84
7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM 100
CqAxDdBRnawzx9q4PYM3wrybLHBhDZ4P6BTV13WsRJYJ AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM 0
JAopo117aj6HMwCRjXSyNpZfGDJRi7ukqHgs2inXD8Rc AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM 0
```

### Example: Wrapping SOL in a Token

```sh
$ spl-token wrap 1
Wrapping 1 SOL into GJTxcnA5Sydy8YRhqvHxbQ5QNsPyRKvzguodQEaShJje
Signature: 4f4s5QVMKisLS6ihZcXXPbiBAzjnvkBcp2A7KKER7k9DwJ4qjbVsQBKv2rAyBumXC1gLn8EJQhwWkybE4yJGnw2Y
```

To unwrap the Token back to SOL:
```
$ spl-token unwrap GJTxcnA5Sydy8YRhqvHxbQ5QNsPyRKvzguodQEaShJje
Unwrapping GJTxcnA5Sydy8YRhqvHxbQ5QNsPyRKvzguodQEaShJje
  Amount: 1 SOL
  Recipient: vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg
Signature: f7opZ86ZHKGvkJBQsJ8Pk81v8F3v1VUfyd4kFs4CABmfTnSZK5BffETznUU3tEWvzibgKJASCf7TUpDmwGi8Rmh
```

### Example: Transferring tokens

```
$ spl-token create-account AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM
Creating account CqAxDdBRnawzx9q4PYM3wrybLHBhDZ4P6BTV13WsRJYJ
Signature: 4yPWj22mbyLu5mhfZ5WATNfYzTt5EQ7LGzryxM7Ufu7QCVjTE7czZdEBqdKR7vjKsfAqsBdjU58NJvXrTqCXvfWW
```
```
$ spl-token accounts AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM
Account                                      Token                                        Balance
-------------------------------------------------------------------------------------------------
7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM 100
CqAxDdBRnawzx9q4PYM3wrybLHBhDZ4P6BTV13WsRJYJ AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM 0
```

```
$ spl-token transfer 7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi 50 CqAxDdBRnawzx9q4PYM3wrybLHBhDZ4P6BTV13WsRJYJ
Transfer 50 tokens
  Sender: 7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi
  Recipient: CqAxDdBRnawzx9q4PYM3wrybLHBhDZ4P6BTV13WsRJYJ
Signature: 5a3qbvoJQnTAxGPHCugibZTbSu7xuTgkxvF4EJupRjRXGgZZrnWFmKzfEzcqKF2ogCaF4QKVbAtuFx7xGwrDUcGd
```
```
$ spl-token accounts AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM
Account                                      Token                                        Balance
-------------------------------------------------------------------------------------------------
7UX2i7SucgLMQcfZ75s3VXmZZY4YRUyJN9X1RgfMoDUi AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM 50
CqAxDdBRnawzx9q4PYM3wrybLHBhDZ4P6BTV13WsRJYJ AQoKYV7tYpTrFZN6P5oUufbQKAUr9mNYGe1TTJC9wajM 50
```

### Example: Create a non-fungible token

Create the token type,
```
$ spl-token create-token
Creating token 559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z
Signature: 4kz82JUey1B9ki1McPW7NYv1NqPKCod6WNptSkYqtuiEsQb9exHaktSAHJJsm4YxuGNW4NugPJMFX9ee6WA2dXts
```

then create an account to hold tokens of this new type:
```
$ spl-token create-account 559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z
Creating account 7KqpRwzkkeweW5jQoETyLzhvs9rcCj9dVQ1MnzudirsM
Signature: sjChze6ecaRtvuQVZuwURyg6teYeiH8ZwT6UTuFNKjrdayQQ3KNdPB7d2DtUZ6McafBfEefejHkJ6MWQEfVHLtC
```

Now mint only one token into the account,
```
$ spl-token mint 559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z 1 7KqpRwzkkeweW5jQoETyLzhvs9rcCj9dVQ1MnzudirsM
Minting 1 tokens
  Token: 559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z
  Recipient: 7KqpRwzkkeweW5jQoETyLzhvs9rcCj9dVQ1MnzudirsM
Signature: 2Kzg6ZArQRCRvcoKSiievYy3sfPqGV91Whnz6SeimhJQXKBTYQf3E54tWg3zPpYLbcDexxyTxnj4QF69ucswfdY
```

and disable future minting:
```
$ spl-token authorize 559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z mint --disable
Updating 559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z
  Current mint authority: vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg
  New mint authority: disabled
Signature: 5QpykLzZsceoKcVRRFow9QCdae4Dp2zQAcjebyEWoezPFg2Np73gHKWQicHG1mqRdXu3yiZbrft3Q8JmqNRNqhwU
```

Now the `7KqpRwzkkeweW5jQoETyLzhvs9rcCj9dVQ1MnzudirsM` account holds the
one and only `559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z` token:

```
$ spl-token account-info 7KqpRwzkkeweW5jQoETyLzhvs9rcCj9dVQ1MnzudirsM

Address: 7KqpRwzkkeweW5jQoETyLzhvs9rcCj9dVQ1MnzudirsM
Balance: 1
Mint: 559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z
Owner: vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg
State: Initialized
Delegation: (not set)
Close authority: (not set)
```

```
$ spl-token supply 559u4Tdr9umKwft3yHMsnAxohhzkFnUBPAFtibwuZD9z
1
```

## JSON RPC methods

There is a rich set of JSON RPC methods available for use with SPL Token:
* `getTokenAccountBalance`
* `getTokenAccountsByDelegate`
* `getTokenAccountsByOwner`
* `getTokenLargestAccounts`
* `getTokenSupply`

See https://docs.solana.com/apps/jsonrpc-api for more details.

Additionally the versatile `getProgramAcccounts` JSON RPC method can be employed in various ways to fetch SPL Token accounts of interest.

### Finding all token accounts for a specific mint

To find all token accounts for the `TESTpKgj42ya3st2SQTKiANjTBmncQSCqLAZGcSPLGM` mint:
```
curl http://api.mainnet-beta.solana.com -X POST -H "Content-Type: application/json" -d '
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getProgramAccounts",
    "params": [
      "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
      {
        "encoding": "jsonParsed",
        "filters": [
          {
            "dataSize": 165
          },
          {
            "memcmp": {
              "offset": 0,
              "bytes": "TESTpKgj42ya3st2SQTKiANjTBmncQSCqLAZGcSPLGM"
            }
          }
        ]
      }
    ]
  }
'
```

The `"dataSize": 165` filter selects all [Token
Acccount](https://github.com/solana-labs/solana-program-library/blob/08d9999f997a8bf38719679be9d572f119d0d960/token/program/src/state.rs#L86-L106)s,
and then the `"memcmp": ...` filter selects based on the
[mint](https://github.com/solana-labs/solana-program-library/blob/08d9999f997a8bf38719679be9d572f119d0d960/token/program/src/state.rs#L88)
address within each token account.

### Finding all token accounts for a wallet

Find all token accounts owned by the `vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg` user:
```
curl http://api.mainnet-beta.solana.com -X POST -H "Content-Type: application/json" -d '
  {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "getProgramAccounts",
    "params": [
      "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
      {
        "encoding": "jsonParsed",
        "filters": [
          {
            "dataSize": 165
          },
          {
            "memcmp": {
              "offset": 32,
              "bytes": "vines1vzrYbzLMRdu58ou5XTby4qAqVRLmqo36NKPTg"
            }
          }
        ]
      }
    ]
  }
```

The `"dataSize": 165` filter selects all [Token
Acccount](https://github.com/solana-labs/solana-program-library/blob/08d9999f997a8bf38719679be9d572f119d0d960/token/program/src/state.rs#L86-L106)s,
and then the `"memcmp": ...` filter selects based on the
[owner](https://github.com/solana-labs/solana-program-library/blob/08d9999f997a8bf38719679be9d572f119d0d960/token/program/src/state.rs#L90)
address within each token account.

## Operational overview

### Creating a new token type

A new token type can be created by initializing a new Mint with the
`InitializeMint` instruction. The Mint is used to create or "mint" new tokens,
and these tokens are stored in Accounts. A Mint is associated with each
Account, which means that the total supply of a particular token type is equal
to the balances of all the associated Accounts.

It's important to note that the `InitializeMint` instruction does not require
the Solana account being initialized also be a signer. The `InitializeMint`
instruction should be atomically processed with the system instruction that
creates the Solana account by including both instructions in the same
transaction.

Once a Mint is initialized, the `mint_authority` can create new tokens using the
`MintTo` instruction.  As long as a Mint contains a valid `mint_authority`, the
Mint is considered to have a non-fixed supply, and the `mint_authority` can
create new tokens with the `MintTo` instruction at any time.  The `SetAuthority`
instruction can be used to irreversibly set the Mint's authority to `None`,
rendering the Mint's supply fixed.  No further tokens can ever be Minted.

Token supply can be reduced at any time by issuing a `Burn` instruction which
removes and discards tokens from an Account.

### Creating accounts

Accounts hold token balances and are created using the `InitializeAccount`
instruction. Each Account has an owner who must be present as a signer in some
instructions.

An Account's owner may transfer ownership of an account to another using the
`SetAuthority` instruction.

It's important to note that the `InitializeAccount` instruction does not require
the Solana account being initialized also be a signer. The `InitializeAccount`
instruction should be atomically processed with the system instruction that
creates the Solana account by including both instructions in the same
transaction.

### Transferring tokens
Balances can be transferred between Accounts using the `Transfer` instruction.
The owner of the source Account must be present as a signer in the `Transfer`
instruction when the source and destination accounts are different.

It's important to note that when the source and destination of a `Transfer` are
the **same**, the `Transfer` will _always_ succeed. Therefore, a successful `Transfer`
does not necessarily imply that the involved Accounts were valid SPL Token
accounts, that any tokens were moved, or that the source Account was present as
a signer. We strongly recommend that developers are careful about checking that
the source and destination are **different** before invoking a `Transfer`
instruction from within their program.

### Burning

The `Burn` instruction decreases an Account's token balance without transferring
to another Account, effectively removing the token from circulation permanently.

### Authority delegation

Account owners may delegate authority over some or all of their token balance
using the `Approve` instruction. Delegated authorities may transfer or burn up
to the amount they've been delegated. Authority delegation may be revoked by the
Account's owner via the `Revoke` instruction.

### Multisignatures

M of N multisignatures are supported and can be used in place of Mint
authorities or Account owners or delegates. Multisignature authorities must be
initialized with the `InitializeMultisig` instruction. Initialization specifies
the set of N public keys that are valid and the number M of those N that must be
present as instruction signers for the authority to be legitimate.

It's important to note that the `InitializeMultisig` instruction does not
require the Solana account being initialized also be a signer. The
`InitializeMultisig` instruction should be atomically processed with the system
instruction that creates the Solana account by including both instructions in
the same transaction.

### Freezing accounts

The Mint may also contain a `freeze_authority` which can be used to issue
`FreezeAccount` instructions that will render an Account unusable.  Token
instructions that include a frozen account will fail until the Account is thawed
using the `ThawAccount` instruction.  The `SetAuthority` instruction can be used
to change a Mint's `freeze_authority`.  If a Mint's `freeze_authority` is set to
`None` then account freezing and thawing is permanently disabled and all
currently frozen accounts will also stay frozen permanently.

### Wrapping SOL

The Token Program can be used to wrap native SOL. Doing so allows native SOL to
be treated like any other Token program token type and can be useful when being
called from other programs that interact with the Token Program's interface.

Accounts containing wrapped SOL are associated with a specific Mint called the
"Native Mint" using the public key
`So11111111111111111111111111111111111111112`.

These accounts have a few unique behaviors

- `InitializeAccount` sets the balance of the initialized Account to the SOL
  balance of the Solana account being initialized, resulting in a token balance
  equal to the SOL balance.
- Transfers to and from not only modify the token balance but also transfer an
  equal amount of SOL from the source account to the destination account.
- Burning is not supported
- When closing an Account the balance may be non-zero.

The Native Mint supply will always report 0, regardless of how much SOL is currently wrapped.

### Rent-exemption

To ensure a reliable calculation of supply, a consistency valid Mint, and
consistently valid Multisig accounts all Solana accounts holding a Account,
Mint, or Multisig must contain enough SOL to be considered [rent
exempt](https://docs.solana.com/implemented-proposals/rent)

### Closing accounts

An account may be closed using the `CloseAccount` instruction. When closing an
Account, all remaining SOL will be transferred to another Solana account
(doesn't have to be associated with the Token Program). Non-native Accounts must
have a balance of zero to be closed.

### Non-Fungible tokens
An NTF is simply a token type where only a single token has been minted.
