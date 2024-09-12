# Mina On-Chain Voting

On-Chain Voting is a protocol developed to help with governing the Mina L1 blockchain.

## Vote With [Auro Wallet](https://www.aurowallet.com/)

1. Ensure you're on the correct network (in this case devnet).
2. Click on your address to copy it to the clipboard (we'll be using it in the next step).
3. Click on "Send."

![Ouro Home](./ouro_screens/1.home.png)

4. Paste your address into `To`.
5. Add `1` in the `Amount` field.
6. Depending on how you want to cast your vote, enter either `MIP#` or `no MIP#` where `#` is
   substituted with the actual MIP number.

![Vote Details](./ouro_screens/2.vote_details.png)

7. Confirm the transaction.

![Vote Confirmation](./ouro_screens/3.confirmation.png)

## Protocol Specifications

The On-Chain Voting Protocol is designed to provide community members with a transparent and secure
method of participating in the decision-making process for the Mina blockchain. The aim for this
protocol is to provide stakeholders with the ability to vote on MIPs (Mina Improvement Proposals)
with ease.

(Individual MIPs should be created by following the process described on the
[GitHub repository](https://github.com/MinaProtocol/MIPs).)

### Voting on a MIP

To cast a vote on a particular MIP, a user must send a transaction to **themselves** with a
specially-constructed memo field. The memo field must adhere to the following convention.

**For example:**

```
To vote in favor of 'MIP1', the memo field must be populated with: 'MIP1'
Similarly, if the intent is to vote against 'MIP1', the memo field must
contain: 'no MIP1'.
```

**The transaction amount must be 0, with the user only paying for the transaction fee.**

For more details, see:

- The [article by Granola](https://granola.team/blog/mina-on-chain-voting-results-instructions/)
- The [FAQ](https://forums.minaprotocol.com/t/on-chain-voting-frequently-asked-questions-faq/5959)

## Software Development

Install [Nix](https://nixos.org/download) and [direnv](https://direnv.net/docs/installation.html).

The `.envrc` will instruct `direnv` to use Nix to install the prerequisites for development on this
repo.

Without Nix, you will need to install, most notably:

- [just](https://just.systems/man/en/)
- libpq
- rustup
- libiconv
- llvm
- nodejs
- pkg-config
- podman/Docker
- pnpm (and issue `pnpm env use --global 18`)

Configure your `.env` file. See the [`.env.example`](./.env.example).

Optionally issue `just install` to install yet more dependencies.

Issue `just` to see the possible development actions. Examine [Justfile](./Justfile) to see how they
work.

For more information about Diesel and database migrations, see
[the official docs.](https://crates.io/crates/diesel_cli).

## Contributing

Contributions are welcome! This project uses the
[C4 (Collective Code Construction Contract)](https://rfc.zeromq.org/spec/42/) process for
contributions.

## License

This project is licensed under the Mozilla Public License 2.0. See the [LICENSE](./LICENSE) file for
the full license text.
