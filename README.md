<h1 align="center">Mina On-Chain Voting</h1>

<p align="center">
  <b>On-Chain Voting is a protocol developed to govern the Mina Blockchain.</b>
</p>

## Protocol Specifications (WIP)

The On-Chain Voting Protocol is designed to provide community members with a transparent and secure method of participating in the decision-making process for the Mina blockchain. The aim for this protocol is to provide stake holders with the ability to vote on MIPs (Mina Improvement Proposals) in a clear & concise way.

Individual MIPs can be created on Github. ([https://github.com/MinaProtocol/MIPs](https://github.com/MinaProtocol/MIPs))

### Voting on a MIP

To cast a vote on a particular MIP, users must send a transaction to the **themselves** with a specific memo.<br>
The memo field must adhere to the following convention:<br>

**For example:**

```
To vote in favor of 'MIP1', the memo field would be populated with: 'MIP1'
Similarly - if your intent is to vote against 'MIP1', the memo field would contain: 'no MIP1'
```

**The transaction amount must be 0, with the user only paying for the transaction fee.**

### Protocol Flow

This flow chart illustrates the process of voting for a specific MIP on Mina blockchain.<br>
**Documentation will be updated.**

## Development

- If not installed, install [`nvm`](https://github.com/nvm-sh/nvm)

  ```bash
  curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.3/install.sh | bash

  # or ...

  wget -qO- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.3/install.sh | bash
  ```

  ```bash
  nvm install 16

  # and ...

  nvm use default v16
  ```

- If not installed, install [`pnpm`](https://pnpm.io/)

  ```bash
  brew install pnpm

  # or ...

  curl -fsSL https://get.pnpm.io/install.sh | sh -
  ```

- If not installed, install [Rust](https://www.rust-lang.org/) - [Cargo-Make](https://github.com/sagiegurari/cargo-make) - [Typeshare-CLI](https://github.com/1Password/typeshare) - [Diesel-CLI](https://crates.io/crates/diesel_cli/2.0.1)

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh # install rust
  cargo install --force cargo-make # install cargo-make
  cargo install diesel_cli --no-default-features --features postgres # install diesel-cli
  cargo install typeshare-cli # install typeshare-cli

  ```

- Checkout this repository via `git` or the [Github CLI](https://cli.github.com/)

  ```bash
  git clone git@github.com:Granola-Team/mina-governance.git

  # or ...

  gh repo clone Granola-Team/mina-governance
  ```

- In the new directory, install dependencies

  ```bash
  pnpm clean && pnpm install
  ```

### Generated files and types

This project relies on [Typeshare-CLI](https://github.com/1Password/typeshare) to generate `web/models/generated.ts` containing .ts types.</br>
These files are **required** for running, testing, and deploying the application.</br>

Typeshare needs to be run manually run via `pnpm cargo:generate`.</br>
If you're encountering errors with missing references, re-run the generator.

### Running in Docker

Run `docker-compose up` or `pnpm docker` to mount the cluster, and then run all pending migrations.

> **IMPORTANT:**
When running locally, modify the respective `.env` variables to point to `db` and `server` (the internal Docker host).

### Running in the console

You can run the web-app in console and mount the database and server in Docker to exercise more authority over the environment.

> **IMPORTANT:** When running this way, the database URL in the `.env` file has to point to `localhost`.</br>
See [`.env.example`](./.env.example) for more information on the `DATABASE_URL` env var.

- Mount the database and server in Docker.

  ```sh
  pnpm docker:server-db

  # or ...

  docker-compose --profile server-db up
  ```

- Run migrations.

  ```sh
  diesel migration run
  ```

- Run the app in development mode.

  ```sh
  pnpm web dev
  ```

### Managing the database and migrations

The development database is mounted in Docker and managed via the
[Diesel CLI](https://diesel.rs/guides/getting-started)

- `diesel database reset` — reset the database (**all data will be wiped out!**)

- `diesel database setup` — create the database if it doesn't exist and run all migrations.

- `diesel migration generate [name]` — create a new migration for changes to the schema.

For more commands and options, see [the official docs.](https://crates.io/crates/diesel_cli)

## Resources

- [Next.js Documentation](https://nextjs.org/docs/getting-started)
- [Rust Programming Language](https://doc.rust-lang.org/book/)
- [Typescript](https://www.typescriptlang.org/docs/)

## License

This project is licensed under the Mozilla Public License 2.0. See the [LICENSE](LICENSE) file for the full license text.