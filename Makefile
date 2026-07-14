CARGO ?= cargo
PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
PROFILE ?= release

.PHONY: all lite run check test fmt clippy install clean

all:
	$(CARGO) build --profile $(PROFILE)

lite:
	$(CARGO) build --release --no-default-features

run:
	$(CARGO) run

check:
	$(CARGO) check --all-features

test:
	$(CARGO) test --all-features

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --all-targets --all-features -- -D warnings

install: all
	mkdir -p $(BINDIR)
	cp target/$(PROFILE)/orion $(BINDIR)/orion

clean:
	$(CARGO) clean
