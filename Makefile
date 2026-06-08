.PHONY: arch arch-deps mac mac-deps run

CHANNEL ?= oss
ARTIFACT ?= app
ARCH ?= x86_64
MAC_ARCH ?= $(shell uname -m | sed 's/^arm64$$/aarch64/')
MAC_BUILD_PATH ?= $(HOME)/.cargo/bin:$$PATH
TAG ?= v0.local
DMG_NAME_SUFFIX ?=

run:
	./script/run

arch-deps:
	./script/install_cargo_release_deps

arch:
	GIT_RELEASE_TAG=$(TAG) ./script/bundle --channel $(CHANNEL) --artifact $(ARTIFACT) --packages arch --arch $(ARCH)

mac-deps:
	PATH="$(MAC_BUILD_PATH)" ./script/install_cargo_release_deps
	PATH="$(MAC_BUILD_PATH)" rustup target add $(MAC_ARCH)-apple-darwin
	PATH="$(MAC_BUILD_PATH)" command -v sccache >/dev/null || PATH="$(MAC_BUILD_PATH)" brew install sccache
	PATH="$(MAC_BUILD_PATH)" command -v cargo-bundle >/dev/null || PATH="$(MAC_BUILD_PATH)" cargo install cargo-bundle --git=https://github.com/burtonageo/cargo-bundle --rev ae4c76e92c08774bf54ff077b1c52e3d1cd6c16d
	PATH="$(MAC_BUILD_PATH)" command -v create-dmg >/dev/null || PATH="$(MAC_BUILD_PATH)" brew install create-dmg

mac:
	PATH="$(MAC_BUILD_PATH)" RUSTC_WRAPPER=sccache GIT_RELEASE_TAG=$(TAG) ./script/macos/bundle --channel $(CHANNEL) --artifact $(ARTIFACT) --arch $(MAC_ARCH)$(if $(DMG_NAME_SUFFIX), --dmg-name-suffix $(DMG_NAME_SUFFIX))
