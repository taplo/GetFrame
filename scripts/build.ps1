param(
    [switch]$NoCache,
    [string]$Config = "config.docker.yaml"
)

$VM_USER = "taplo"
$VM_HOST = "127.0.0.1"
$VM_PATH = "/home/taplo/getframe"

# Sync files
Write-Host "=== Syncing source files to VM ==="
@(
    "Cargo.toml", "Cargo.lock",
    "config.docker.yaml", "docker-compose.yml", "config.example.yaml",
    "Dockerfile", ".dockerignore"
) | ForEach-Object {
    scp "D:\projects\GetFrame\$_" "${VM_USER}@${VM_HOST}:${VM_PATH}/"
}

@("src", "migrations", "tests", "web", ".cargo") | ForEach-Object {
    scp -r "D:\projects\GetFrame\$_" "${VM_USER}@${VM_HOST}:${VM_PATH}/"
}

# Build on VM
Write-Host "=== Building Docker image on VM ==="
$cacheFlag = if ($NoCache) { "--no-cache" } else { "" }

ssh -o BatchMode=yes "${VM_USER}@${VM_HOST}" @"
cd ${VM_PATH}
docker buildx build --network host \
  --build-arg https_proxy=http://192.168.3.200:8787 \
  ${cacheFlag} \
  -t getframe-worker:latest .
"@

Write-Host "=== Build complete ==="
