REPO              := ravidhu/nabu
TARGET            := aarch64-apple-darwin
BIN_DIR           := bin
BINARY            := $(BIN_DIR)/nabu-$(TARGET)

APP_PATH          := $(HOME)/Applications/nabu.app
WESPEAKER_CACHE   := $(HOME)/.wespeaker/english
WESPEAKER_ARCHIVE := voxceleb_resnet221_LM.tar.gz

.PHONY: dev-setup build run install reinstall uninstall mirror-wespeaker test clean

dev-setup:
	rustup target add $(TARGET)
	uv sync --directory transcribe
	cargo run -- --setup

build:
	cargo build --release --target $(TARGET)
	cp target/$(TARGET)/release/nabu $(BINARY)

run:
	cargo run

install: build
	sudo cp $(BINARY) /usr/local/bin/nabu
	sudo xattr -d com.apple.quarantine /usr/local/bin/nabu 2>/dev/null || true
	rm -rf ~/.nabu
	nabu --setup
	mkdir -p $(HOME)/Applications
	osacompile -o $(APP_PATH) -e 'tell application "Terminal" to do script "nabu"'
	@echo "nabu installed — double-click nabu.app in ~/Applications to start recording."

reinstall: build
	sudo cp $(BINARY) /usr/local/bin/nabu
	rm -rf ~/.nabu
	nabu --setup

uninstall:
	./uninstall.sh

test:
	cargo test --bin nabu
	uv run --directory transcribe pytest

mirror-wespeaker:
	@test -f $(WESPEAKER_CACHE)/avg_model.pt || (echo "Run 'nabu --setup' first to cache the model." && exit 1)
	mkdir -p models/wespeaker
	cd $(WESPEAKER_CACHE) && tar -czf $(CURDIR)/models/wespeaker/$(WESPEAKER_ARCHIVE) avg_model.pt config.yaml
	@echo "Model archived to models/wespeaker/$(WESPEAKER_ARCHIVE) — commit and push to publish."

clean:
	cargo clean
	rm -rf ~/.nabu
	rm -rf ~/.wespeaker
	rm -rf ~/nabu_data/.tmp
	rm -rf transcribe/.venv
	find transcribe -type d -name __pycache__ -exec rm -rf {} +
