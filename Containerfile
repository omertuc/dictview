FROM alpine as builder

RUN apk add cargo rust

# Trigger an index update
RUN cargo search foo

COPY . /src
WORKDIR /src
RUN cargo build --release

FROM alpine

COPY --from=builder /src/target/release/dictview /usr/bin/dictview

ENTRYPOINT ["/usr/bin/dictview"]

