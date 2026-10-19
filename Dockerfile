# Multi-arch Unified Dockerfile for iventoymersed
# Supports: linux/amd64, linux/arm/v7, linux/arm64
ARG BASE_IMAGE=debian:bookworm-slim
FROM ${BASE_IMAGE}

ARG TARGETARCH

RUN echo 'nameserver 223.5.5.5' > /etc/resolv.conf \
    && (sed -i 's/deb.debian.org/mirrors.ustc.edu.cn/g; s/security.debian.org/mirrors.ustc.edu.cn/g' /etc/apt/sources.list.d/debian.sources 2>/dev/null || sed -i 's/deb.debian.org/mirrors.ustc.edu.cn/g; s/security.debian.org/mirrors.ustc.edu.cn/g' /etc/apt/sources.list) \
    && apt-get update \
    && apt-get install -y --no-install-recommends tgt ca-certificates procps kmod \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary based on architecture or default to dist/iventoymersed
COPY dist/iventoymersed* /app/bin_staging/
COPY docker/entrypoint.sh /app/entrypoint.sh

RUN if [ "$TARGETARCH" = "arm" ] && [ -f /app/bin_staging/iventoymersed-armv7 ]; then \
        mv /app/bin_staging/iventoymersed-armv7 /app/iventoymersed; \
    elif [ "$TARGETARCH" = "arm64" ] && [ -f /app/bin_staging/iventoymersed-aarch64 ]; then \
        mv /app/bin_staging/iventoymersed-aarch64 /app/iventoymersed; \
    else \
        mv /app/bin_staging/iventoymersed /app/iventoymersed; \
    fi \
    && rm -rf /app/bin_staging \
    && chmod +x /app/iventoymersed /app/entrypoint.sh

EXPOSE 16000 16001 9111 3260

ENTRYPOINT ["/app/entrypoint.sh"]
