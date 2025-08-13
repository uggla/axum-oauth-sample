# Use the official Rust 1.74 image as the base image
FROM rust:1.88 as builder

# Set the working directory in the container
WORKDIR /app/src

# Install SQLx CLI
RUN cargo install sqlx-cli

# Copy the necessary files for SQLx migrations
COPY ./migrations ./migrations

# Create and apply the migration
RUN mkdir data && sqlx database create --database-url "sqlite:./data/data.db"
RUN sqlx migrate run --database-url "sqlite:./data/data.db"

# Copy the entire project to the container
COPY . .

# Build the Rust project
RUN export DATABASE_URL="sqlite:./data/data.db" && cargo build --release

# Create a new image
FROM ubuntu:24.04 as runner
RUN apt-get update && \
    apt-get install openssl -y && \
    apt-get install -y ca-certificates

# Set the working directory in the container
WORKDIR /app

# Copy necessary files to run the binary
COPY --from=builder /app/src/target/release/axum-oauth-sample /app/axum-oauth-sample
COPY --from=builder /app/src/data /app/data
COPY --from=builder /app/src/public /app/public
COPY --from=builder /app/src/templates /app/templates

# Run the Rust project
CMD ["./axum-oauth-sample"]
