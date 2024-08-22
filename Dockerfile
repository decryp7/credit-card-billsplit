FROM rust:latest as build

RUN cargo install trunk

WORKDIR /usr/src/credit-card-billsplit
COPY . .

RUN rustup target add wasm32-unknown-unknown
RUN trunk build --release


FROM nginx:latest as prod

COPY --from=build /usr/src/credit-card-billsplit/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/nginx.conf

EXPOSE 80/tcp

CMD ["/usr/sbin/nginx", "-g", "daemon off;"]