ARG FEATURES="cloud-hypervisor,qemu"
ARG EXTRA_PACKAGES=""

FROM registry.access.redhat.com/ubi9/ubi as builder

RUN dnf install -y gcc make openssl-devel && dnf clean all

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /build
COPY . .

ARG FEATURES
RUN cargo build --release --features "${FEATURES}"

FROM registry.access.redhat.com/ubi9/ubi-minimal

ARG EXTRA_PACKAGES
RUN if [ -n "${EXTRA_PACKAGES}" ]; then microdnf install -y ${EXTRA_PACKAGES} && microdnf clean all; fi

COPY --from=builder /build/target/release/vbmc-rs /usr/local/bin/vbmc-rs

EXPOSE 8000
ENTRYPOINT ["/usr/local/bin/vbmc-rs"]
CMD ["-c", "/etc/vbmc-rs/config.toml"]
