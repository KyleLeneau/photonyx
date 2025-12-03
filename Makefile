-include .env

clean:
	echo "cleaning build folder..."
	rm -rf ./dist

build: clean
	# bump the version with `uv version --bump <major|minor|patch> OR uv version <version>`
	uv build --no-sources

publish:
	uv publish --token ${UV_PUBLISH_TOKEN}

test:
	uv run pytest

check:
	uv run ruff check
	@echo ""
	uv run ty check --output-format concise

format:
	uv run ruff format

sync-all:
	uv sync --all-packages --dev
