# Telegram Bot Backend

A production-grade, highly scalable asynchronous backend for Telegram bots built with Rust.

This repository serves as a portfolio showcase of advanced software engineering patterns. It demonstrates how to build robust, maintainable, and strictly typed systems using modern architectural principles.

*Note: This is the "Core" version of the project. Proprietary business logic, specific billing integrations, and marketing UI copies have been replaced with generic stubs or mocked adapters to protect commercial interests.*

## Architectural Highlights

This project strictly adheres to **Clean Architecture** and **Domain-Driven Design (DDD)** principles, ensuring a complete decoupling of business logic from infrastructure and delivery mechanisms.

* **Domain-Driven Design (DDD):** Rich domain model encapsulation. Business rules (e.g., trial eligibility, discount calculations) are enforced within Domain Entities (`User`, `Subscription`), not in application services. Extensive use of the Newtype pattern (`UserId`, `TelegramId`, `Money`) prevents logic errors at compile time.
* **CQRS (Command and Query Responsibility Segregation):** Application logic is divided into isolated `Commands` (state-mutating operations) and `Queries` (read-only operations), eliminating bloated "God Objects" or God Services.
* **Unit of Work (UoW) Pattern:** Custom UoW implementation using `tokio::sync::Mutex` and `sqlx::Transaction`. It guarantees atomic operations and transactional consistency across multiple PostgreSQL repositories.
* **Dependency Injection:** Infrastructure layers (Database, Telegram API) are decoupled from the core logic using Rust's dynamic trait objects (`dyn Trait`), making the business logic 100% testable without requiring a live database connection.
* **Isolated Error Handling:** Strict separation between Domain errors (business rule violations) and Application errors (system/infrastructure failures), utilizing the `thiserror` crate for clean error mapping.

## Technology Stack

* **Language:** Rust
* **Async Runtime:** Tokio
* **Database:** PostgreSQL
* **Database Toolkit:** SQLx (Raw SQL macros for zero-cost abstraction and compile-time query verification)
* **Telegram Framework:** Teloxide
* **Configuration:** Config (TOML based), dotenv

## Project Structure

The codebase is organized into four primary layers, with dependencies strictly pointing inward toward the Domain.

```text
src/
├── adapters/       # Delivery mechanisms: Telegram framework integration, routing, handlers, and UI views.
├── application/    # Orchestration layer: Use cases (Commands/Queries) and application-level error definitions.
├── domain/         # Core business logic: Entities, Value Objects, Domain Errors, and Repository/UoW interfaces.
└── infrastructure/ # External concerns: PostgreSQL connections, SQLx implementations, and configuration parsers.
```

## Getting Started

### Prerequisites

* Rust (latest stable toolchain)
* PostgreSQL database

### Configuration

1. Clone the repository:
   ```bash
   git clone <repository_url>
   cd enterprise-bot-core
   ```

2. Set up environment variables:
   Copy `.env.example` to `.env` and provide your database URL.
   ```bash
   cp .env.example .env
   ```

3. Set up the application configuration:
   Copy `config.example.toml` to `config.toml` and fill in the required parameters (Telegram token, database URL, etc.).
   ```bash
   cp config.example.toml config.toml
   ```

### Running the Application

To compile and run the bot in development mode:

```bash
cargo run
```

To run the test suite (Domain unit tests and Use Case retry-logic tests):

```bash
cargo test
```

## License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0). See the `LICENSE` file for details.