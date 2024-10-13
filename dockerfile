FROM rust:1.81.0 AS build
# ENV PKG_CONFIG_ALLOW_CROSS=1

WORKDIR /usr/src/hc-axum
COPY . .

RUN cargo install --path hc-axum

FROM gcr.io/distroless/cc-debian12

COPY --from=build /usr/local/cargo/bin/hc-axum /usr/local/bin/hc-axum

# ENV HOPLITE_DB= set from outside

EXPOSE 8088

WORKDIR /usr/local/bin

CMD ["hc-axum"]
