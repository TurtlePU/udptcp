FROM rust:alpine as build
COPY . .
RUN apk add --no-cache build-base
RUN cargo install --root / --path .

FROM alpine:latest
COPY --from=build /bin/udptcp /udptcp
RUN apk add --no-cache iproute2-tc
