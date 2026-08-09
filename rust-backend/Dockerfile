FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
RUN apk add --no-cache musl-dev protobuf-dev make gcc g++ openssl-dev openssl-libs-static pkgconfig

WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock* build.rs ./
COPY proto ./proto
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Layer này được Docker cache 100% khi Cargo.toml/Cargo.lock không thay đổi
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

# Copy duy nhất source code Rust để biên dịch binary
COPY Cargo.toml Cargo.lock* build.rs ./
COPY proto ./proto
COPY src ./src
RUN --mount=type=cache,id=cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=cargo-target,target=/app/target \
    cargo build --release && cp /app/target/release/rust-backend /app/rust-backend

# Copy giao diện tĩnh (HTML/CSS/JS) SAU KHI đã biên dịch xong binary Rust
# Giúp việc chỉnh sửa FE KHÔNG BAO GIỜ kích hoạt lại bước cargo build!
COPY templates ./templates
COPY public ./public

FROM alpine:3.20 AS runner
WORKDIR /app

RUN apk add --no-cache ca-certificates tzdata
RUN addgroup -S appgroup && adduser -S appuser -G appgroup

COPY --from=builder /app/rust-backend /app/rust-backend
COPY --from=builder /app/templates /app/templates
COPY --from=builder /app/public /app/public

RUN chown -R appuser:appgroup /app
USER appuser

EXPOSE 4000
ENV RUST_LOG=info

CMD ["/app/rust-backend"]
