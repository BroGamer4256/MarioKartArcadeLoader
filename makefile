.PHONY: dist

all:
	@cargo b --release --target i686-pc-windows-gnu
	@mv target/i686-pc-windows-gnu/release/mkgpdx.dll target/i686-pc-windows-gnu/release/dinput8.dll 

dist:
	@mkdir -p out/
	@cp target/i686-pc-windows-gnu/release/dinput8.dll out/
	@cp -r dist/* out/
	@cd out && 7z a -t7z ../dist.7z .
	@rm -rf out
