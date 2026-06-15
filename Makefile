tree:
	cargo tree

clean:
	cargo clean

build-debug:
	cargo build

build-release:
	cargo build --release

run-release: build-release
	RUST_LOG=info,mkvpeel=info ./target/release/mkvpeel

