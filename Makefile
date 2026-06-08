.PHONY: arch arch-deps mac mac-deps run wasm-deps wasm wasm-run

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

WASM_PORT ?= 8000
WASM_BUILD_PATH ?= $(HOME)/.local/bin:$(HOME)/.cargo/bin:$$PATH
WASM_TTY_PORT ?= 3030
WASM_TTY_SHELL ?=

wasm-deps:
	rustup target add wasm32-unknown-unknown
	PATH="$(WASM_BUILD_PATH)" ./script/install_cargo_binstall
	wasm_bindgen_version="$$(cargo metadata --format-version 1 | jq -rc '.packages[] | select(.name == "wasm-bindgen") | .version')"; \
		installed_version="$$(PATH="$(WASM_BUILD_PATH)" wasm-bindgen --version 2>/dev/null | awk '{print $$2}')"; \
		if [ "$$installed_version" != "$$wasm_bindgen_version" ]; then \
			PATH="$(WASM_BUILD_PATH)" cargo binstall --force -y wasm-bindgen-cli --version "$$wasm_bindgen_version"; \
		fi
	PATH="$(WASM_BUILD_PATH)" command -v wasm-opt >/dev/null || PATH="$(WASM_BUILD_PATH)" cargo binstall -y wasm-opt
	mkdir -p "$(HOME)/.local/bin"
	PATH="$(WASM_BUILD_PATH)" command -v wasm-split >/dev/null || { \
		wasm_split="$$(mktemp)" || exit $$?; \
		curl -fL https://github.com/getsentry/symbolicator/releases/download/26.3.1/wasm-split-Linux-x86_64 -o "$$wasm_split"; \
		status=$$?; \
		if [ $$status -eq 0 ]; then install -m 755 "$$wasm_split" "$(HOME)/.local/bin/wasm-split"; fi; \
		rm -f "$$wasm_split"; \
		exit $$status; \
	}

wasm:
	@if [ -n "$(filter $(CHANNEL),local dev stable preview)" ] && ! command -v warp-channel-config >/dev/null 2>&1; then \
		echo "CHANNEL=$(CHANNEL) requires warp-channel-config. Run ./script/install_channel_config."; \
		exit 1; \
	fi
	PATH="$(WASM_BUILD_PATH)" GIT_RELEASE_TAG=$(TAG) ./script/wasm/bundle --channel $(CHANNEL)

wasm-run: SHELL := /bin/bash
wasm-run: .SHELLFLAGS := -eo pipefail -c
wasm-run:
	PATH="$(WASM_BUILD_PATH)"; \
	source ./script/wasm/bundle --debug --no-split --channel oss --features release_bundle,gui,remote_tty; \
	bundle_dir="$$(dirname "$$OUT_DIR")"; \
	cp "$$WORKSPACE_ROOT_DIR/script/wasm/dev-index.html" "$$bundle_dir/index.html"; \
	echo "Built Warp to $$bundle_dir"; \
	tty_args=(--port "$(WASM_TTY_PORT)"); \
	if [ -n "$(WASM_TTY_SHELL)" ]; then tty_args+=(--shell "$(WASM_TTY_SHELL)"); fi; \
	uv run python script/wasm/local-tty-server.py "$${tty_args[@]}" & \
	tty_pid=$$!; \
	trap 'kill "$$tty_pid" 2>/dev/null || true; wait "$$tty_pid" 2>/dev/null || true' EXIT INT TERM; \
	cargo run --release --package serve-wasm -- "$$bundle_dir" --port $(WASM_PORT)
