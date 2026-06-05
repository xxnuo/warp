CHANNEL ?= oss
ARTIFACT ?= app
ARCH ?= x86_64
MAC_ARCH ?= $(shell uname -m | sed 's/^arm64$$/aarch64/')
MAC_BUILD_PATH ?= $(HOME)/.cargo/bin:$$PATH
RUSTC_WRAPPER ?= sccache
TAG ?= v0.local
DOCKER_IMAGE ?= arch-bundle-builder
DMG_NAME_SUFFIX ?=

.PHONY: arch arch-docker arch-image binary deps mac mac-deps release-deps clean-bundles

deps:
	./script/linux/install_build_deps

release-deps:
	./script/install_cargo_release_deps

binary:
	GIT_RELEASE_TAG=$(TAG) ./script/bundle --channel $(CHANNEL) --artifact $(ARTIFACT) --packages none

arch: release-deps
	GIT_RELEASE_TAG=$(TAG) ./script/bundle --channel $(CHANNEL) --artifact $(ARTIFACT) --packages arch --arch $(ARCH)

mac-deps:
	PATH="$(MAC_BUILD_PATH)" ./script/install_cargo_release_deps
	PATH="$(MAC_BUILD_PATH)" rustup target add $(MAC_ARCH)-apple-darwin
	PATH="$(MAC_BUILD_PATH)" command -v sccache >/dev/null || PATH="$(MAC_BUILD_PATH)" brew install sccache
	PATH="$(MAC_BUILD_PATH)" command -v cargo-bundle >/dev/null || PATH="$(MAC_BUILD_PATH)" cargo install cargo-bundle --git=https://github.com/burtonageo/cargo-bundle --rev ae4c76e92c08774bf54ff077b1c52e3d1cd6c16d
	PATH="$(MAC_BUILD_PATH)" command -v create-dmg >/dev/null || PATH="$(MAC_BUILD_PATH)" brew install create-dmg

mac: mac-deps
	PATH="$(MAC_BUILD_PATH)" RUSTC_WRAPPER=$(RUSTC_WRAPPER) GIT_RELEASE_TAG=$(TAG) ./script/macos/bundle --channel $(CHANNEL) --artifact $(ARTIFACT) --arch $(MAC_ARCH)$(if $(DMG_NAME_SUFFIX), --dmg-name-suffix $(DMG_NAME_SUFFIX))

arch-image:
	docker build -t $(DOCKER_IMAGE) .github/actions/bundle_arch_package

arch-docker: binary arch-image
	docker run --rm \
		-v "$(CURDIR):/github/workspace" \
		-v "$(CURDIR)/target:/github/workspace/target" \
		-w /github/workspace \
		-v "$${CARGO_HOME:-$$HOME/.cargo}/git:/home/build/.cargo/git" \
		-v "$${CARGO_HOME:-$$HOME/.cargo}/registry:/home/build/.cargo/registry" \
		$(DOCKER_IMAGE) \
		$(CHANNEL) $(TAG) $(ARCH) $(ARTIFACT)

clean-bundles:
	rm -rf target/*/bundle/linux
