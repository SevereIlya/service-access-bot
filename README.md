# Telegram Subscription Management Backend

A production-grade, highly scalable asynchronous backend for Telegram bots built with Rust.

This repository serves as a portfolio showcase of advanced software engineering patterns. It demonstrates how to build robust, maintainable, and strictly typed systems using modern architectural principles, avoiding the common pitfalls of tight coupling and anemic domain models.

*Note: Proprietary business logic and specific integrations have been abstracted into generic interfaces to protect commercial interests, while the core architectural skeleton remains fully intact.*

## Architectural Overview

This project strictly adheres to **Clean Architecture** and **Domain-Driven Design (DDD)** principles. The codebase is organized into four distinct layers, with dependencies strictly pointing inward toward the Domain.

```text
src/
├── adapters/       # Delivery mechanisms: Telegram framework integration, UI views, routing.
├── application/    # Orchestration layer: Use cases (Commands/Queries) and transaction boundaries.
├── domain/         # Core business logic: Entities, Value Objects, Domain Errors.
└── infrastructure/ # External concerns: PostgreSQL connections, SQLx, Configuration, DI Container.
```

## Core Engineering Patterns

### 1. Domain-Driven Design (DDD)
*   **Rich Domain Models:** Business rules (e.g., trial eligibility, discount processing) are encapsulated directly within Domain Entities (`User`, `Subscription`). State mutation is strictly controlled via domain methods; properties are private to prevent invalid states.
*   **Value Objects (Newtype Pattern):** Extensive use of domain primitives (`UserId`, `TelegramId`, `Money`, `SubscriptionPlan`) to leverage Rust's type system, preventing logic errors and invalid comparisons at compile time.
*   **Reconstitution Pattern:** Entities utilize isolated `restore_from_db` constructors used strictly by the Infrastructure layer mapping, ensuring business invariants are not bypassed during data retrieval.

### 2. CQRS (Command and Query Responsibility Segregation)
Application logic is divided into isolated `Commands` (state-mutating operations) and `Queries` (read-only operations). This eliminates bloated service classes and allows independent scaling and testing of read and write paths.

### 3. Unit of Work (UoW)
A custom UoW implementation manages transaction boundaries across multiple PostgreSQL repositories. By abstracting `sqlx::Transaction` behind a `tokio::sync::Mutex`, the Domain and Application layers can execute atomic operations safely in an asynchronous environment without being coupled to the underlying database driver.

### 4. Advanced Database Design
*   **Internal vs. External Keys:** The database utilizes `BIGINT` identity columns as Primary Keys for highly optimized internal joins, while exposing `UUID` (External IDs) for secure API integrations and third-party interactions.
*   **Slowly Changing Dimensions (SCD):** Financial integrity is guaranteed by snapshotting base prices (`frozen_base_price`) at the time of user registration, ensuring historical data remains consistent regardless of future pricing changes.
*   **Zero-Cost Compile-Time Queries:** Heavy reliance on `sqlx` macros ensures that all SQL queries are verified against the active database schema during compilation.

### 5. Observability & Structured Logging
The project implements rigorous structured logging using the `tracing` crate. Logs are strictly separated by severity levels. `#[instrument]` macros are used at the Adapter layer to automatically inject user context (e.g., `telegram_id`) into the span, ensuring complete request traceability across all application layers without polluting the domain logic.

### 6. Dependency Injection
Infrastructure layers (Repositories, UoW) are instantiated in a central Composition Root (`AppState` container) and injected into the Application layer via `Arc`. This ensures the business logic remains 100% testable using mock repositories without requiring a live database connection.

## Technology Stack

*   **Language:** Rust (Stable)
*   **Async Runtime:** Tokio
*   **Database:** PostgreSQL
*   **Database Toolkit:** SQLx
*   **Telegram Framework:** Teloxide
*   **Observability:** Tracing, Tracing-Subscriber
*   **Configuration:** Config (TOML based)

## Local Development

While this repository acts primarily as a codebase showcase, it is fully functional.

To run the environment locally:
1. Provide a PostgreSQL database and a Telegram Bot token.
2. Copy `.env.example` to `.env`, `config.example.toml` to `config.toml` and `en.example.toml` to `en.toml`
3. Apply database migrations: `cargo sqlx migrate run`.
4. Run the test suite: `cargo test`.
5. Start the backend: `cargo run`.

## License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0). See the `LICENSE` file for details.