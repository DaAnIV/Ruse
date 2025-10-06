FROM rust:1.89-bullseye AS builder

# download public key for github.com
RUN mkdir -p -m 0600 ~/.ssh && ssh-keyscan github.com >> ~/.ssh/known_hosts

WORKDIR /usr/src/
RUN --mount=type=ssh git clone git@github.com:DaAnIV/Ruse.git

WORKDIR /usr/src/Ruse
RUN cargo install --path Ruse

FROM sobeq:latest

WORKDIR /workspace
RUN --mount=type=ssh git clone git@github.com:DaAnIV/Ruse.git
COPY --from=builder /usr/local/cargo/bin/Ruse /usr/local/bin/Ruse

WORKDIR /workspace/Ruse

WORKDIR /workspace/
CMD ["bash"]
