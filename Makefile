IPV6_PC=fe80::6e82:4a60:9728:5c8e
IPV4_PC=192.168.50.60
MAC_PC=34:5a:60:7e:ad:1b

tree:
	cargo tree

clean:
	cargo clean

build-debug:
	cargo build

build-release:
	cargo build --release

arp: build-release
	sudo RUST_LOG=info,netprobe=info ./target/release/netprobe arp --ip $(IPV4_PC)

