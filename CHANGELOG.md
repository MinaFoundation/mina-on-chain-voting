# Changelog

## [Post-MIP] as of 2023-Sep-26

### Added

- We streamlined development and deployment using Nix, Just, and Podman: for building, following
  best open-source practices and enhancements, being able to build with one command, and not reliant
  on Docker
  - Nix ensures reproducibility and isolation by defining precise dependencies
  - Just simplifies and automates common tasks with single-command actions
  - Podman offers enhanced security and flexibility for containerization
- Updated Results button to be a ternary operator with either Results or Go Vote appearing next to
  the button icon
- Adding corresponding, respective URL links and matching text with the database
- Control log level functionality from an .env file
- Added customized code and fix for the next-router-mock given lack of support from Next 13 and this
  customization fixes broken tests in CI

### Improvements or Migrations

- Clarify and improve build instructions
- Migrated to Next 13's app router with a redesigned UI
- Migrated components to Tailwind & Radix
- Migrated pages to Next 13's app router
- Migrated & extended tests to increase coverage
- Improve precision by extending to 4 decimal places from 2 and remove any unnecessary trailing
  zeros
- Improve Docker build and integration between postgres and server
- Improve .env.example file for better networking options and ports suggested
- Improve various frontend and UI changes regarding filtering and sorting on tables, title, nav bar,
  footer, coloring, MIP key to be just MIP# vs # - MIP#
- Fix graph dates display, order of dates, and overall display
- Update multiple dependencies across the repo (some advised by dependabot)
- Extend README to DEVELOPER docs and splits across the server and web directories
- Improve GitHub CI configuration

### Deprecated or Removed

- Removed unused Playwright functionality
- Remove unused features (MUI - Storybook - Typeshare)

## [Pre-MIP4] - 2023-05-20

### Added

- Database migration script to make global_slot numbers i64 in Mina Proposals
- Updated the OCV app so it supports other networks other than mainnet
- Added rustsec audit-check in github action support
- Updated TTL cache with filtering and whitelisting
- Installed and configured Storybook, Playwright, Jest configurations (and custom jest render
  utilities), and MUI base tooling
- Migrated, converted and refactored tests
- Setup Typeshare CLI to generate TypeScript bindings from Rust types
- Added zod schema validation for frontend as well as queries and store
- Update environment variables and configuration
- Added core-info endpoint for server as well as minor schema modifications
- Added extension settings [.vscode]
- Setup workspaces [pnpm cargo] and added workspace scripts
- Added and updated top-level configurations [pnpm eslint prettier] [other IDE Extensions]
- Create Dockerfile and schema using Diesel ORM

### Improvements

- Sanitize SQL query params in function fetch_transactions using bind
- Improved security with specific CORS origins and preflight
- Improve the Github actions workflows to only build projects that have changed
- Decouple frontend and backend codebases and then migrate and refactor them to NextJS and Vercel
  deployment as well as refactor and restructure server modules
- Double precision errors that occur with floating point types by using Decimal type
- Updated README.md files

### Deprecated or Removed

- Removed static page serving
- Removed dummy data and Nix build system
- Deprecated Haskell tools and scripts related to the archive node

## [Pre-MIP1] - 2023-01-04

### Added

- Created FAQs and feedback forms
- New feature and support for pagination of results
- Archive node missing blocks script and runbook to check and patch missing blocks when needed
- Create tests (unit, functional, etc.) for server and client
- Added timestamp checking to the canonical OCV query so as to avoid votes being counted that were
  cast before the start of the voting period
- Created Haskell scripts to download, clean and run archive dumps and connect to the archive node's
  PostgreSQL database
- Ability for users explore a tx hash instead of copying and manually going to verify the
  transaction with a third-party
- Updated the SQL query, sqlx uses github workflows to determine query validity, and LedgerAccount
  changed to an array from a vector and begins empty rather than zero
- Created a progress bar to display the progress of the voting time period
- Memo all lowercase to avoid case sensitivity
- Delegating stake is clearly expressed in browser
- Maintain certain version of tokio and axum to avoid security issue
- Created tooltips, such as for total votes calculations

### Deprecated or Removed

- Deprecated feature to count valid signals from accounts not on the Staking Ledger in the total
  number of signals, but not for the total stake
