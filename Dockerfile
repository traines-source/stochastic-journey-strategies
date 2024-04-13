FROM traines-source/stost-build-env

WORKDIR /app
COPY . .

RUN cargo build --release

RUN cargo test

EXPOSE 1234

CMD ["target/release/api", "deployments/config.json"]