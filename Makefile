tree:
	cargo tree

clean:
	cargo clean

build-debug:
	cargo build

build-release:
	cargo build --release

run-debug: build-debug
	RUST_LOG=info,mkvpeel=info ./target/debug/mkvpeel --src=./dat/The.Italian.Job.1969.2160p.BluRay.REMUX.HEVC.DTS-HD.MA.5.1.SHD13.mkv --dst=./dat/It.mkv --languages=en,ru

#run-release: build-release
#	RUST_LOG=info,mkvpeel=info ./target/release/mkvpeel --src="./dat/The 6th Day.mkv" --dst=./dat/6th.mkv --languages=en,ru

run-release: build-release
	RUST_LOG=info,mkvpeel=info ./target/release/mkvpeel --src="./dat/Fallen.mkv" --dst=./dat/F.mkv --languages=en,ru

