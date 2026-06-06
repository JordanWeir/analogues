echo "PWD: $(pwd)"
echo
echo "Filesystem:"
df -T .
echo
echo "Cargo:"
which cargo
cargo --version
echo
echo "Rustc:"
which rustc
rustc --version
echo
echo "WSL kernel:"
uname -a
echo
echo "CPU/memory:"
nproc
free -h
echo
echo "Cargo config:"
cargo config get build.target-dir 2>/dev/null || true
cargo config get profile.dev.incremental 2>/dev/null || true
echo
echo "Relevant env:"
env | grep -E 'CARGO|RUST|SCCACHE' || true