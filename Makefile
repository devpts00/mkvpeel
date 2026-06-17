tree:
	cargo tree

clean:
	cargo clean

build-debug:
	cargo build --target x86_64-unknown-linux-musl

build-release:
	cargo build --release --target x86_64-unknown-linux-musl

run-debug: build-debug
	RUST_LOG=info,mkvpeel=info ./target/x86_64-unknown-linux-musl/debug/mkvpeel --src=/home/akz/ds923/downloads --dst=./out --languages=en,ru

run-release: build-release
	RUST_LOG=info,mkvpeel=info ./target/x86_64-unknown-linux-musl/release/mkvpeel --src=/home/akz/ds923/downloads --dst=./out --languages=en,ru

#run-release: build-release
#	RUST_LOG=info,mkvpeel=info ./target/release/mkvpeel --src="./dat/Fallen.mkv" --dst=./dat/F.mkv --languages=en,ru

