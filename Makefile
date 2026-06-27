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
		--buff a:c:A_TRUEHD:1000 \
		--buff a:c:A_EAC3:100 \
		--buff a:c:A_AC3:10 \
		--buff a:c:A_DTS:-10 \
		--buff a:n:пучков:1000 \
		--buff a:n:comment:-1000 \
		--buff a:n:коммент:-1000 \
		--buff s:c:S_TEXT/UTF8:100 \
		--buff s:c:S_HDMV/PGS:10 \
		--buff s:n:пучков:1000 \
		--buff s:n:full:100 \
		--buff s:n:sdh:-100 \
		--buff s:n:форс:-100 \
		--buff s:n:comment:-1000 \
		--buff s:n:коммент:-1000

run-release: build-release
	RUST_LOG=info,mkvpeel=info \
		./target/x86_64-unknown-linux-musl/release/mkvpeel \
		--src=./in \
		--dst=./out \
		--languages=en,ru \
		--buff a:c:A_TRUEHD:1000 \
		--buff a:c:A_EAC3:100 \
		--buff a:c:A_AC3:10 \
		--buff a:c:A_DTS:-10 \
		--buff a:n:пучков:1000 \
		--buff a:n:comment:-1000 \
		--buff a:n:коммент:-1000 \
		--buff s:c:S_TEXT/UTF8:100 \
		--buff s:c:S_HDMV/PGS:10 \
		--buff s:n:пучков:1000 \
		--buff s:n:full:100 \
		--buff s:n:sdh:-100 \
		--buff s:n:форс:-100 \
		--buff s:n:comment:-1000 \
		--buff s:n:коммент:-1000

pull-docker:
	docker compose pull

run-docker: build-release
	docker compose up

#run-release: build-release
#	RUST_LOG=info,mkvpeel=info ./target/release/mkvpeel --src="./dat/Fallen.mkv" --dst=./dat/F.mkv --languages=en,ru
