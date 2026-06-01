CHANNEL ?= oss
ARTIFACT ?= app
ARCH ?= x86_64
TAG ?= v0.local
DOCKER_IMAGE ?= arch-bundle-builder

.PHONY: arch arch-docker arch-image binary deps release-deps clean-bundles

deps:
	./script/linux/install_build_deps

release-deps:
	./script/install_cargo_release_deps

binary:
	GIT_RELEASE_TAG=$(TAG) ./script/bundle --channel $(CHANNEL) --artifact $(ARTIFACT) --packages none

arch: release-deps
	GIT_RELEASE_TAG=$(TAG) ./script/bundle --channel $(CHANNEL) --artifact $(ARTIFACT) --packages arch --arch $(ARCH)

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
