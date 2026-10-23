ARG LLAMA_CPP_COMMIT=18443257a30c884d5332abb8e7dc43c7ffe42fda

FROM ubuntu:24.04 AS llama-build
ARG LLAMA_CPP_COMMIT
RUN apt-get update && apt-get install --no-install-recommends -y \
    build-essential ca-certificates cmake git && rm -rf /var/lib/apt/lists/*
RUN git clone --depth 1 https://github.com/ggml-org/llama.cpp.git /src/llama.cpp \
    && git -C /src/llama.cpp fetch --depth 1 origin "${LLAMA_CPP_COMMIT}" \
    && git -C /src/llama.cpp checkout --detach "${LLAMA_CPP_COMMIT}"
RUN cmake -S /src/llama.cpp -B /src/llama-build \
    -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DGGML_METAL=OFF \
    -DGGML_BLAS=OFF -DLLAMA_BUILD_TESTS=OFF -DLLAMA_BUILD_EXAMPLES=OFF \
    -DLLAMA_CURL=OFF -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /src/llama-build --config Release --target llama --parallel 2 \
    && mkdir -p /opt/talvik/lib \
    && find /src/llama-build -type f -o -type l | grep -E '/(libllama|libggml)[^/]*\.(so|so\.[0-9]+)$' | xargs -r -I{} cp -a {} /opt/talvik/lib/

FROM rust:1.90-bookworm AS talvik-build
COPY --from=llama-build /opt/talvik/lib /opt/talvik/lib
WORKDIR /src
COPY . .
ENV LLAMA_CPP_LIB_DIR=/opt/talvik/lib
ENV LLAMA_CPP_REQUIRED=1
RUN cargo build --release --locked -p oid-console --bin talvik --bin talvikd

FROM ubuntu:24.04
RUN apt-get update && apt-get install --no-install-recommends -y \
    ca-certificates curl && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --home-dir /var/lib/talvik --shell /usr/sbin/nologin talvik
COPY --from=talvik-build /src/target/release/talvik /opt/talvik/bin/talvik
COPY --from=talvik-build /src/target/release/talvikd /opt/talvik/bin/talvikd
COPY --from=talvik-build /opt/talvik/lib /opt/talvik/lib
RUN mkdir -p /var/lib/talvik/models /var/lib/talvik/state \
    && chown -R talvik:talvik /var/lib/talvik
ENV TALVIK_HOME=/var/lib/talvik
ENV TALVIK_STATE_DIR=/var/lib/talvik/state
ENV TALVIK_MODEL_DIR=/var/lib/talvik/models
ENV TALVIK_HTTP_ADDR=0.0.0.0:11434
ENV TALVIK_HTTP_AUTH=true
USER talvik
EXPOSE 11434
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=6 \
  CMD curl --fail --silent http://127.0.0.1:11434/healthz || exit 1
ENTRYPOINT ["/opt/talvik/bin/talvikd"]
