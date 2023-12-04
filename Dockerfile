FROM rust:1

WORKDIR /usr/src/app
COPY . .

RUN cargo install --path .

EXPOSE 1234

CMD ["api"]