tree:
	cargo tree

clean:
	cargo clean

build-debug:
	cargo build

build-release:
	cargo build --release

run-debug: build-debug
	RUST_LOG=info,mkvpeel=info ./target/debug/mkvpeel --src=/home/akz/ds923/downloads --dst=./out --languages=en,ru

run-release: build-release
	RUST_LOG=info,mkvpeel=info ./target/release/mkvpeel --src=/home/akz/ds923/downloads --dst=./out --languages=en,ru

#run-release: build-release
#	RUST_LOG=info,mkvpeel=info ./target/release/mkvpeel --src="./dat/Fallen.mkv" --dst=./dat/F.mkv --languages=en,ru

