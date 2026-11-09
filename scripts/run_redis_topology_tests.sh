#!/usr/bin/env bash
# 搭建 TLS / Cluster / Sentinel 三种真实 Redis 拓扑并运行对应 ignored 集成测试。
#
# 依赖：docker、openssl、bash、cargo。
# 环境变量：
#   REDIS_TEST_HOST_IP  容器与宿主都可访问的 IP（默认取 `hostname -I` 的第一个）。
#
# 该脚本只创建/删除以下命名容器，不会触碰其他 Redis 容器：
#   yang-redis-tls
#   yang-redis-cluster-0..5
#   yang-redis-master / yang-redis-replica
#   yang-redis-sentinel-26379..26381
set -euo pipefail
# Git Bash 会把容器内路径（/tls/...）转换成 Windows 路径，必须关闭。
export MSYS_NO_PATHCONV=1

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORKDIR="$ROOT/target/redis-topology"
HOSTIP="${REDIS_TEST_HOST_IP:-}"
if [ -z "$HOSTIP" ]; then
    HOSTIP="$(hostname -I 2>/dev/null | awk '{print $1}' || true)"
fi
if [ -z "$HOSTIP" ]; then
    echo "请设置 REDIS_TEST_HOST_IP（容器与宿主都可访问的 IP）" >&2
    exit 1
fi

# Git Bash 下把挂载源转换成 Windows 路径；Linux 下保持原路径。
WINROOT="$(cd "$ROOT" && pwd -W 2>/dev/null || printf '%s' "$ROOT")"
MSYS_PREFIX=""
case "$(uname -s 2>/dev/null || true)" in
    MINGW* | MSYS* | CYGWIN*) MSYS_PREFIX="/" ;;
esac

cleanup() {
    docker rm -f \
        yang-redis-tls \
        yang-redis-cluster-0 yang-redis-cluster-1 yang-redis-cluster-2 \
        yang-redis-cluster-3 yang-redis-cluster-4 yang-redis-cluster-5 \
        yang-redis-master yang-redis-replica \
        yang-redis-sentinel-26379 yang-redis-sentinel-26380 yang-redis-sentinel-26381 \
        >/dev/null 2>&1 || true
}
trap cleanup EXIT

wait_for() {
    local description="$1"
    shift
    for _ in $(seq 1 60); do
        if "$@" >/dev/null 2>&1; then
            return 0
        fi
        sleep 1
    done
    echo "等待超时: $description" >&2
    return 1
}

echo "==> 生成 TLS 自签名 CA 与服务端证书"
mkdir -p "$WORKDIR/tls"
(cd "$WORKDIR/tls" && rm -f ca.crt ca.key server.crt server.key server.csr ext.cnf ca.srl \
    && openssl req -x509 -newkey rsa:2048 -nodes -keyout ca.key -out ca.crt -days 2 \
        -subj "${MSYS_PREFIX}/CN=yang-test-ca" >/dev/null 2>&1 \
    && openssl req -newkey rsa:2048 -nodes -keyout server.key -out server.csr -days 2 \
        -subj "${MSYS_PREFIX}/CN=localhost" >/dev/null 2>&1 \
    && printf 'subjectAltName=DNS:localhost,IP:127.0.0.1\n' > ext.cnf \
    && openssl x509 -req -in server.csr -CA ca.crt -CAkey ca.key -CAcreateserial \
        -out server.crt -days 2 -extfile ext.cnf >/dev/null 2>&1)

echo "==> 启动 TLS Redis (6382)"
docker rm -f yang-redis-tls >/dev/null 2>&1 || true
MSYS_NO_PATHCONV=1 docker run -d --name yang-redis-tls -p 6382:6382 \
    -v "$WINROOT/target/redis-topology/tls:/tls:ro" redis:7-alpine \
    redis-server --port 0 --tls-port 6382 --tls-cert-file /tls/server.crt \
    --tls-key-file /tls/server.key --tls-ca-cert-file /tls/ca.crt \
    --tls-auth-clients no --save '' >/dev/null
wait_for "TLS Redis 就绪" docker exec yang-redis-tls redis-cli --tls \
    --cacert /tls/ca.crt -h 127.0.0.1 -p 6382 PING

