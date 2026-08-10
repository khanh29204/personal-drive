FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
# Bỏ openssl-dev + openssl-libs-static: reqwest đã chuyển sang rustls nên không
# còn crate nào trong cây phụ thuộc link OpenSSL (`cargo tree -i openssl-sys`
# không ra kết quả). Phần còn lại giữ nguyên — aws-lc-sys và ring dựng được
# bằng gcc/g++ có sẵn, không cần cmake.
RUN apk add --no-cache musl-dev protobuf-dev make gcc g++ pkgconfig

WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock* build.rs ./
COPY proto ./proto
COPY src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# build.rs + proto phải có mặt từ đây để build script (tonic-build, prost-build)
# cũng được biên dịch ngay trong bước cook
COPY build.rs ./
COPY proto ./proto
# Không dùng --mount=type=cache: cache mount của BuildKit không được export qua
# cache-to, nên trên CI nó luôn rỗng. Để target/ nằm trong layer thì layer cache
# mới mang được dependencies đã biên dịch sang lần build sau.
RUN cargo chef cook --release --recipe-path recipe.json

# Copy duy nhất source code Rust để biên dịch binary
COPY Cargo.toml Cargo.lock* build.rs ./
COPY proto ./proto
COPY src ./src
RUN cargo build --release && cp /app/target/release/rust-backend /app/rust-backend

# Copy giao diện tĩnh (HTML/CSS/JS) SAU KHI đã biên dịch xong binary Rust
# Giúp việc chỉnh sửa FE KHÔNG BAO GIỜ kích hoạt lại bước cargo build!
COPY templates ./templates
COPY public ./public

FROM alpine:3.20 AS runner
WORKDIR /app

# Không cài tzdata: toàn bộ mốc thời gian trong code dùng chrono::Utc, không có
# chỗ nào đổi sang timezone địa phương. Nếu sau này cần hiển thị giờ local thì
# phải thêm lại, nếu không chrono-tz sẽ không tìm thấy database.
RUN apk add --no-cache ca-certificates
RUN addgroup -S appgroup && adduser -S appuser -G appgroup

COPY --from=builder /app/rust-backend /app/rust-backend
COPY --from=builder /app/templates /app/templates
COPY --from=builder /app/public /app/public

RUN chown -R appuser:appgroup /app
USER appuser

EXPOSE 4000
ENV RUST_LOG=info

CMD ["/app/rust-backend"]
