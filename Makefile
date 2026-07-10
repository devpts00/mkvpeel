tree:
	cargo tree

clean:
	cargo clean

build-debug:
	cargo build --target x86_64-unknown-linux-musl

build-release:
	cargo build --release --target x86_64-unknown-linux-musl

run-debug: build-debug
	RUST_LOG=info,mkvpeel=trace \
		./target/x86_64-unknown-linux-musl/debug/mkvpeel \
		--src=./in \
		--dst=./out \
		--languages=en,ru \
		--skip-commentary \
		--codec A_TRUEHD:1000 \
		--codec A_EAC3:100 \
		--codec A_AC3:10 \
		--codec A_FLAC:-10 \
		--codec S_TEXT/UTF8:100 \
		--codec S_HDMV/PGS:10 \
		--name пучков:1000 \
		--name full:100 \
		--name sdh:-100 \
		--name форс:-100 \
		--name conversation:-1000 \
		--name comment:-1000 \
		--name коммент:-1000

run-release: build-release
	RUST_LOG=info,mkvpeel=info \
		./target/x86_64-unknown-linux-musl/release/mkvpeel \
		--src=./in \
		--dst=./out \
		--languages=en,ru \
		--skip-commentary \
		--codec A_TRUEHD:1000 \
		--codec A_EAC3:100 \
		--codec A_AC3:10 \
		--codec A_FLAC:-10 \
		--codec S_TEXT/UTF8:100 \
		--codec S_HDMV/PGS:10 \
		--name пучков:1000 \
		--name full:100 \
		--name sdh:-100 \
		--name форс:-100 \
		--name conversation:-1000 \
		--name comment:-1000 \
		--name коммент:-1000

pull-docker:
	docker compose pull

run-docker: build-release
	docker compose up

#run-release: build-release
#	RUST_LOG=info,mkvpeel=info ./target/release/mkvpeel --src="./dat/Fallen.mkv" --dst=./dat/F.mkv --languages=en,ru