echo "==> 启动 6 节点 Redis Cluster (7000-7005)"
for i in 0 1 2 3 4 5; do
    docker rm -f "yang-redis-cluster-$i" >/dev/null 2>&1 || true
    data_port=$((7000 + i))
    bus_port=$((17000 + i))
    docker run -d --name "yang-redis-cluster-$i" \
        -p "${data_port}:6379" -p "${bus_port}:16379" redis:7-alpine \
        redis-server --port 6379 --cluster-enabled yes --cluster-config-file nodes.conf \
        --cluster-node-timeout 5000 --cluster-port 16379 \
        --cluster-announce-ip "$HOSTIP" --cluster-announce-port "$data_port" \
        --cluster-announce-bus-port "$bus_port" --save '' --appendonly no >/dev/null
done
sleep 2
docker run --rm redis:7-alpine redis-cli --cluster create \
    "$HOSTIP:7000" "$HOSTIP:7001" "$HOSTIP:7002" \
    "$HOSTIP:7003" "$HOSTIP:7004" "$HOSTIP:7005" \
    --cluster-replicas 1 --cluster-yes >/dev/null
wait_for "Cluster 状态 ok" bash -c \
    "docker run --rm redis:7-alpine redis-cli -h $HOSTIP -p 7000 CLUSTER INFO | grep -q 'cluster_state:ok'"

echo "==> 启动 Sentinel 拓扑 (master 6380 / replica 6381 / sentinels 26379-26381)"
mkdir -p "$WORKDIR/sentinel"
for port in 26379 26380 26381; do
    cat > "$WORKDIR/sentinel/sentinel-$port.conf" <<EOF
port $port
bind 0.0.0.0
protected-mode no
sentinel monitor mymaster $HOSTIP 6380 2
sentinel down-after-milliseconds mymaster 5000
sentinel failover-timeout mymaster 10000
sentinel announce-ip $HOSTIP
sentinel announce-port $port
EOF
done
for name in master replica sentinel-26379 sentinel-26380 sentinel-26381; do
    docker rm -f "yang-redis-$name" >/dev/null 2>&1 || true
done
docker run -d --name yang-redis-master -p 6380:6379 redis:7-alpine \
    redis-server --save '' --appendonly no --protected-mode no >/dev/null
docker run -d --name yang-redis-replica -p 6381:6379 redis:7-alpine \
    redis-server --save '' --appendonly no --protected-mode no \
    --replicaof "$HOSTIP" 6380 --replica-announce-ip "$HOSTIP" \
    --replica-announce-port 6381 >/dev/null
for port in 26379 26380 26381; do
    MSYS_NO_PATHCONV=1 docker run -d --name "yang-redis-sentinel-$port" \
        -p "${port}:${port}" \
        -v "$WINROOT/target/redis-topology/sentinel:/sentinel:rw" redis:7-alpine \
        redis-server "/sentinel/sentinel-$port.conf" --sentinel >/dev/null
done
wait_for "Sentinel 发现 master" bash -c \
    "docker run --rm redis:7-alpine redis-cli -h $HOSTIP -p 26379 SENTINEL get-master-addr-by-name mymaster | grep -q '$HOSTIP'"

echo "==> TLS 拓扑测试"
REDIS_TLS_URL=rediss://127.0.0.1:6382 cargo test -p yang-db \
    --features redis-tls,redis-tls-insecure-test \
    --test integration_redis_topologies tls_ --locked -- --ignored --test-threads=1

echo "==> Cluster 拓扑测试"
REDIS_CLUSTER_URLS="redis://$HOSTIP:7000,redis://$HOSTIP:7001,redis://$HOSTIP:7002" \
    cargo test -p yang-db --features redis-cluster \
    --test integration_redis_topologies cluster_ --locked -- --ignored --test-threads=1

echo "==> Sentinel 拓扑测试"
REDIS_SENTINEL_URLS="redis://$HOSTIP:26379,redis://$HOSTIP:26380,redis://$HOSTIP:26381" \
REDIS_SENTINEL_MASTER=mymaster \
    cargo test -p yang-db --features redis-sentinel \
    --test integration_redis_topologies sentinel_ --locked -- --ignored --test-threads=1

echo "Redis topology tests passed"
