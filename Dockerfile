FROM rust:1.72.1 as build

# create a new empty shell project
RUN USER=root cargo new --bin countrygen
WORKDIR /countrygen

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy your source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/countrygen*
RUN cargo build --release

# our final base
FROM ubuntu:24.04

RUN set -eux; \
		apt update; \
		apt install -y --no-install-recommends ca-certificates;

# copy the build artifact from the build stage
COPY --from=build /countrygen/target/release/countrygen .

# set the startup command to run your binary
CMD ["./countrygen"]