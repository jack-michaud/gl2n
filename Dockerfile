FROM ekidd/rust-musl-builder:stable as builder

ADD . /home/rust/src

RUN cargo build --release

FROM alpine

COPY --from=builder /home/rust/src/target/x86_64-unknown-linux-musl/release/glennbot /glennbot

ENV RUST_LOG=debug
CMD ["/glennbot"]
