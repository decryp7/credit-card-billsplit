FROM rust:latest AS build

# --locked uses trunk's own Cargo.lock; without it newer dependency releases can break the build.
RUN cargo install trunk --version 0.21.14 --locked

WORKDIR /usr/src/credit-card-billsplit
COPY . .

RUN rustup target add wasm32-unknown-unknown
RUN trunk build --release


FROM nginx:latest AS prod

COPY --from=build /usr/src/credit-card-billsplit/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf

EXPOSE 80/tcp

CMD ["/usr/sbin/nginx", "-g", "daemon off;"]